//! Разбор LAPS-атрибутов объекта компьютера.
//!
//! Источники данных (см. `services/ldap` и MS LAPS schema reference):
//! * legacy LAPS: `ms-Mcs-AdmPwd` (строка-пароль),
//!   `ms-Mcs-AdmPwdExpirationTime` (десятичный FILETIME);
//! * Windows LAPS, открытое хранение: `msLAPS-Password` — JSON
//!   `{"n":"Administrator","t":"1d8161b41c41cde","p":"<пароль>"}`
//!   (`t` — **шестнадцатеричный** FILETIME момента обновления),
//!   `msLAPS-PasswordExpirationTime` (десятичный FILETIME);
//! * Windows LAPS, шифрованное хранение: `msLAPS-EncryptedPassword`
//!   (бинарный DPAPI-NG/KDS-блок) — фиксируем только факт присутствия:
//!   расшифровка выполняется контроллером домена по RPC и доступна
//!   через `Get-LapsADPassword`, но не нативным LDAP-чтением.

use serde::Deserialize;

use crate::services::ldap::NormalizedEntry;

use super::model::{LapsFlavor, LapsInfo};
use super::time;

/// LDAP-имена атрибутов.
pub const ATTR_CN: &str = "cn";
pub const ATTR_SAM_ACCOUNT_NAME: &str = "samaccountname";
pub const ATTR_DNS_HOST_NAME: &str = "dnshostname";
pub const ATTR_OPERATING_SYSTEM: &str = "operatingsystem";
pub const ATTR_LEGACY_PASSWORD: &str = "ms-mcs-admpwd";
pub const ATTR_LEGACY_EXPIRATION: &str = "ms-mcs-admpwdexpirationtime";
pub const ATTR_WIN_PASSWORD: &str = "mslaps-password";
pub const ATTR_WIN_EXPIRATION: &str = "mslaps-passwordexpirationtime";
pub const ATTR_WIN_ENCRYPTED: &str = "mslaps-encryptedpassword";

/// Полный список атрибутов поискового запроса.
///
/// `msLAPS-EncryptedPassword` запрашивается только для детекта presence —
/// значение (бинарный блок) наружу не отдаётся и в логах не появляется.
pub const COMPUTER_ATTRIBUTES: &[&str] = &[
    "cn",
    "sAMAccountName",
    "dNSHostName",
    "operatingSystem",
    "ms-Mcs-AdmPwd",
    "ms-Mcs-AdmPwdExpirationTime",
    "msLAPS-Password",
    "msLAPS-PasswordExpirationTime",
    "msLAPS-EncryptedPassword",
];

/// JSON содержимого `msLAPS-Password` (документированные поля `n`/`t`/`p`).
///
/// Допускаются и «длинные» имена полей — некоторые сторонние агенты
/// и ранние сборки писали `{"Password":...,"TimeStamp":...}`; парсер
/// терпим к обоим форматам.
#[derive(Debug, Deserialize)]
struct WindowsLapsJson {
    #[serde(default, alias = "AccountName")]
    n: Option<String>,
    #[serde(default)]
    t: Option<String>,
    #[serde(default, alias = "Password")]
    p: Option<String>,
    #[serde(default, alias = "TimeStamp")]
    timestamp: Option<String>,
}

/// Данные открытого пароля Windows LAPS.
#[derive(Debug, PartialEq, Eq)]
pub struct WindowsPlainPassword {
    pub account_name: Option<String>,
    pub password: String,
    /// FILETIME момента обновления пароля (поле `t`).
    pub updated_at: Option<i64>,
}

/// Разбор JSON из `msLAPS-Password`.
///
/// Возвращает `None`, если JSON невалиден или в нём нет пароля —
/// такой атрибут трактуется как отсутствие данных (см. `pick_flavor`).
/// Совместимость с «TimeStamp» в десятичном виде.
pub fn parse_windows_plain_json(raw: &str) -> Option<WindowsPlainPassword> {
    let parsed: WindowsLapsJson = serde_json::from_str(raw.trim()).ok()?;
    let password = parsed.p.filter(|p| !p.is_empty())?;

    let updated_at = parsed
        .t
        .as_deref()
        .and_then(time::parse_filetime_hex)
        .or_else(|| {
            parsed.timestamp.as_deref().and_then(|value| {
                time::parse_filetime_decimal(value).or_else(|| time::parse_filetime_hex(value))
            })
        });

    Some(WindowsPlainPassword {
        account_name: parsed.n.filter(|n| !n.is_empty()),
        password,
        updated_at,
    })
}

/// Определяет вариант хранения по набору присутствующих атрибутов.
///
/// Приоритет: открытый Windows LAPS > legacy > шифрованный Windows LAPS.
/// (Открытый новый формат предпочтительнее legacy-остатков миграции;
/// шифрованный — только если открытых паролей нет вовсе.)
pub fn pick_flavor(
    windows_plain_password: Option<&WindowsPlainPassword>,
    legacy_password: Option<&str>,
    encrypted_present: bool,
) -> LapsFlavor {
    if windows_plain_password.is_some() {
        LapsFlavor::WindowsPlain
    } else if legacy_password.is_some() {
        LapsFlavor::Legacy
    } else if encrypted_present {
        LapsFlavor::WindowsEncrypted
    } else {
        LapsFlavor::None
    }
}

/// Собирает [`LapsInfo`] из записи каталога.
/// Срок истечения: для Windows LAPS — новый атрибут, для legacy — старый.
/// В смешанном домене берём тот, что соответствует выбранному варианту,
/// с запасным падением на второй.
/// Писать срок истечения умеем в атрибут того семейства, которое
/// присутствует на объекте (операция сброса/продления).
/// Шифрованный пароль не читается нативно — наружу не отдаём ничего.
pub fn build_laps_info(entry: &NormalizedEntry) -> LapsInfo {
    let computer_name = entry
        .text(ATTR_SAM_ACCOUNT_NAME)
        .map(|sam| sam.trim_end_matches('$').to_string())
        .or_else(|| entry.text(ATTR_CN))
        .unwrap_or_else(|| cn_from_dn(&entry.dn));

    let windows_plain = entry
        .text(ATTR_WIN_PASSWORD)
        .as_deref()
        .and_then(parse_windows_plain_json);
    let legacy_password = entry.text(ATTR_LEGACY_PASSWORD);
    let encrypted_present = entry.has(ATTR_WIN_ENCRYPTED);

    let flavor = pick_flavor(
        windows_plain.as_ref(),
        legacy_password.as_deref(),
        encrypted_present,
    );

    let win_expiration = entry
        .text(ATTR_WIN_EXPIRATION)
        .as_deref()
        .and_then(time::parse_filetime_decimal);
    let legacy_expiration = entry
        .text(ATTR_LEGACY_EXPIRATION)
        .as_deref()
        .and_then(time::parse_filetime_decimal);
    let expiration = match flavor {
        LapsFlavor::Legacy => legacy_expiration.or(win_expiration),
        LapsFlavor::WindowsPlain | LapsFlavor::WindowsEncrypted => {
            win_expiration.or(legacy_expiration)
        }
        LapsFlavor::None => win_expiration.or(legacy_expiration),
    };

    let win_expiration_present = win_expiration.is_some();
    let legacy_expiration_present = legacy_expiration.is_some();
    let rotation_possible = win_expiration_present || legacy_expiration_present;

    let (password, account_name, updated_at) = match &flavor {
        LapsFlavor::WindowsPlain => {
            let plain = windows_plain.as_ref().expect("flavor вычислен из Some");
            (
                Some(plain.password.clone()),
                plain.account_name.clone(),
                plain.updated_at,
            )
        }
        LapsFlavor::Legacy => (legacy_password.clone(), None, None),
        LapsFlavor::WindowsEncrypted | LapsFlavor::None => (None, None, None),
    };

    LapsInfo {
        dn: entry.dn.clone(),
        computer_name,
        dns_host_name: entry.text(ATTR_DNS_HOST_NAME),
        operating_system: entry.text(ATTR_OPERATING_SYSTEM),
        flavor,
        account_name,
        password,
        updated_at: updated_at.and_then(time::filetime_to_rfc3339_local),
        expires_at: expiration.and_then(time::filetime_to_rfc3339_local),
        decrypted: false,
        decrypt_method: None,
        legacy_present: legacy_password.is_some() || legacy_expiration.is_some(),
        windows_plain_present: windows_plain.is_some() || win_expiration.is_some(),
        encrypted_present,
        rotation_possible,
        win_expiration_present,
        legacy_expiration_present,
    }
}

/// Аварийное извлечение имени из DN (`CN=WS01,OU=...` -> `WS01`).
fn cn_from_dn(dn: &str) -> String {
    dn.split(',')
        .next()
        .and_then(|first| first.split_once('='))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_else(|| dn.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ldap3::SearchEntry;
    use std::collections::HashMap;

    fn entry_with(attrs: Vec<(&str, &str)>) -> NormalizedEntry {
        NormalizedEntry::from_search(SearchEntry {
            dn: "CN=WS01,OU=Workstations,DC=company,DC=ru".to_string(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string(), vec![v.to_string()]))
                .collect::<HashMap<_, _>>(),
            bin_attrs: HashMap::new(),
        })
    }

    #[test]
    /// Формат из MS LAPS schema reference.
    fn parses_documented_windows_laps_json() {
        let raw = r#"{"n":"Administrator","t":"1d8161b41c41cde","p":"A6a3#7%eb!57be4a4B95Z4"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.account_name.as_deref(), Some("Administrator"));
        assert_eq!(parsed.password, "A6a3#7%eb!57be4a4B95Z4");
        assert_eq!(parsed.updated_at, Some(0x1d8161b41c41cde));
    }

    #[test]
    fn parses_custom_admin_account_json() {
        let raw = r#"{"n":"LocalAdmin","t":"1d978c1a2b3c4d5","p":"p@ss"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.account_name.as_deref(), Some("LocalAdmin"));
    }

    #[test]
    fn tolerates_long_field_names() {
        let raw = r#"{"Password":"secret123","TimeStamp":"133256448000000000"}"#;
        let parsed = parse_windows_plain_json(raw).expect("валидный JSON");
        assert_eq!(parsed.password, "secret123");
        assert_eq!(parsed.updated_at, Some(133_256_448_000_000_000));
    }

    #[test]
    fn rejects_broken_or_empty_json() {
        assert!(parse_windows_plain_json("не json").is_none());
        assert!(parse_windows_plain_json(r#"{"n":"Administrator"}"#).is_none());
        assert!(parse_windows_plain_json(r#"{"p":""}"#).is_none());
    }

    #[test]
    fn flavor_priority_prefers_windows_plain_over_legacy() {
        let plain = WindowsPlainPassword {
            account_name: None,
            password: "new".into(),
            updated_at: None,
        };
        assert_eq!(
            pick_flavor(Some(&plain), Some("old"), true),
            LapsFlavor::WindowsPlain
        );
        assert_eq!(pick_flavor(None, Some("old"), true), LapsFlavor::Legacy);
        assert_eq!(pick_flavor(None, None, true), LapsFlavor::WindowsEncrypted);
        assert_eq!(pick_flavor(None, None, false), LapsFlavor::None);
    }

    #[test]
    fn builds_info_for_legacy_computer() {
        let entry = entry_with(vec![
            ("CN", "WS01"),
            ("sAMAccountName", "WS01$"),
            ("dNSHostName", "ws01.company.ru"),
            ("operatingSystem", "Windows 10 Pro"),
            ("ms-Mcs-AdmPwd", "OldP@ssw0rd"),
            ("ms-Mcs-AdmPwdExpirationTime", "133256448000000000"),
        ]);
        let info = build_laps_info(&entry);
        assert_eq!(info.computer_name, "WS01");
        assert_eq!(info.flavor, LapsFlavor::Legacy);
        assert_eq!(info.password.as_deref(), Some("OldP@ssw0rd"));
        assert!(info.expires_at.is_some());
        assert!(info.legacy_present);
        assert!(!info.windows_plain_present);
        assert!(info.rotation_possible);
    }

    #[test]
    fn builds_info_for_windows_laps_plain_computer() {
        let entry = entry_with(vec![
            ("CN", "WS02"),
            ("sAMAccountName", "WS02$"),
            (
                "msLAPS-Password",
                r#"{"n":"Administrator","t":"1d8161b41c41cde","p":"N3wP@ss!"}"#,
            ),
            ("msLAPS-PasswordExpirationTime", "133256448000000000"),
        ]);
        let info = build_laps_info(&entry);
        assert_eq!(info.flavor, LapsFlavor::WindowsPlain);
        assert_eq!(info.password.as_deref(), Some("N3wP@ss!"));
        assert_eq!(info.account_name.as_deref(), Some("Administrator"));
        assert!(info.updated_at.is_some());
        assert!(info.windows_plain_present);
        assert!(info.rotation_possible);
    }

    #[test]
    /// Имитируем бинарный зашифрованный пароль + срок истечения.
    /// Сброс срока возможен и для шифрованного варианта: агент сам
    /// сгенерирует новый пароль, чтение здесь не нужно.
    fn encrypted_only_computer_has_no_password() {
        let with_encrypted = NormalizedEntry::from_search(SearchEntry {
            dn: "CN=WS03,OU=Workstations,DC=company,DC=ru".to_string(),
            attrs: HashMap::from([
                ("cn".to_string(), vec!["WS03".to_string()]),
                ("sAMAccountName".to_string(), vec!["WS03$".to_string()]),
                (
                    "msLAPS-PasswordExpirationTime".to_string(),
                    vec!["133256448000000000".to_string()],
                ),
            ]),
            bin_attrs: HashMap::from([(
                "msLAPS-EncryptedPassword".to_string(),
                vec![vec![0x01, 0x02, 0x03]],
            )]),
        });
        let info = build_laps_info(&with_encrypted);
        assert_eq!(info.flavor, LapsFlavor::WindowsEncrypted);
        assert!(info.password.is_none());
        assert!(info.encrypted_present);
        assert!(info.expires_at.is_some());
        assert!(info.rotation_possible);
    }

    #[test]
    fn extracts_cn_from_dn_as_fallback() {
        assert_eq!(cn_from_dn("CN=WS01,OU=X,DC=y"), "WS01");
        assert_eq!(cn_from_dn("garbage"), "garbage");
    }
}

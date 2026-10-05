//! Доменная логика LAPS: поиск компьютера, чтение пароля, сброс срока.
//!
//! Все функции работают поверх нативного LDAP-клиента
//! ([`crate::services::ldap`]): одно соединение на операцию, никаких
//! внешних процессов, PowerShell и RSAT.
//!
//! Поддерживаемые варианты хранения пароля:
//! * legacy LAPS — `ms-Mcs-AdmPwd` (открытый текст, ACL на атрибут);
//! * Windows LAPS — `msLAPS-Password` (JSON, открытое хранение);
//! * Windows LAPS — `msLAPS-EncryptedPassword` (шифрованное хранение):
//!   факт присутствия и срок истечения читаются, сам пароль не
//!   расшифровывается — дешифровка выполняется контроллером домена
//!   по закрытому RPC ( cmdlet `Get-LapsADPassword`), что принципиально
//!   недоступно «чистому» LDAP-клиенту. UI показывает инженеру понятную
//!   диагностику и доступные действия (например, сброс срока — агент
//!   сгенерирует новый пароль; если политика домена хранит пароли
//!   открыто, он станет читаем).

#[cfg(windows)]
mod decrypt;
#[cfg(windows)]
mod decrypt_module;
#[cfg(not(windows))]
mod decrypt_module {
    use super::parse::WindowsPlainPassword;
    pub fn decrypt_via_laps_module(
        _computer_name: &str,
        _ldap_url: &str,
    ) -> Option<WindowsPlainPassword> {
        None
    }
}
#[cfg(not(windows))]
mod decrypt {
    //! Дешифровка KDS/DPAPI-NG доступна только в Windows-сборке.
    use super::parse::WindowsPlainPassword;
    pub fn decrypt_windows_laps_password(_blob: &[u8]) -> Option<WindowsPlainPassword> {
        None
    }
}
mod expiration;
mod model;
mod parse;
mod query;
mod time;

pub use expiration::RotationMode;
pub use model::{LapsFlavor, LapsInfo};
pub use parse::COMPUTER_ATTRIBUTES;

use ldap3::Scope;
use tracing::{debug, info};

use crate::error::AppError;
use crate::services::config::ConnectionTest;
use crate::services::ldap::{
    open_session, read_root_dse, search, DirectoryCredentials, NormalizedEntry, OPERATION_TIMEOUT,
};

use expiration::build_rotation_mods;
use parse::{build_laps_info, ATTR_WIN_ENCRYPTED};
use query::{find_computers, parse_query};

/// Карточка компьютера: для шифрованного хранения пытаемся расшифровать
/// пароль двумя уровнями: нативный DPAPI-NG/KDS ([`decrypt`]), затем —
/// системный модуль Windows LAPS ([`decrypt_module`], маршрут MS-GKDI+KDS
/// изнутри cmdlet). Не удалось — карточка остаётся без пароля, с
/// диагностикой в UI. Модульный уровень блокирующий — только spawn_blocking.
async fn laps_info_from_entry(entry: &NormalizedEntry, ldap_url: &str) -> LapsInfo {
    let mut info = build_laps_info(entry);
    if info.flavor != LapsFlavor::WindowsEncrypted {
        return info;
    }
    if let Some(blob) = entry.binary(ATTR_WIN_ENCRYPTED) {
        if let Some(plain) = decrypt::decrypt_windows_laps_password(&blob) {
            apply_decrypted(&mut info, plain, "kds");
            return info;
        }
    }
    let computer = info.computer_name.clone();
    let url = ldap_url.to_string();
    let via_module = tokio::task::spawn_blocking(move || {
        decrypt_module::decrypt_via_laps_module(&computer, &url)
    })
    .await
    .ok()
    .flatten();
    if let Some(plain) = via_module {
        apply_decrypted(&mut info, plain, "laps-module");
    }
    info
}

fn apply_decrypted(info: &mut LapsInfo, plain: parse::WindowsPlainPassword, method: &str) {
    info.password = Some(plain.password);
    info.account_name = plain.account_name;
    info.updated_at = plain.updated_at.and_then(time::filetime_to_rfc3339_local);
    info.decrypted = true;
    info.decrypt_method = Some(method.to_string());
}

/// Максимум имён в диагностике неоднозначного поиска.
const AMBIGUOUS_LIST_LIMIT: usize = 8;
/// Поиск компьютера и чтение его LAPS-карточки.
///
/// Ошибки возвращаются для транспортных/протокольных проблем, «не найден»
/// и неоднозначности. Компьютер с шифрованным паролем — **не** ошибка:
/// возвращается карточка с `flavor = WindowsEncrypted` и `password = None`,
/// чтобы UI показал диагностику и доступные операции.
pub async fn search_computer(
    credentials: &DirectoryCredentials<'_>,
    raw_query: &str,
) -> Result<LapsInfo, AppError> {
    let query = parse_query(raw_query)?;

    let (mut ldap, _bind_ms) = open_session(credentials).await?;
    let base = resolve_base_dn(&mut ldap, credentials).await?;
    let entries = find_computers(
        &mut ldap,
        credentials,
        &base,
        &query,
        COMPUTER_ATTRIBUTES.to_vec(),
    )
    .await;
    let entries = match entries {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let _ = ldap.unbind().await;

    match entries.len() {
        0 => Err(AppError::ComputerNotFound(format!(
            "«{raw_query}» не найден в {base}. Проверьте имя; для машин вне \
             стандартных контейнеров укажите точный Base DN в настройках."
        ))),
        1 => {
            let info = laps_info_from_entry(&entries[0], credentials.ldap_url).await;
            ensure_password_readable(&info)?;
            info!(
                computer = %info.computer_name,
                flavor = info.flavor.audit_label(),
                "LAPS: карточка компьютера получена"
            );
            Ok(info)
        }
        _ => {
            let names: Vec<String> = entries
                .iter()
                .map(|entry| build_laps_info(entry).computer_name)
                .take(AMBIGUOUS_LIST_LIMIT)
                .collect();
            Err(AppError::ComputerAmbiguous(names.join(", ")))
        }
    }
}

/// Карточка компьютера обязана содержать читаемый пароль, кроме случаев
/// шифрованного хранения (там UI показывает диагностику и управление сроком).
/// Шифрованный вариант возвращаем карточкой с `password = None` —
/// это не ошибка: UI показывает диагностику и доступные операции.
fn ensure_password_readable(info: &LapsInfo) -> Result<(), AppError> {
    match info.flavor {
        model::LapsFlavor::Legacy
        | model::LapsFlavor::WindowsPlain
        | model::LapsFlavor::WindowsEncrypted => Ok(()),
        model::LapsFlavor::None => Err(AppError::NoLapsData(format!(
            "Компьютер {} найден, но пароля LAPS не видно: машина не управляется LAPS, \
             пароль ещё не сгенерирован, либо вашей учётной записи не делегировано \
             право чтения атрибутов LAPS (AD скрывает атрибуты без прав).",
            info.computer_name
        ))),
    }
}

/// Сброс/продление срока истечения пароля LAPS по DN объекта.
///
/// Возвращает свежую карточку компьютера (перечитанную после модификации),
/// чтобы UI показал актуальное состояние.
/// Текущее состояние объекта: какие атрибуты срока присутствуют.
/// После записи объект перечитывается: UI показывает фактическое состояние.
/// Объект исчез между операциями — крайне маловероятно, но контракт
/// требует карточку: возвращаем состояние «до».
pub async fn rotate_password(
    credentials: &DirectoryCredentials<'_>,
    dn: &str,
    mode: &RotationMode,
) -> Result<LapsInfo, AppError> {
    validate_dn(dn)?;

    let (mut ldap, _bind_ms) = open_session(credentials).await?;
    let url = credentials.ldap_url;

    let before = match search_base_computer(&mut ldap, url, dn).await {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let entry = before.into_iter().next().ok_or_else(|| {
        AppError::ComputerNotFound(format!("объект {dn} не найден или перемещён"))
    })?;
    let info_before = laps_info_from_entry(&entry, credentials.ldap_url).await;

    let mods = match build_rotation_mods(&info_before, mode) {
        Ok(mods) => mods,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };

    debug!(dn, mods = mods.len(), "LDAP: модификация срока истечения");
    let result = ldap.with_timeout(OPERATION_TIMEOUT).modify(dn, mods).await;
    let result = match result {
        Ok(result) => result,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(crate::services::ldap::classify_transport_error(url, &e));
        }
    };
    if result.rc != 0 {
        let _ = ldap.unbind().await;
        return Err(crate::services::ldap::classify_result(url, &result));
    }

    let after = match search_base_computer(&mut ldap, url, dn).await {
        Ok(entries) => entries,
        Err(e) => {
            let _ = ldap.unbind().await;
            return Err(e);
        }
    };
    let _ = ldap.unbind().await;

    let info = match after.into_iter().next() {
        Some(entry) => laps_info_from_entry(&entry, credentials.ldap_url).await,
        None => info_before,
    };
    info!(dn, mode = ?mode, "LAPS: срок истечения обновлён");
    Ok(info)
}

/// Диапазоны валидации DN объекта, приходящего с фронтенда.
fn validate_dn(dn: &str) -> Result<(), AppError> {
    let dn = dn.trim();
    if dn.is_empty() || dn.len() > 1024 {
        return Err(AppError::Validation(
            "Некорректный DN объекта компьютера".into(),
        ));
    }
    if dn.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(
            "DN объекта содержит недопустимые символы".into(),
        ));
    }
    let starts_with_known_rdn = ["CN=", "cn=", "OU=", "ou=", "DC=", "dc="]
        .iter()
        .any(|prefix| dn.starts_with(prefix));
    if !starts_with_known_rdn {
        return Err(AppError::Validation(
            "DN объекта компьютера должен начинаться с CN=".into(),
        ));
    }
    Ok(())
}

/// Base-поиск объекта компьютера по DN.
async fn search_base_computer(
    ldap: &mut ldap3::Ldap,
    url: &str,
    dn: &str,
) -> Result<Vec<NormalizedEntry>, AppError> {
    search(
        ldap,
        url,
        dn,
        Scope::Base,
        "(&(objectCategory=computer)(objectClass=computer))",
        COMPUTER_ATTRIBUTES.to_vec(),
    )
    .await
}

/// База поиска: Base DN из настроек или `defaultNamingContext` из rootDSE.
pub(crate) async fn resolve_base_dn(
    ldap: &mut ldap3::Ldap,
    credentials: &DirectoryCredentials<'_>,
) -> Result<String, AppError> {
    let configured = credentials.base_dn.trim();
    if !configured.is_empty() {
        return Ok(configured.to_string());
    }
    let root = read_root_dse(ldap, credentials.ldap_url).await;
    root.default_naming_context.ok_or_else(|| {
        AppError::Config(
            "Не удалось определить корень домена: Base DN в настройках пуст, \
             а rootDSE сервера не вернул defaultNamingContext. \
             Укажите Base DN вручную (например, DC=company,DC=ru)."
                .into(),
        )
    })
}

/// Диагностика подключения: bind, rootDSE, пробная выборка компьютеров.
/// `auth_mode` — метка режима, которым выполнялась проверка
/// (`integrated` / `account` / `anonymous`): вызывающий знает сессию.
pub async fn test_connection(
    credentials: &DirectoryCredentials<'_>,
    auth_mode: &str,
) -> Result<ConnectionTest, AppError> {
    let (mut ldap, bind_ms) = open_session(credentials).await?;

    let root = read_root_dse(&mut ldap, credentials.ldap_url).await;

    let _ = ldap.unbind().await;
    Ok(ConnectionTest {
        bind_ms,
        server_dns_name: root.server_dns_name,
        default_naming_context: root.default_naming_context,
        auth_mode: auth_mode.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_computer_dn() {
        assert!(validate_dn("CN=WS01,OU=Workstations,DC=company,DC=ru").is_ok());
        assert!(matches!(validate_dn(""), Err(AppError::Validation(_))));
        assert!(matches!(validate_dn("WS01"), Err(AppError::Validation(_))));
        assert!(matches!(
            validate_dn("CN=WS01\n,DC=x"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_dn(&("CN=x".to_string() + &"y".repeat(2000))),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn readable_flavors_pass_and_none_is_error() {
        let base = LapsInfo {
            dn: "CN=WS01,DC=x".into(),
            computer_name: "WS01".into(),
            dns_host_name: None,
            operating_system: None,
            flavor: model::LapsFlavor::WindowsPlain,
            account_name: None,
            password: Some("p".into()),
            updated_at: None,
            expires_at: None,
            legacy_present: false,
            windows_plain_present: true,
            encrypted_present: false,
            rotation_possible: true,
            decrypted: false,
            decrypt_method: None,
            win_expiration_present: true,
            legacy_expiration_present: false,
        };
        assert!(ensure_password_readable(&base).is_ok());

        let encrypted = LapsInfo {
            flavor: model::LapsFlavor::WindowsEncrypted,
            password: None,
            ..base.clone()
        };
        assert!(ensure_password_readable(&encrypted).is_ok());

        let none = LapsInfo {
            flavor: model::LapsFlavor::None,
            password: None,
            ..base
        };
        assert!(matches!(
            ensure_password_readable(&none),
            Err(AppError::NoLapsData(_))
        ));
    }
}

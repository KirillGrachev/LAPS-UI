//! Поиск объекта компьютера в каталоге и валидация пользовательского ввода.
//!
//! Поддерживаемые форматы запроса:
//! * NetBIOS-имя: `WS01`, `kma-w0001` (до 15 символов) — ищется по
//!   `sAMAccountName=WS01$` и `cn=WS01`;
//! * FQDN: `ws01.company.ru` — ищется по `dNSHostName`;
//! * IPv4/IPv6-адрес — сначала разрешается в имя обратным DNS-запросом
//!   (Windows: `GetNameInfoW`), затем ищется как имя/FQDN.
//!
//! Все значения перед подстановкой в фильтр экранируются
//! ([`crate::services::ldap::escape_filter_value`]) и дополнительно
//! проверяются на допустимый набор символов — защита от LDAP-инъекций.

use std::net::IpAddr;

use ldap3::Scope;
use tracing::debug;

use crate::error::AppError;
use crate::services::ldap::{escape_filter_value, search, DirectoryCredentials, NormalizedEntry};

/// Разобранный и проверенный поисковый запрос.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputerQuery {
    /// Короткое имя (NetBIOS): ищем по sAMAccountName/cn.
    NetBios(String),
    /// Полное доменное имя: ищем по dNSHostName.
    Fqdn(String),
}

/// Классифицирует и валидирует пользовательский ввод.
/// IP-адрес — в имя (обратный DNS), далее как обычный запрос.
pub fn parse_query(raw: &str) -> Result<ComputerQuery, AppError> {
    let query = raw.trim();
    if query.is_empty() {
        return Err(AppError::Validation(
            "Введите имя компьютера, FQDN или IP-адрес".into(),
        ));
    }
    if query.len() > 255 {
        return Err(AppError::Validation(
            "Запрос слишком длинный (максимум 255 символов)".into(),
        ));
    }

    if let Ok(ip) = query.parse::<IpAddr>() {
        let name = reverse_dns(&ip)?;
        debug!(ip = %ip, resolved = %name, "IP разрешён в имя обратным DNS");
        return classify_name(&name);
    }

    classify_name(query)
}

/// Убирает хвостовую точку FQDN и возможный суффикс `$` (копипаст из ADUC).
///
/// FQDN: метки из букв/цифр/дефисов, разделённые точками.
/// Четыре полностью числовых метки — вырожденный IPv4 (в т.ч.
/// невалидный, вроде 10.0.0.256): как имя хоста не принимаем.
/// NetBIOS-имя: A-Z, 0-9, дефис (AD также разрешает `_`, встречающийся
/// в именах некоторых типов объектов), длина до 15.
fn classify_name(name: &str) -> Result<ComputerQuery, AppError> {
    let lower = name.to_ascii_lowercase();
    let lower = lower.trim_end_matches('.').trim_end_matches('$');
    if lower.is_empty() {
        return Err(AppError::Validation("Пустое имя компьютера".into()));
    }

    if lower.contains('.') {
        let numeric_quad = lower.split('.').count() == 4
            && lower
                .split('.')
                .all(|label| !label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()));
        if numeric_quad {
            return Err(AppError::Validation(
                "Похоже на некорректный IPv4-адрес: октеты должны быть в диапазоне 0-255".into(),
            ));
        }
        let valid = lower.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        });
        if !valid {
            return Err(AppError::Validation(
                "Некорректное доменное имя: допускаются буквы, цифры, дефис и точки".into(),
            ));
        }
        return Ok(ComputerQuery::Fqdn(lower.to_string()));
    }

    let valid = lower.len() <= 15
        && lower
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(AppError::Validation(
            "Некорректное имя компьютера: допускаются A-Z, 0-9, дефис (до 15 символов). \
             Для полных имён используйте FQDN (ws01.company.ru)."
                .into(),
        ));
    }
    Ok(ComputerQuery::NetBios(lower.to_string()))
}

/// Строит LDAP-фильтр поиска компьютера по разобранный запрос.
///
/// Фильтр всегда ограничен objectClass/objectCategory=computer: объекты
/// пользователей или групп с похожими именами не должны попадать в выдачу
/// (иначе злоумышленник с правом чтения LAPS мог бы пытаться читать чужие
/// объекты, а инженер — видеть бессмысленные результаты).
/// Дополнительно ищем по cn короткого имени: у части машин
/// dNSHostName не заполнен, а имя в домене известно.
pub fn build_filter(query: &ComputerQuery) -> String {
    match query {
        ComputerQuery::NetBios(name) => {
            let escaped = escape_filter_value(name);
            format!(
                "(&(objectCategory=computer)(objectClass=computer)(|(sAMAccountName={escaped}$)(cn={escaped})))"
            )
        }
        ComputerQuery::Fqdn(fqdn) => {
            let escaped = escape_filter_value(fqdn);
            let short = escape_filter_value(fqdn.split('.').next().unwrap_or(fqdn));
            format!(
                "(&(objectCategory=computer)(objectClass=computer)(|(dNSHostName={escaped})(cn={short})))"
            )
        }
    }
}

/// Выполняет поиск компьютера; возвращает все подходящие записи.
pub async fn find_computers(
    ldap: &mut ldap3::Ldap,
    credentials: &DirectoryCredentials<'_>,
    base_dn: &str,
    query: &ComputerQuery,
    attributes: Vec<&str>,
) -> Result<Vec<NormalizedEntry>, AppError> {
    let filter = build_filter(query);
    debug!(base = base_dn, filter = %filter, "LDAP: поиск компьютера");
    search(
        ldap,
        credentials.ldap_url,
        base_dn,
        Scope::Subtree,
        &filter,
        attributes,
    )
    .await
}

/// Обратный DNS для IP-адреса.
///
/// Windows: `GetNameInfoW` из `ws2_32` (системный резолвер: учитывает
/// суффиксы DNS и настройки соединения). Вызов оформлен минимальным
/// прямым FFI: ABI функции стабилен со времён Windows XP и не зависит
/// от генерируемых биндингов crate `windows`, чьи типы неудобно
/// верифицировать в кроссплатформенной сборке.
///
/// Другие платформы: не поддерживается — приложение целевым образом
/// работает в доменной сети Windows.
///
/// # Safety
/// GetNameInfoW — стабильный Win32 API; передаём указатели на
/// локальные repr(C)-структуры корректных размеров и буфер hostlen
/// элементов. Функция не сохраняет указатели после возврата.
#[cfg(windows)]
pub fn reverse_dns(ip: &IpAddr) -> Result<String, AppError> {
    const AF_INET: u16 = 2;
    const AF_INET6: u16 = 23;
    const NI_NAMEREQD: i32 = 1;
    const NI_MAXHOST: usize = 1025;

    #[repr(C)]
    struct SockAddrIn {
        sin_family: u16,
        sin_port: u16,
        /// `s_addr` хранится в сетевом порядке байт: октеты `Ipv4Addr`
        /// уже в таком порядке — собираем `u32` нативным представлением.
        sin_addr: u32,
        sin_zero: [u8; 8],
    }

    #[repr(C)]
    struct SockAddrIn6 {
        sin6_family: u16,
        sin6_port: u16,
        sin6_flowinfo: u32,
        sin6_addr: [u8; 16],
        sin6_scope_id: u32,
    }

    #[link(name = "ws2_32")]
    extern "system" {
        fn GetNameInfoW(
            sa: *const u8,
            salen: i32,
            host: *mut u16,
            hostlen: u32,
            serv: *mut u16,
            servlen: u32,
            flags: i32,
        ) -> i32;
    }

    let mut host = [0u16; NI_MAXHOST];
    let rc = unsafe {
        match ip {
            IpAddr::V4(v4) => {
                let addr = SockAddrIn {
                    sin_family: AF_INET,
                    sin_port: 0,
                    sin_addr: u32::from_ne_bytes(v4.octets()),
                    sin_zero: [0; 8],
                };
                GetNameInfoW(
                    &addr as *const SockAddrIn as *const u8,
                    std::mem::size_of::<SockAddrIn>() as i32,
                    host.as_mut_ptr(),
                    NI_MAXHOST as u32,
                    std::ptr::null_mut(),
                    0,
                    NI_NAMEREQD,
                )
            }
            IpAddr::V6(v6) => {
                let addr = SockAddrIn6 {
                    sin6_family: AF_INET6,
                    sin6_port: 0,
                    sin6_flowinfo: 0,
                    sin6_addr: v6.octets(),
                    sin6_scope_id: 0,
                };
                GetNameInfoW(
                    &addr as *const SockAddrIn6 as *const u8,
                    std::mem::size_of::<SockAddrIn6>() as i32,
                    host.as_mut_ptr(),
                    NI_MAXHOST as u32,
                    std::ptr::null_mut(),
                    0,
                    NI_NAMEREQD,
                )
            }
        }
    };

    if rc != 0 {
        return Err(AppError::NotFound(format!(
            "не удалось разрешить IP-адрес {ip} в имя компьютера \
             (обратный DNS не отвечает, код {rc}). Введите имя компьютера вручную."
        )));
    }
    let len = host.iter().position(|&ch| ch == 0).unwrap_or(host.len());
    let name = String::from_utf16_lossy(&host[..len]);
    if name.trim().is_empty() {
        return Err(AppError::NotFound(format!(
            "обратный DNS вернул пустое имя для {ip}. Введите имя компьютера вручную."
        )));
    }
    Ok(name)
}

#[cfg(not(windows))]
pub fn reverse_dns(ip: &IpAddr) -> Result<String, AppError> {
    Err(AppError::Validation(format!(
        "поиск по IP-адресу ({ip}) поддерживается только в Windows-сборке: \
         введите имя компьютера вручную"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Хвостовой `$` из ADUC-копипаста допустим.
    fn classifies_netbios_names() {
        assert_eq!(
            parse_query("WS01").unwrap(),
            ComputerQuery::NetBios("ws01".into())
        );
        assert_eq!(
            parse_query("  kma-w0001  ").unwrap(),
            ComputerQuery::NetBios("kma-w0001".into())
        );
        assert_eq!(
            parse_query("WS01$").unwrap(),
            ComputerQuery::NetBios("ws01".into())
        );
    }

    #[test]
    fn classifies_fqdn() {
        assert_eq!(
            parse_query("ws01.company.ru").unwrap(),
            ComputerQuery::Fqdn("ws01.company.ru".into())
        );
        assert_eq!(
            parse_query("WS01.COMPANY.RU.").unwrap(),
            ComputerQuery::Fqdn("ws01.company.ru".into())
        );
    }

    #[test]
    /// Вырожденный IPv4 с октетом вне диапазона не должен
    /// проскакивать как FQDN из числовых меток.
    fn rejects_invalid_input() {
        for bad in [
            "",
            "   ",
            "очень-длинное-имя-компьютера-больше-пятнадцати",
            "ws 01",
            "ws*01",
            "ws01)(cn=*",
            "имя-в-юникоде",
            ".company.ru",
            "ws01..company.ru",
            "10.0.0.256",
        ] {
            assert!(
                matches!(parse_query(bad), Err(AppError::Validation(_))),
                "ожидался отказ для {bad:?}"
            );
        }
    }

    #[test]
    /// Даже если валидация когда-нибудь ослабнет, экранирование
    /// нейтрализует спецсимволы.
    fn filter_is_injection_safe() {
        let query = ComputerQuery::NetBios("ws01".into());
        let filter = build_filter(&query);
        assert_eq!(
            filter,
            "(&(objectCategory=computer)(objectClass=computer)(|(sAMAccountName=ws01$)(cn=ws01)))"
        );

        let hostile = ComputerQuery::Fqdn("a*b)(cn=*.company.ru".into());
        let filter = build_filter(&hostile);
        assert!(filter.contains("\\2a"), "{filter}");
        assert!(!filter.contains(")(cn=*."), "{filter}");
    }

    #[test]
    fn fqdn_filter_also_matches_short_cn() {
        let filter = build_filter(&ComputerQuery::Fqdn("ws01.company.ru".into()));
        assert!(filter.contains("(dNSHostName=ws01.company.ru)"));
        assert!(filter.contains("(cn=ws01)"));
    }
}

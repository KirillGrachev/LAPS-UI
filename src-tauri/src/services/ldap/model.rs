//! Типы LDAP-слоя: учётные данные подключения.

use std::time::Duration;

/// Таймаут установки соединения (и StartTLS-рукопожатия).
pub(super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Таймаут отдельной LDAP-операции (bind, поиск, modify, rootDSE).
/// `pub` — константа реэкспортируется из `ldap/mod.rs` для доменных модулей.
pub const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Параметры подключения к каталогу организации.
///
/// Пароль — заимствованная строка: секрет живёт в `Zeroizing<String>`
/// у вызывающего кода и не клонируется без необходимости.
#[derive(Debug, Clone)]
pub struct DirectoryCredentials<'a> {
    pub ldap_url: &'a str,
    pub base_dn: &'a str,
    pub bind_dn: Option<&'a str>,
    pub password: Option<&'a str>,
    pub use_start_tls: bool,
    pub allow_invalid_tls: bool,
    pub use_integrated_auth: bool,
}

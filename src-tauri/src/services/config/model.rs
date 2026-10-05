//! Модель конфигурации: подключение к AD, интерфейс, аудит.
//!
//! Пароль bind-учётки в модели не живёт: вместо него флаг `hasPassword` —
//! секрет остаётся в системном хранилище (см. [`super::store::ConfigStore`]).

use serde::{Deserialize, Serialize};

pub(super) fn default_theme() -> String {
    "system".to_string()
}

pub(super) fn default_language() -> String {
    "ru".to_string()
}

const fn default_true() -> bool {
    true
}

const fn default_clipboard_clear() -> u32 {
    30
}

/// Конфигурация приложения (одна организация / один домен).
///
/// Эта структура одновременно описывает контракт фронтенда: вместо пароля
/// в ней флаг `hasPassword` — секрет не покидает keyring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Тема интерфейса: `light` | `dark` | `system`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Язык интерфейса: `ru` | `en`.
    #[serde(default = "default_language")]
    pub language: String,

    /// Адрес контроллера домена: `ldap://dc1.company.ru` или `ldaps://...`.
    /// Пустая строка — автообнаружение через rootDSE невозможно, потребуется
    /// заполнить (приложение не выполняет DC Locator, как PS-модуль AD).
    #[serde(default)]
    pub ldap_url: String,
    /// Корень поиска: `DC=company,DC=ru`. Пустой — берётся
    /// `defaultNamingContext` из rootDSE.
    #[serde(default)]
    pub base_dn: String,
    /// StartTLS поверх `ldap://` (порт 389 + STARTTLS).
    #[serde(default)]
    pub use_start_tls: bool,
    /// Не проверять сертификат сервера (внутренние CA с самоподписанными
    /// сертификатами). Понижает безопасность — в UI помечается как опасная.
    #[serde(default)]
    pub allow_invalid_tls: bool,
    /// Локальный журнал аудита операций с паролями LAPS.
    #[serde(default = "default_true")]
    pub audit_enabled: bool,
    /// Автоочистка буфера обмена через N секунд после копирования пароля
    /// (0 — не очищать).
    #[serde(default = "default_clipboard_clear")]
    pub clipboard_clear_seconds: u32,
    /// Тестовый режим: доменные команды (вход, поиск, ротация, журнал)
    /// обслуживаются демо-данными на стороне фронтенда, реальные запросы
    /// к Active Directory не выполняются. Конфигурация и системные команды
    /// продолжают работать как обычно.
    #[serde(default)]
    pub test_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            ldap_url: String::new(),
            base_dn: String::new(),
            use_start_tls: false,
            allow_invalid_tls: false,
            audit_enabled: true,
            clipboard_clear_seconds: default_clipboard_clear(),
            test_mode: false,
        }
    }
}

/// Входные данные сохранения конфигурации (без пароля — он передаётся
/// отдельной командой `set_ldap_password`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigRequest {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub ldap_url: String,
    #[serde(default)]
    pub base_dn: String,
    #[serde(default)]
    pub use_start_tls: bool,
    #[serde(default)]
    pub allow_invalid_tls: bool,
    #[serde(default = "default_true")]
    pub audit_enabled: bool,
    #[serde(default = "default_clipboard_clear")]
    pub clipboard_clear_seconds: u32,
    #[serde(default)]
    pub test_mode: bool,
}

/// Результат диагностической команды «Проверить подключение».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTest {
    /// Время bind, мс.
    pub bind_ms: u64,
    /// `dnsHostName` из rootDSE.
    pub server_dns_name: Option<String>,
    /// `defaultNamingContext` из rootDSE — подсказка для Base DN.
    pub default_naming_context: Option<String>,
    /// Режим аутентификации, использованный при проверке:
    /// `integrated` | `account` | `anonymous`.
    pub auth_mode: String,
}

//! Сервисы приложения: конфигурация, нативный LDAP-клиент, доменная
//! логика LAPS, журнал аудита, предпроверка WebView2 Runtime.

pub mod audit;
pub mod config;
pub mod laps;
pub mod ldap;

/// Предпроверка WebView2 Runtime (только Windows): нативный диалог вместо
/// «окна-зомби», когда интерфейс создать нечем.
#[cfg(target_os = "windows")]
pub mod webview_prereq;

//! Системные команды: сведения об окружении, открытие внешних ссылок.

use serde::Serialize;

use crate::error::AppError;
use crate::services::audit::current_actor;

/// Сведения об окружении для вкладки «О программе» и статусной панели.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEnvironment {
    /// Версия приложения (из Cargo.toml).
    pub version: String,
    pub os: String,
    pub arch: String,
    /// Доступна ли интегрированная аутентификация текущей сессии
    /// (только Windows-сборки).
    pub integrated_auth_available: bool,
    /// Текущий пользователь Windows (`DOMAIN\user@MACHINE`) — подсказка
    /// инженеру: под какой учёткой идут запросы к AD в режиме SSO.
    pub windows_identity: String,
}

#[tauri::command]
pub fn app_environment() -> Result<AppEnvironment, AppError> {
    Ok(AppEnvironment {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        integrated_auth_available: cfg!(windows),
        windows_identity: current_actor(true, None),
    })
}

/// Схемы URL, которые разрешено открывать во внешней программе.
/// Всё остальное (file:, javascript:, произвольные exe-схемы) отклоняется.
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Открывает URL во внешней программе (браузер, почтовый клиент).
///
/// Реализовано через прямой запуск OS-хелпера **без командной оболочки**,
/// поэтому shell-инъекции через содержимое URL невозможны; схема и символы
/// дополнительно валидируются по allow-list.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), AppError> {
    validate_external_url(&url)?;

    #[cfg(target_os = "windows")]
    let (program, prefix): (&str, Option<&str>) = ("rundll32", Some("url.dll,FileProtocolHandler"));
    #[cfg(target_os = "macos")]
    let (program, prefix): (&str, Option<&str>) = ("open", None);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, prefix): (&str, Option<&str>) = ("xdg-open", None);

    let mut command = std::process::Command::new(program);
    if let Some(prefix) = prefix {
        command.arg(prefix);
    }
    command.arg(&url);

    command
        .spawn()
        .map_err(|e| AppError::Internal(format!("не удалось открыть ссылку ({program}): {e}")))?;
    Ok(())
}

fn validate_external_url(url: &str) -> Result<(), AppError> {
    const MAX_LEN: usize = 2048;

    if url.len() > MAX_LEN {
        return Err(AppError::Validation("Ссылка слишком длинная".into()));
    }
    if url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '"' || c == '\'')
    {
        return Err(AppError::Validation(
            "Ссылка содержит недопустимые символы".into(),
        ));
    }
    let scheme = url
        .split_once("://")
        .map(|(scheme, _)| scheme)
        .or_else(|| url.split_once(':').map(|(scheme, _)| scheme))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::Validation("Ссылка без схемы".into()))?;

    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return Err(AppError::Validation(format!(
            "Схема «{scheme}» не разрешена к открытию"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;
    use crate::error::AppError;

    #[test]
    fn allows_http_https_mailto() {
        assert!(validate_external_url("https://github.com").is_ok());
        assert!(validate_external_url("http://intranet.company.ru/laps").is_ok());
        assert!(validate_external_url("mailto:help@company.ru").is_ok());
    }

    #[test]
    fn rejects_dangerous_schemes_and_chars() {
        for bad in [
            "file:///C:/Windows/system32",
            "javascript:alert(1)",
            "https://ok.ru/a b",
            "https://ok.ru\"&calc.exe",
            "ftp://host",
            "no-scheme",
            &format!("https://{}", "x".repeat(3000)),
        ] {
            assert!(
                matches!(validate_external_url(bad), Err(AppError::Validation(_))),
                "ожидался отказ для {bad:?}"
            );
        }
    }
}

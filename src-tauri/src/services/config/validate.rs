//! Валидация и нормализация входных данных конфигурации.

use crate::error::AppError;

use super::model::SaveConfigRequest;

/// Приводит тему к одному из допустимых значений.
pub(super) fn normalize_theme(theme: &str) -> String {
    match theme.trim().to_ascii_lowercase().as_str() {
        "light" | "dark" => theme.trim().to_ascii_lowercase(),
        _ => "system".to_string(),
    }
}

/// Приводит язык к одному из поддерживаемых.
pub(super) fn normalize_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "en" => "en".to_string(),
        _ => "ru".to_string(),
    }
}

/// Автоочистка буфера: 0 (выключена) или 10..=300 секунд.
pub(super) fn normalize_clipboard_clear(seconds: u32) -> u32 {
    match seconds {
        0 => 0,
        s => s.clamp(10, 300),
    }
}

/// Проверяет URL LDAP-сервера: схема `ldap://`/`ldaps://`, непустой хост,
/// отсутствие пробелов и управляющих символов.
/// Пустой URL допустим до первого сохранения — но см. `ensure_connection_config`.
pub(super) fn validate_ldap_url(raw: &str) -> Result<String, AppError> {
    let url = raw.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return Ok(url);
    }
    if !(url.starts_with("ldap://") || url.starts_with("ldaps://")) {
        return Err(AppError::Validation(
            "Адрес сервера должен начинаться с ldap:// или ldaps:// (например, ldap://dc1.company.ru)".into(),
        ));
    }
    let host_port = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default();
    if host_port.is_empty() {
        return Err(AppError::Validation(
            "В адресе сервера не указано имя хоста".into(),
        ));
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(AppError::Validation(
            "Адрес сервера содержит недопустимые символы".into(),
        ));
    }
    Ok(url)
}

/// Проверяет Base DN: последовательность RDN `KEY=value`, разделённых запятыми.
/// Пустое значение допустимо (поиск от `defaultNamingContext`).
pub(super) fn validate_base_dn(raw: &str) -> Result<String, AppError> {
    let dn = raw.trim().to_string();
    if dn.is_empty() {
        return Ok(dn);
    }
    if dn.chars().any(|c| c.is_control()) {
        return Err(AppError::Validation(
            "Base DN содержит недопустимые символы".into(),
        ));
    }
    let valid = dn.split(',').all(|part| {
        let part = part.trim();
        match part.split_once('=') {
            Some((key, value)) => !key.trim().is_empty() && !value.trim().is_empty(),
            None => false,
        }
    });
    if !valid {
        return Err(AppError::Validation(
            "Base DN должен иметь вид DC=company,DC=ru или OU=...,DC=company,DC=ru".into(),
        ));
    }
    Ok(dn)
}

/// Полная валидация запроса сохранения.
pub(super) fn validate_request(request: SaveConfigRequest) -> Result<SaveConfigRequest, AppError> {
    Ok(SaveConfigRequest {
        theme: normalize_theme(&request.theme),
        language: normalize_language(&request.language),
        ldap_url: validate_ldap_url(&request.ldap_url)?,
        base_dn: validate_base_dn(&request.base_dn)?,
        use_start_tls: request.use_start_tls,
        allow_invalid_tls: request.allow_invalid_tls,
        audit_enabled: request.audit_enabled,
        clipboard_clear_seconds: normalize_clipboard_clear(request.clipboard_clear_seconds),
        test_mode: request.test_mode,
    })
}

/// Проверка, что конфигурация позволяет выполнить LDAP-операцию.
/// Учётные данные проверяются отдельно при входе (сессия).
pub fn ensure_connection_config(ldap_url: &str) -> Result<(), AppError> {
    if ldap_url.trim().is_empty() {
        return Err(AppError::Config(
            "Не указан адрес контроллера домена. Откройте «Настройки → Active Directory» \
             и заполните LDAP-адрес (например, ldap://dc1.company.ru)."
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_theme_and_language() {
        assert_eq!(normalize_theme(" Dark "), "dark");
        assert_eq!(normalize_theme("purple"), "system");
        assert_eq!(normalize_language("EN"), "en");
        assert_eq!(normalize_language("de"), "ru");
    }

    #[test]
    fn normalizes_clipboard_clear() {
        assert_eq!(normalize_clipboard_clear(0), 0);
        assert_eq!(normalize_clipboard_clear(5), 10);
        assert_eq!(normalize_clipboard_clear(30), 30);
        assert_eq!(normalize_clipboard_clear(9999), 300);
    }

    #[test]
    fn validates_ldap_urls() {
        assert_eq!(
            validate_ldap_url(" ldap://dc1.company.ru/ ").unwrap(),
            "ldap://dc1.company.ru"
        );
        assert_eq!(
            validate_ldap_url("ldaps://dc1.company.ru:636").unwrap(),
            "ldaps://dc1.company.ru:636"
        );
        assert!(validate_ldap_url("").is_ok());
        assert!(matches!(
            validate_ldap_url("dc1.company.ru"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_ldap_url("ldap://"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_ldap_url("ldap://dc1 company.ru"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn validates_base_dn() {
        assert_eq!(
            validate_base_dn(" DC=company,DC=ru ").unwrap(),
            "DC=company,DC=ru"
        );
        assert_eq!(
            validate_base_dn("OU=Workstations,DC=company,DC=ru").unwrap(),
            "OU=Workstations,DC=company,DC=ru"
        );
        assert!(validate_base_dn("").is_ok());
        assert!(matches!(
            validate_base_dn("company.ru"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validate_base_dn("DC="),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn connection_config_guards() {
        assert!(ensure_connection_config("ldap://dc1").is_ok());
        assert!(matches!(
            ensure_connection_config(""),
            Err(AppError::Config(_))
        ));
        assert!(matches!(
            ensure_connection_config("   "),
            Err(AppError::Config(_))
        ));
    }
}

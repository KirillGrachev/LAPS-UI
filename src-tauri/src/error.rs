//! Единая модель ошибок приложения.
//!
//! Ошибки пересекают IPC-границу в виде структуры `{ code, message }`:
//! * `code` — стабильный машиночитаемый идентификатор. Фронтенд использует его
//!   как ключ локализации (`errors.<CODE>`) и не парсит человекочитаемый текст.
//! * `message` — текст для пользователя (по-русски, продукт RU-first) с
//!   техническими подробностями, которые не жалко показать в тосте.
//!
//! Модель повторяет контракт `kmaruda-phonebook`, расширенный доменными
//! ошибками LAPS (компьютер не найден, нет данных LAPS, только шифрованный
//! пароль, отказано в записи срока истечения).

use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Config(String),

    #[error("{0}")]
    Validation(String),

    #[error("Сервер каталога недоступен: {0}")]
    LdapUnreachable(String),

    #[error("Превышено время ожидания ответа сервера каталога. Проверьте адрес сервера и доступность сети (VPN).")]
    LdapTimeout,

    #[error("Отказано в авторизации в каталоге: {0}")]
    LdapAuth(String),

    #[error("Ошибка защищённого соединения (TLS): {0}")]
    LdapTls(String),

    #[error("Ошибка протокола LDAP: {0}")]
    LdapProtocol(String),

    /// Объект компьютера не найден в каталоге.
    #[error("Компьютер не найден в Active Directory: {0}")]
    ComputerNotFound(String),

    /// Фильтру соответствует несколько объектов — требуется уточнение.
    #[error("Найдено несколько компьютеров, уточните имя: {0}")]
    ComputerAmbiguous(String),

    /// Компьютер найден, но LAPS-атрибуты пусты или не видны текущей учётной
    /// записи (AD молча скрывает атрибуты, на которые нет прав чтения).
    #[error("{0}")]
    NoLapsData(String),

    /// Недостаточно прав на операцию (чтение/запись атрибутов LAPS).
    #[error("{0}")]
    PermissionDenied(String),

    /// Операция требует входа: сессия не установлена или истекла.
    #[error("Требуется вход: выполните аутентификацию, чтобы продолжить")]
    NotAuthenticated,

    #[error("Журнал аудита: {0}")]
    Audit(String),

    #[error("{0}")]
    NotFound(String),

    #[error("Внутренняя ошибка: {0}")]
    Internal(String),
}

impl AppError {
    /// Стабильный код ошибки для фронтенда (i18n-ключ `errors.<CODE>`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "CONFIG_ERROR",
            Self::Validation(_) => "VALIDATION",
            Self::LdapUnreachable(_) => "LDAP_UNREACHABLE",
            Self::LdapTimeout => "LDAP_TIMEOUT",
            Self::LdapAuth(_) => "LDAP_AUTH",
            Self::LdapTls(_) => "LDAP_TLS",
            Self::LdapProtocol(_) => "LDAP_PROTOCOL",
            Self::ComputerNotFound(_) => "COMPUTER_NOT_FOUND",
            Self::ComputerAmbiguous(_) => "COMPUTER_AMBIGUOUS",
            Self::NoLapsData(_) => "NO_LAPS_DATA",
            Self::PermissionDenied(_) => "PERMISSION_DENIED",
            Self::NotAuthenticated => "NOT_AUTHENTICATED",
            Self::Audit(_) => "AUDIT_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Internal(_) => "INTERNAL",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Payload<'a> {
            code: &'static str,
            message: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            details: Option<&'a str>,
        }

        Payload {
            code: self.code(),
            message: self.to_string(),
            details: None,
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_stable_contract() {
        let err = AppError::NoLapsData("пароль LAPS не виден этой учётке".into());
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["code"], "NO_LAPS_DATA");
        assert!(json["message"].as_str().unwrap().contains("не виден"));
        assert!(json.get("details").is_none());
    }

    #[test]
    fn every_variant_has_unique_code() {
        let variants = [
            AppError::Config("x".into()),
            AppError::Validation("x".into()),
            AppError::LdapUnreachable("x".into()),
            AppError::LdapTimeout,
            AppError::LdapAuth("x".into()),
            AppError::LdapTls("x".into()),
            AppError::LdapProtocol("x".into()),
            AppError::ComputerNotFound("x".into()),
            AppError::ComputerAmbiguous("x".into()),
            AppError::NoLapsData("x".into()),
            AppError::PermissionDenied("x".into()),
            AppError::NotAuthenticated,
            AppError::Audit("x".into()),
            AppError::NotFound("x".into()),
            AppError::Internal("x".into()),
        ];
        let codes: Vec<&str> = variants.iter().map(AppError::code).collect();
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            codes.len(),
            unique.len(),
            "коды ошибок обязаны быть уникальны"
        );
    }
}

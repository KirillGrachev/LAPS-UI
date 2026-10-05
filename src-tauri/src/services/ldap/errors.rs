//! Классификация ошибок LDAP: отказ аутентификации AD, анонимный поиск,
//! сетевые сбои, отказ в доступе к атрибутам. Тексты сообщений — подсказки
//! пользователю (инженеру поддержки), а не сырые коды протокола.

use ldap3::LdapError;

use crate::error::AppError;

/// Коды результата LDAP-операций (RFC 4511 / MS-AD).
pub(super) const RC_OPERATIONS_ERROR: u32 = 1;
pub(super) const RC_INSUFFICIENT_ACCESS: u32 = 50;
pub(super) const RC_INVALID_CREDENTIALS: u32 = 49;
pub(super) const RC_UNAVAILABLE: u32 = 52;
pub(super) const RC_BUSY: u32 = 51;
pub(super) const RC_NO_SUCH_OBJECT: u32 = 32;

/// Классификация кода результата поисковой/модифицирующей операции.
///
/// Самый частый случай в боевых доменах: `rc=1` (operationsError) с текстом
/// «a successful bind must be completed» — сервер запрещает анонимный поиск.
/// Превращаем это в понятное указание заполнить учётную запись.
pub(super) fn classify_search_result(url: &str, result: &ldap3::LdapResult) -> AppError {
    let text = result.text.trim();
    match result.rc {
        0 => return AppError::Internal("не должно вызываться при rc=0".into()),
        RC_INSUFFICIENT_ACCESS => {
            return AppError::PermissionDenied(format!(
                "недостаточно прав для операции на {url}: {text}. \
                 Попросите администратора домена делегировать вашей учётной записи \
                 соответствующие права (чтение атрибутов LAPS или «Reset password expiration»)."
            ))
        }
        RC_NO_SUCH_OBJECT => {
            return AppError::NotFound(format!("объект не найден в каталоге {url}: {text}"))
        }
        RC_UNAVAILABLE | RC_BUSY => {
            return AppError::LdapUnreachable(format!(
                "{url} — контроллер домена недоступен или перегружен (код {}): {text}",
                result.rc
            ))
        }
        _ => {}
    }

    let mentions_bind = text.contains("bind") || text.contains("DSID");
    if result.rc == RC_OPERATIONS_ERROR && mentions_bind {
        return AppError::LdapAuth(format!(
            "сервер {url} отклонил анонимный поиск: домен требует аутентификацию. \
             Включите интегрированную аутентификацию или заполните «Учётная запись» и «Пароль» \
             в настройках Active Directory. Подробности: {text}"
        ));
    }
    AppError::LdapProtocol(format!(
        "операция завершилась с кодом {} ({}), сервер: {url}",
        result.rc, text
    ))
}

pub(super) fn classify_error(url: &str, error: &LdapError) -> AppError {
    match error {
        LdapError::Timeout { .. } => AppError::LdapTimeout,
        LdapError::Io { source } => classify_io_error(url, source),
        LdapError::UnknownScheme(_) | LdapError::UrlParsing { .. } => {
            AppError::Validation(format!("Некорректный адрес LDAP-сервера: {url}"))
        }
        LdapError::FilterParsing => AppError::Validation("Некорректный фильтр поиска LDAP".into()),
        other => {
            let text = other.to_string();
            let lower = text.to_lowercase();
            if lower.contains("tls") || lower.contains("certificate") || lower.contains("ssl") {
                AppError::LdapTls(text)
            } else {
                AppError::LdapProtocol(text)
            }
        }
    }
}

fn classify_io_error(url: &str, io: &std::io::Error) -> AppError {
    let text = io.to_string().to_lowercase();
    if text.contains("certificate") || text.contains("ssl") || text.contains("tls") {
        return AppError::LdapTls(io.to_string());
    }
    use std::io::ErrorKind;
    match io.kind() {
        ErrorKind::TimedOut => AppError::LdapTimeout,
        ErrorKind::ConnectionRefused => AppError::LdapUnreachable(format!(
            "{url} — подключение отклонено (проверьте адрес и порт: 389 для ldap://, 636 для ldaps://)"
        )),
        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::BrokenPipe => {
            AppError::LdapUnreachable(format!("{url} — соединение разорвано сервером"))
        }
        _ => AppError::LdapUnreachable(format!("{url} — {io}")),
    }
}

/// Человекочитаемая расшифровка отказа аутентификации AD.
///
/// AD возвращает в тексте ошибки шестнадцатеричный код причины
/// (`... data 52e, v4f7c`), который мы переводим в понятное сообщение.
pub fn auth_error_text(rc: u32, text: &str) -> String {
    if rc != RC_INVALID_CREDENTIALS {
        return format!("код {rc}: {}", text.trim());
    }
    let reason = match extract_ad_data_code(text).as_deref() {
        Some("525") => "пользователь не найден",
        Some("52e") => "неверный логин или пароль",
        Some("530") => "вход с текущей станции запрещён политикой",
        Some("531") => "вход в это время запрещён политикой",
        Some("532") => "срок действия пароля истёк",
        Some("533") => "учётная запись отключена",
        Some("701") => "срок действия учётной записи истёк",
        Some("773") => "требуется смена пароля",
        Some("775") => "учётная запись заблокирована (превышено число попыток входа)",
        _ => return format!("отказано в аутентификации: {}", text.trim()),
    };
    reason.to_string()
}

fn extract_ad_data_code(text: &str) -> Option<String> {
    let index = text.find("data ")?;
    let rest = &text[index + "data ".len()..];
    let code: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if code.is_empty() {
        None
    } else {
        Some(code.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Не-49 коды проходят через общий формат.
    fn explains_ad_auth_failures() {
        let text = "80090308: LdapErrText: AcceptSecurityContext error, data 52e, v4f7c";
        assert_eq!(auth_error_text(49, text), "неверный логин или пароль");

        let locked = "...data 775...";
        assert!(auth_error_text(49, locked).contains("заблокирована"));

        assert!(auth_error_text(50, "insufficient access").contains("50"));
    }

    #[test]
    fn extracts_hex_data_code() {
        assert_eq!(
            extract_ad_data_code("data 52e, v4f7c").as_deref(),
            Some("52e")
        );
        assert_eq!(extract_ad_data_code("no code here"), None);
    }

    #[test]
    fn classifies_anonymous_search_rejection() {
        let result = ldap3::LdapResult {
            rc: 1,
            matched: String::new(),
            text: "000004DC: LdapErr: DSID-0C090D21, comment: In order to perform this operation, \
                   a successful bind must be completed on the connection., data 0, v4f7c"
                .into(),
            refs: vec![],
            ctrls: vec![],
        };
        let err = classify_search_result("ldap://dc1", &result);
        match err {
            AppError::LdapAuth(message) => {
                assert!(message.contains("аутентификацию"), "{message}");
                assert!(message.contains("Пароль"), "{message}");
            }
            other => panic!("ожидался LdapAuth, получен {other:?}"),
        }
    }

    #[test]
    fn classifies_insufficient_access_with_guidance() {
        let result = ldap3::LdapResult {
            rc: 50,
            matched: String::new(),
            text: "00002098: SecErr: DSID-03150BB9, problem 4003".into(),
            refs: vec![],
            ctrls: vec![],
        };
        match classify_search_result("ldap://dc1", &result) {
            AppError::PermissionDenied(message) => {
                assert!(message.contains("делегировать"), "{message}");
            }
            other => panic!("ожидался PermissionDenied, получен {other:?}"),
        }
    }
}

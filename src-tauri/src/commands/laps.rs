//! Команды LAPS: поиск компьютера с паролем, сброс/продление срока,
//! отметка о копировании пароля.
//!
//! Каждая операция журналируется **здесь**, до возврата результата во
//! фронтенд: журнал аудита ведёт бэкенд, а не UI (его нельзя обойти
//! манипуляциями в интерфейсе).

use tauri::State;

use crate::error::AppError;
use crate::services::audit::{AuditAction, AuditLogger};
use crate::services::laps::{LapsInfo, RotationMode};
use crate::state::AppState;

use super::{write_audit, AdSession};

/// Поиск компьютера и чтение пароля LAPS.
///
/// `query` — имя (WS01), FQDN (ws01.company.ru) или IP-адрес.
#[tauri::command(rename_all = "camelCase")]
pub async fn search_computer(
    state: State<'_, AppState>,
    query: String,
) -> Result<LapsInfo, AppError> {
    let session = AdSession::gated(&state).await?;

    let actor = session.actor.clone();
    let audit_enabled = session.audit_enabled();
    let credentials = session.credentials();
    let result = crate::services::laps::search_computer(&credentials, &query).await;

    match &result {
        Ok(info) => {
            let mut event =
                AuditLogger::event(actor, AuditAction::Search, info.computer_name.clone(), "OK");
            event.flavor = Some(info.flavor.audit_label().to_string());
            if info.decrypted {
                event.details = Some(format!(
                    "decrypted={}",
                    info.decrypt_method.as_deref().unwrap_or("unknown")
                ));
            } else if matches!(
                info.flavor,
                crate::services::laps::LapsFlavor::WindowsEncrypted
            ) {
                event.details = Some("decrypt=unavailable".to_string());
            }
            write_audit(&state, audit_enabled, event).await;
        }
        Err(e) => {
            let mut event = AuditLogger::event(
                actor,
                AuditAction::SearchFailed,
                query.trim().to_string(),
                e.code(),
            );
            event.details = Some(truncate_details(&e.to_string()));
            write_audit(&state, audit_enabled, event).await;
        }
    }
    result
}

/// Сброс («сменить сейчас») или продление срока истечения пароля LAPS.
///
/// * `dn` — DN объекта компьютера из результата `search_computer`;
/// * `mode` — `"now"` (удалить срок → агент сгенерирует новый пароль при
///   следующем применении политики) или `"at"` (записать конкретную дату);
/// * `localDateTime` — для режима `"at"`: наивная локальная строка
///   `YYYY-MM-DDTHH:MM[:SS]` из datetime-пикера.
#[tauri::command(rename_all = "camelCase")]
pub async fn rotate_password(
    state: State<'_, AppState>,
    dn: String,
    mode: String,
    local_date_time: Option<String>,
) -> Result<LapsInfo, AppError> {
    let rotation = match mode.trim().to_ascii_lowercase().as_str() {
        "now" => RotationMode::Now,
        "at" => RotationMode::At(local_date_time.unwrap_or_default()),
        other => {
            return Err(AppError::Validation(format!(
                "Неизвестный режим сброса срока: {other:?} (ожидается \"now\" или \"at\")"
            )))
        }
    };
    if matches!(rotation, RotationMode::At(ref value) if value.trim().is_empty()) {
        return Err(AppError::Validation(
            "Для режима «указать дату» не передано значение даты".into(),
        ));
    }

    let session = AdSession::gated(&state).await?;

    let actor = session.actor.clone();
    let audit_enabled = session.audit_enabled();
    let credentials = session.credentials();
    let result = crate::services::laps::rotate_password(&credentials, &dn, &rotation).await;

    let target = match &result {
        Ok(info) => info.computer_name.clone(),
        Err(_) => dn.clone(),
    };
    match &result {
        Ok(info) => {
            let mut event = AuditLogger::event(actor, AuditAction::Rotate, target, "OK");
            event.flavor = Some(info.flavor.audit_label().to_string());
            event.details = Some(match &rotation {
                RotationMode::Now => "reset-now".to_string(),
                RotationMode::At(value) => format!("set:{value}"),
            });
            write_audit(&state, audit_enabled, event).await;
        }
        Err(e) => {
            let mut event = AuditLogger::event(actor, AuditAction::RotateFailed, target, e.code());
            event.details = Some(truncate_details(&e.to_string()));
            write_audit(&state, audit_enabled, event).await;
        }
    }
    result
}

/// Отметка о копировании пароля в буфер обмена. Копирование выполняет
/// фронтенд (clipboard API webview), но событие пишется в общий журнал —
/// вызов этой команды обязателен для полноты аудита.
#[tauri::command(rename_all = "camelCase")]
pub async fn note_password_copied(
    state: State<'_, AppState>,
    computer_name: String,
) -> Result<(), AppError> {
    let session = AdSession::gated(&state).await?;
    write_audit(
        &state,
        session.audit_enabled(),
        AuditLogger::event(
            session.actor.clone(),
            AuditAction::Copy,
            computer_name,
            "OK",
        ),
    )
    .await;
    Ok(())
}

/// Подробности для журнала: одна строка ограниченной длины
/// (тексты LDAP-ошибок бывают многосотнезнаковыми).
fn truncate_details(message: &str) -> String {
    const MAX_LEN: usize = 300;
    let single_line: String = message.chars().filter(|c| !c.is_control()).collect();
    if single_line.chars().count() <= MAX_LEN {
        single_line
    } else {
        single_line.chars().take(MAX_LEN).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_details;

    #[test]
    fn truncates_long_details_to_single_line() {
        let long = "ошибка ".repeat(100);
        let truncated = truncate_details(&long);
        assert!(truncated.chars().count() <= 301);
        assert!(!truncated.contains('\n'));
        assert!(truncate_details("коротко\nмногострочно").contains("коротко"));
        assert!(!truncate_details("коротко\nмногострочно").contains('\n'));
    }
}

//! Команды конфигурации и диагностики подключения.

use tauri::State;
use tracing::info;

use crate::error::AppError;
use crate::services::audit::{current_actor, AuditAction, AuditLogger};
use crate::services::config::{AppConfig, ConfigStore, ConnectionTest, SaveConfigRequest};
use crate::state::AppState;

use super::{config_dir, run_blocking, write_audit, AdSession};

/// Загрузка конфигурации. Учётных данных в конфигурации нет вовсе —
/// вход выполняется при запуске и живёт только в памяти процесса.
#[tauri::command]
pub async fn load_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    let config_dir = config_dir(&state).to_path_buf();
    run_blocking(move || ConfigStore::load(&config_dir)).await
}

/// Сохранение конфигурации (автосохранение из настроек). Возвращает
/// итоговую конфигурацию, чтобы фронтенд синхронизировал нормализованные
/// значения (trim, темы, флаги).
/// Субъект аудита — живая сессия, а до входа — идентификатор будущей аутентификации.
#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    config: SaveConfigRequest,
) -> Result<AppConfig, AppError> {
    let config_dir = config_dir(&state).to_path_buf();
    let audit_enabled = config.audit_enabled;
    let saved = run_blocking(move || ConfigStore::save(&config_dir, config)).await?;

    info!("конфигурация сохранена");
    let actor = {
        let guard = super::session_guard(&state);
        match guard.as_ref() {
            Some(session) => session.actor.clone(),
            None => current_actor(true, None),
        }
    };
    write_audit(
        &state,
        audit_enabled,
        AuditLogger::event(
            actor,
            AuditAction::ConfigSaved,
            saved.ldap_url.clone(),
            "OK",
        ),
    )
    .await;
    Ok(saved)
}

/// Диагностика подключения: bind (сессия / SSO / анонимно), rootDSE,
/// пробная выборка компьютеров. Доступна до входа — администратор может
/// настроить подключение до первой аутентификации; режим помечается
/// в результате (`authMode`).
#[tauri::command]
pub async fn test_ldap_connection(state: State<'_, AppState>) -> Result<ConnectionTest, AppError> {
    let session = AdSession::ungated(&state).await?;

    let auth_mode = session.auth_mode().to_string();
    let credentials = session.credentials();
    let result = crate::services::laps::test_connection(&credentials, &auth_mode).await;

    let (code, details) = match &result {
        Ok(test) => (
            "OK".to_string(),
            format!(
                "bind {} мс ({}), сервер: {}",
                test.bind_ms,
                auth_mode,
                test.server_dns_name.as_deref().unwrap_or("?")
            ),
        ),
        Err(e) => (e.code().to_string(), truncate_for_log(&e.to_string())),
    };
    let mut event = AuditLogger::event(
        session.actor.clone(),
        AuditAction::ConnectionTest,
        session.config.ldap_url.clone(),
        code,
    );
    event.details = Some(details);
    write_audit(&state, session.audit_enabled(), event).await;

    result
}

/// Однострочная выжимка сообщения для журнала.
fn truncate_for_log(message: &str) -> String {
    const MAX_LEN: usize = 200;
    let single_line: String = message.chars().filter(|c| !c.is_control()).collect();
    if single_line.chars().count() <= MAX_LEN {
        single_line
    } else {
        single_line.chars().take(MAX_LEN).collect::<String>() + "…"
    }
}

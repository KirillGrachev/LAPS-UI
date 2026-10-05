//! Команды журнала аудита: чтение страницы и открытие папки журнала.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::AppError;
use crate::services::audit::AuditPage;
use crate::state::AppState;

use super::run_blocking;

/// Страница журнала (новые записи сверху).
#[tauri::command]
pub async fn list_audit(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<AuditPage, AppError> {
    let logger = state.audit.clone();
    run_blocking(move || logger.list(limit, offset)).await
}

/// Показывает файл журнала в системном проводнике.
#[tauri::command]
pub async fn open_audit_folder(app: AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let path = state.audit.log_path();
    if !path.exists() {
        return Err(AppError::NotFound(
            "Журнал аудита ещё не создан: выполните хотя бы одну операцию поиска".into(),
        ));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::Internal(format!("не удалось открыть папку журнала: {e}")))
}

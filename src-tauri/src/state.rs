//! Общее управляемое состояние приложения.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

use crate::error::AppError;
use crate::services::audit::AuditLogger;

/// Сессия входа: живёт только в памяти процесса, не сохраняется на диск
/// и не переживает перезапуск приложения (требование безопасности:
/// аутентификация при каждом входе, учётные данные не запоминаются).
pub struct Session {
    /// Человекочитаемый субъект для журнала аудита.
    pub actor: String,
    /// `true` — интегрированная аутентификация (SSPI текущей сессии Windows);
    /// `false` — явная учётная запись, введённая при входе.
    pub integrated: bool,
    /// Учётная запись явного входа (bind identifier), если режим не SSO.
    pub account: Option<String>,
    /// Пароль явного входа. Хранится только в памяти, обёрнут в `Zeroizing`
    /// (память затирается при удалении), никогда не пишется в конфиг/журналы.
    pub password: Option<Zeroizing<String>>,
}

/// Общее состояние, доступное командам через `State<'_, AppState>`.
///
/// Приложение не держит пул LDAP-соединений: каждая операция открывает
/// собственное соединение с аутентификацией текущей сессии (SSPI не
/// поддерживает повторное использование чужих контекстов между
/// соединениями без кеширования кредов), поэтому в состоянии живут
/// только пути, журнал и сессия входа.
#[derive(Clone)]
pub struct AppState {
    /// Папка данных приложения: `config.json` и `audit.log`.
    pub config_dir: PathBuf,
    /// Журнал аудита операций с паролями LAPS.
    pub audit: AuditLogger,
    /// Сессия входа (`None` до успешной аутентификации).
    pub session: Arc<Mutex<Option<Session>>>,
}

impl AppState {
    /// Подготовка состояния: папка данных и журнал аудита.
    ///
    /// Журнал — «схема Minecraft»: прошлый запуск уходит в gzip-архив,
    /// новый запуск начинает чистый живой файл.
    pub fn init(app: &AppHandle) -> Result<Self, AppError> {
        let config_dir = app.path().app_data_dir().map_err(|e| {
            AppError::Config(format!(
                "не удалось определить папку данных приложения: {e}"
            ))
        })?;
        let audit = AuditLogger::new(config_dir.clone());
        audit.archive_previous_run();
        Ok(Self {
            audit,
            config_dir,
            session: Arc::new(Mutex::new(None)),
        })
    }
}

//! Реестр IPC-команд и общие вспомогательные механизмы.

pub mod audit;
pub mod auth;
pub mod config;
pub mod laps;
pub mod system;

use std::path::Path;

use zeroize::Zeroizing;

use crate::error::AppError;
use crate::services::config::{ensure_connection_config, AppConfig, ConfigStore};
use crate::services::ldap::DirectoryCredentials;
use crate::state::{AppState, Session};

/// Выполнить блокирующую работу вне главного потока.
///
/// В Tauri v2 **синхронные** команды исполняются на главном потоке окна
/// (цикл событий окна/WebView). Чтение конфигурации и запись журнала на
/// таком потоке подвесили бы UI. Поэтому все блокирующие операции обёрнуты
/// в `spawn_blocking`: главный поток лишь ждёт готовый результат в
/// async-задаче, окно остаётся отзывчивым.
pub(crate) async fn run_blocking<F, T>(job: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| AppError::Internal(format!("блокирующая задача завершилась аварией: {e}")))?
}

/// Готовая к работе с AD сессия: конфигурация + учётные данные входа.
///
/// Источник учётных данных — живая сессия входа ([`Session`], только в
/// памяти процесса): для SSO это флаг интегрированной аутентификации,
/// для явного входа — учётная запись и пароль, введённые при запуске
/// (пароль обёрнут в `Zeroizing`, живёт до выхода из приложения).
/// До входа команды, обращающиеся к AD, отклоняются с
/// [`AppError::NotAuthenticated`].
pub(crate) struct AdSession {
    pub config: AppConfig,
    /// Субъект для журнала аудита.
    pub actor: String,
    pub integrated: bool,
    pub account: Option<String>,
    pub password: Option<Zeroizing<String>>,
}

impl AdSession {
    /// Сессия для команд, требующих входа (поиск, ротация, аудит-копия).
    pub async fn gated(state: &AppState) -> Result<Self, AppError> {
        Self::build(state, true).await
    }

    /// Сессия для команд, доступных до входа (проверка подключения):
    /// учётные данные берём из живой сессии, если она есть, иначе SSO
    /// или анонимный режим — результат честен и помечается `auth_mode`.
    pub async fn ungated(state: &AppState) -> Result<Self, AppError> {
        Self::build(state, false).await
    }

    /// Сборка сессии: конфигурация и учётные данные живого входа.
    ///
    /// До входа доступен только анонимный режим (или SSO внутри живой
    /// сессии). Конфигурация больше не хранит режим аутентификации —
    /// его выбирает пользователь на экране входа.
    async fn build(state: &AppState, gated: bool) -> Result<Self, AppError> {
        let config_dir = state.config_dir.clone();
        let config = run_blocking(move || ConfigStore::load(&config_dir)).await?;
        ensure_connection_config(&config.ldap_url)?;

        let guard = state.session.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(session) => Ok(Self {
                actor: session.actor.clone(),
                integrated: session.integrated,
                account: session.account.clone(),
                password: session
                    .password
                    .as_ref()
                    .map(|p| Zeroizing::new(p.to_string())),
                config,
            }),
            None if gated => Err(AppError::NotAuthenticated),
            None => Ok(Self {
                actor: crate::services::audit::current_actor(true, None),
                integrated: false,
                account: None,
                password: None,
                config,
            }),
        }
    }

    /// Учётные данные для LDAP-подключения (заимствуют поля сессии).
    ///
    /// Источник правды — сессия входа, а не черновик конфигурации:
    /// смена режима в настройках не должна ронять текущую сессию.
    pub fn credentials(&self) -> DirectoryCredentials<'_> {
        DirectoryCredentials {
            ldap_url: &self.config.ldap_url,
            base_dn: &self.config.base_dn,
            bind_dn: self.account.as_deref(),
            password: self.password.as_deref().map(|s| s.as_str()),
            use_start_tls: self.config.use_start_tls,
            allow_invalid_tls: self.config.allow_invalid_tls,
            use_integrated_auth: self.integrated,
        }
    }

    /// Метка режима аутентификации для диагностики (`ConnectionTest`).
    pub fn auth_mode(&self) -> &'static str {
        if self.integrated {
            "integrated"
        } else if self.account.is_some() {
            "account"
        } else {
            "anonymous"
        }
    }

    /// Включён ли локальный аудит.
    pub fn audit_enabled(&self) -> bool {
        self.config.audit_enabled
    }
}

/// Журналирование события (если аудит включён). Ошибки записи глушатся:
/// журнал — вспомогательный механизм, он не должен ломать основную операцию.
pub(crate) async fn write_audit(
    state: &AppState,
    enabled: bool,
    event: crate::services::audit::AuditEvent,
) {
    if !enabled {
        return;
    }
    let logger = state.audit.clone();
    let _ = run_blocking(move || {
        logger.append(event);
        Ok(())
    })
    .await;
}

/// Чтение guard'а сессии без паники на отравленном мьютексе.
pub(crate) fn session_guard(state: &AppState) -> std::sync::MutexGuard<'_, Option<Session>> {
    state.session.lock().unwrap_or_else(|e| e.into_inner())
}

/// Путь к папке данных (для команд, не нуждающихся в сессии).
pub(crate) fn config_dir(state: &AppState) -> &Path {
    &state.config_dir
}

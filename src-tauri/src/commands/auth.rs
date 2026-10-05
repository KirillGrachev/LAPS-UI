//! Команды входа и выхода: аутентификация при запуске приложения.
//!
//! Модель безопасности второй версии («как в старом LAPS», но нативно):
//! * вход выполняется **при каждом запуске**: учётная запись и пароль
//!   вводятся на экране входа и проверяются настоящим LDAP-bind;
//! * опционально — вход одной кнопкой под текущей сессией Windows
//!   (SASL GSSAPI/SSPI): для этого случая поля не нужны;
//! * учётные данные нигде не сохраняются: пароль живёт только в памяти
//!   процесса (`Zeroizing`) до выхода или закрытия приложения;
//! * поля ввода не содержат подсказок (placeholder) и отключают
//!   автозаполнение WebView.

use serde::Serialize;
use tauri::State;
use tracing::info;
use zeroize::Zeroizing;

use crate::error::AppError;
use crate::services::audit::{current_actor, AuditAction, AuditLogger};
use crate::services::config::{ensure_connection_config, ConfigStore};
use crate::services::ldap::{open_session, DirectoryCredentials};
use crate::state::{AppState, Session};

use super::{config_dir, run_blocking, session_guard, write_audit};

/// Снимок сессии для фронтенда (без секрета).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub actor: String,
    pub integrated: bool,
}

/// Аутентификация при входе в приложение.
///
/// * `sso = true`: SASL GSSAPI bind под текущей сессией Windows
///   (только Windows-сборки); аргументы учётки игнорируются;
/// * иначе: обязательные `account` (DN / UPN / `DOMAIN\user`) и `password`,
///   простой bind; секрет остаётся только в памяти процесса.
#[tauri::command(rename_all = "camelCase")]
pub async fn login(
    state: State<'_, AppState>,
    account: Option<String>,
    password: Option<String>,
    sso: Option<bool>,
) -> Result<SessionInfo, AppError> {
    let config_dir = config_dir(&state).to_path_buf();
    let config = run_blocking(move || ConfigStore::load(&config_dir)).await?;
    ensure_connection_config(&config.ldap_url)?;

    let base = DirectoryCredentials {
        ldap_url: &config.ldap_url,
        base_dn: &config.base_dn,
        bind_dn: None,
        password: None,
        use_start_tls: config.use_start_tls,
        allow_invalid_tls: config.allow_invalid_tls,
        use_integrated_auth: false,
    };

    let (actor, integrated, account_kept, secret) = if sso.unwrap_or(false) {
        #[cfg(windows)]
        {
            let creds = DirectoryCredentials {
                use_integrated_auth: true,
                ..base
            };
            let (mut ldap, _bind_ms) = open_session(&creds).await?;
            let _ = ldap.unbind().await;
            (current_actor(true, None), true, None, None)
        }
        #[cfg(not(windows))]
        {
            let _ = base;
            return Err(AppError::Validation(
                "Вход под сессией Windows доступен только в Windows-сборке".into(),
            ));
        }
    } else {
        let account = account
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Validation("Укажите учётную запись".into()))?;
        let password = password
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Validation("Укажите пароль".into()))?;

        let creds = DirectoryCredentials {
            bind_dn: Some(&account),
            password: Some(&password),
            ..base
        };
        let (mut ldap, _bind_ms) = open_session(&creds).await?;
        let _ = ldap.unbind().await;

        let actor = match std::env::var("COMPUTERNAME").ok().filter(|v| !v.is_empty()) {
            Some(machine) => format!("{account}@{machine}"),
            None => account.clone(),
        };
        (actor, false, Some(account), Some(Zeroizing::new(password)))
    };

    {
        let mut guard = session_guard(&state);
        *guard = Some(Session {
            actor: actor.clone(),
            integrated,
            account: account_kept,
            password: secret,
        });
    }

    info!(actor = %actor, integrated, "вход выполнен");
    write_audit(
        &state,
        config.audit_enabled,
        AuditLogger::event(actor.clone(), AuditAction::Login, actor.clone(), "OK"),
    )
    .await;

    Ok(SessionInfo { actor, integrated })
}

/// Выход: сессия и пароль стираются из памяти немедленно.
#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), AppError> {
    let taken = {
        let mut guard = session_guard(&state);
        guard.take()
    };
    if let Some(session) = taken {
        let config_dir = config_dir(&state).to_path_buf();
        let audit_enabled = run_blocking(move || ConfigStore::load(&config_dir))
            .await
            .map(|config| config.audit_enabled)
            .unwrap_or(true);
        info!(actor = %session.actor, "выход выполнен");
        write_audit(
            &state,
            audit_enabled,
            AuditLogger::event(
                session.actor.clone(),
                AuditAction::Logout,
                session.actor,
                "OK",
            ),
        )
        .await;
    }
    Ok(())
}

/// Текущая сессия входа (`null` — вход не выполнен). Без аудита: опрос
/// статуса не является операцией с данными.
#[tauri::command]
pub async fn session_status(state: State<'_, AppState>) -> Result<Option<SessionInfo>, AppError> {
    let guard = session_guard(&state);
    Ok(guard.as_ref().map(|session| SessionInfo {
        actor: session.actor.clone(),
        integrated: session.integrated,
    }))
}

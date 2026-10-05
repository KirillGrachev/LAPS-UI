//! Хранилище конфигурации: атомарная запись `config.json`.
//!
//! * `config.json` пишется атомарно (temp-файл + rename) — повреждение
//!   невозможно даже при обрыве питания;
//! * учётные данные в конфигурации НЕ хранятся вовсе: вход выполняется
//!   при каждом запуске, пароль живёт только в памяти сессии процесса;
//! * повреждённый конфиг не роняет приложение: делается резервная копия
//!   `.corrupt` и используются значения по умолчанию.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::error::AppError;

use super::model::{AppConfig, SaveConfigRequest};
use super::validate::validate_request;

const CONFIG_FILE: &str = "config.json";

pub struct ConfigStore;

impl ConfigStore {
    /// Путь к `config.json` внутри папки данных приложения.
    pub fn config_path(dir: &Path) -> PathBuf {
        dir.join(CONFIG_FILE)
    }

    /// Загрузка конфигурации. Отсутствующий файл → значения по умолчанию,
    /// повреждённый → бэкап `.corrupt` и значения по умолчанию.
    pub fn load(dir: &Path) -> Result<AppConfig, AppError> {
        let path = Self::config_path(dir);
        let config = Self::load_from_disk(&path);
        Ok(config)
    }

    pub(super) fn load_from_disk(path: &Path) -> AppConfig {
        let Ok(content) = fs::read_to_string(path) else {
            return AppConfig::default();
        };
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(e) => {
                warn!(error = %e, "config.json повреждён, делаю резервную копию и использую значения по умолчанию");
                let backup = path.with_extension("json.corrupt");
                let _ = fs::rename(path, &backup);
                AppConfig::default()
            }
        }
    }

    /// Валидация и атомарное сохранение конфигурации.
    /// Возвращает сохранённую конфигурацию (с актуальным `hasPassword`).
    pub fn save(dir: &Path, request: SaveConfigRequest) -> Result<AppConfig, AppError> {
        let path = Self::config_path(dir);
        let request = validate_request(request)?;

        let config = AppConfig {
            theme: request.theme,
            language: request.language,
            ldap_url: request.ldap_url,
            base_dn: request.base_dn,
            use_start_tls: request.use_start_tls,
            allow_invalid_tls: request.allow_invalid_tls,
            audit_enabled: request.audit_enabled,
            clipboard_clear_seconds: request.clipboard_clear_seconds,
            test_mode: request.test_mode,
        };

        Self::write_atomic(&path, &config)?;
        Ok(config)
    }

    fn write_atomic(path: &Path, config: &AppConfig) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| AppError::Internal(format!("сериализация конфигурации: {e}")))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)?;
        info!("конфигурация сохранена");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SaveConfigRequest {
        SaveConfigRequest {
            theme: "dark".into(),
            language: "ru".into(),
            ldap_url: "ldap://dc1.company.ru".into(),
            base_dn: "DC=company,DC=ru".into(),
            use_start_tls: false,
            allow_invalid_tls: false,
            audit_enabled: true,
            clipboard_clear_seconds: 30,
            test_mode: false,
        }
    }

    #[test]
    fn saves_and_reloads_config_atomically() {
        let dir = std::env::temp_dir().join(format!("kmaruda-laps-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let saved = ConfigStore::save(&dir, request()).expect("save");
        assert_eq!(saved.ldap_url, "ldap://dc1.company.ru");

        let loaded = ConfigStore::load(&dir).expect("load");
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.base_dn, "DC=company,DC=ru");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_config_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("kmaruda-laps-corrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CONFIG_FILE), "{ не json").unwrap();

        let loaded = ConfigStore::load(&dir).expect("load");
        assert_eq!(loaded, AppConfig::default());
        assert!(dir.join("config.json.corrupt").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

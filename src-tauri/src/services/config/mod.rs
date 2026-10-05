//! Конфигурация приложения: модель, валидация, атомарное хранилище,
//! секрет bind-учётки в системном keyring.

mod model;
mod store;
mod validate;

pub use model::{AppConfig, ConnectionTest, SaveConfigRequest};
pub use store::ConfigStore;
pub use validate::ensure_connection_config;

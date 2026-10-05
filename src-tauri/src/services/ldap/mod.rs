//! Нативный LDAP-клиент на `ldap3` (tokio): запросы к Active Directory
//! выполняются напрямую из Rust — без PowerShell, RSAT и модуля LAPS.
//!
//! Возможности:
//! * StartTLS и LDAPS, опционально без проверки сертификата (внутренние CA);
//! * интегрированная аутентификация текущей сессии Windows (SASL GSSAPI/SSPI)
//!   или простой bind сервисной учёткой;
//! * таймауты: `conn_timeout` на подключение и `with_timeout` на операции;
//! * нормализация атрибутов записи с учётом того, что ldap3 уносит значения,
//!   случайно оказавшиеся валидным UTF-8, в строковые атрибуты
//!   (актуально для бинарных `objectGUID`, `msLAPS-EncryptedPassword`);
//! * диагностика кодов отказа аутентификации AD (`data 52e` и т.п.).
//!
//! Доменная логика LAPS живёт в [`crate::services::laps`]; этот модуль —
//! только транспорт и разбор сырых записей.

mod connect;
mod errors;
mod escape;
mod model;

pub use escape::escape_filter_value;
pub use model::{DirectoryCredentials, OPERATION_TIMEOUT};

use std::collections::HashMap;

use ldap3::{Scope, SearchEntry, SearchResult};
use serde::Serialize;
use tracing::debug;

use crate::error::AppError;

use connect::{bind, connect};
use errors::{classify_error, classify_search_result};

/// Публичная обёртка классификации транспортного сбоя (`LdapError`) —
/// для доменных модулей, работающих с `ldap3` напрямую.
pub fn classify_transport_error(url: &str, error: &ldap3::LdapError) -> AppError {
    classify_error(url, error)
}

/// Публичная обёртка классификации кода результата LDAP-операции
/// (поиск/модификация): rc=50 → `PermissionDenied`, rc=1+bind → `LdapAuth`, и т.д.
pub fn classify_result(url: &str, result: &ldap3::LdapResult) -> AppError {
    classify_search_result(url, result)
}

/// Открывает сессию: TCP/TLS-соединение + аутентификация.
/// Возвращает соединение и длительность bind в мс (диагностика).
pub async fn open_session(
    credentials: &DirectoryCredentials<'_>,
) -> Result<(ldap3::Ldap, u64), AppError> {
    let mut ldap = connect(credentials).await?;
    let bind_ms = bind(&mut ldap, credentials).await?;
    Ok((ldap, bind_ms))
}

/// Поля rootDSE, нужные приложению.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootDse {
    pub server_dns_name: Option<String>,
    pub default_naming_context: Option<String>,
}

/// Читает rootDSE (пустой base, scope Base). Недоступность rootDSE
/// не фатальна для поиска — возвращает пустую структуру.
pub async fn read_root_dse(ldap: &mut ldap3::Ldap, url: &str) -> RootDse {
    let SearchResult(entries, result) = match ldap
        .with_timeout(OPERATION_TIMEOUT)
        .search(
            "",
            Scope::Base,
            "(objectClass=*)",
            vec!["dnsHostName", "defaultNamingContext"],
        )
        .await
    {
        Ok(value) => value,
        Err(e) => {
            debug!(error = %e, "rootDSE недоступен (не критично)");
            return RootDse::default();
        }
    };
    if result.rc != 0 {
        debug!(rc = result.rc, url, "rootDSE вернул ошибку (не критично)");
        return RootDse::default();
    }
    let mut root = RootDse::default();
    for raw in entries {
        let entry = NormalizedEntry::from_raw(raw);
        root.server_dns_name = entry.text("dnshostname");
        root.default_naming_context = entry.text("defaultnamingcontext");
    }
    root
}

/// Выполняет поиск и проверяет код результата.
///
/// rc=4 (sizeLimitExceeded) — сервер вернул частичный ответ из-за
/// своего size limit: для диагностики и точечных фильтров это успех,
/// записи в ответе валидны. Ошибка — любой иной ненулевой код.
pub async fn search(
    ldap: &mut ldap3::Ldap,
    url: &str,
    base: &str,
    scope: Scope,
    filter: &str,
    attributes: Vec<&str>,
) -> Result<Vec<NormalizedEntry>, AppError> {
    let SearchResult(entries, result) = ldap
        .with_timeout(OPERATION_TIMEOUT)
        .search(base, scope, filter, attributes)
        .await
        .map_err(|e| classify_error(url, &e))?;
    if result.rc != 0 && result.rc != 4 {
        return Err(classify_search_result(url, &result));
    }
    Ok(entries.into_iter().map(NormalizedEntry::from_raw).collect())
}

/// Запись каталога с нормализованными (нижний регистр) именами атрибутов.
///
/// AD возвращает имена атрибутов в своём регистре (`dNSHostName`,
/// `msLAPS-Password`), а ldap3 раскладывает значения по двум картам:
/// строковые и бинарные — причём бинарное значение, случайно оказавшееся
/// валидным UTF-8, попадает в строковые. `NormalizedEntry` скрывает эти
/// детали: `text()` и `binary()` ищут по обоим картам.
#[derive(Debug, Clone)]
pub struct NormalizedEntry {
    pub dn: String,
    attrs: HashMap<String, Vec<String>>,
    bins: HashMap<String, Vec<Vec<u8>>>,
}

impl NormalizedEntry {
    pub fn from_raw(raw: ldap3::ResultEntry) -> Self {
        Self::from_search(SearchEntry::construct(raw))
    }

    /// Конструктор из разобранной записи (используется и в тестах:
    /// `ResultEntry` — «сырой» BER-тег, его в тестах не собрать).
    pub fn from_search(entry: SearchEntry) -> Self {
        let attrs = entry
            .attrs
            .into_iter()
            .map(|(key, values)| (key.to_lowercase(), values))
            .collect();
        let bins = entry
            .bin_attrs
            .into_iter()
            .map(|(key, values)| (key.to_lowercase(), values))
            .collect();
        Self {
            dn: entry.dn,
            attrs,
            bins,
        }
    }

    /// Первое строковое значение атрибута (регистронезависимое имя).
    pub fn text(&self, name: &str) -> Option<String> {
        self.attrs
            .get(name)
            .and_then(|values| values.first())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    /// Первое бинарное значение атрибута. Строковое значение, оказавшееся
    /// валидным UTF-8, возвращается как его байты (особенность ldap3).
    pub fn binary(&self, name: &str) -> Option<Vec<u8>> {
        if let Some(value) = self.bins.get(name).and_then(|values| values.first()) {
            return Some(value.clone());
        }
        self.attrs
            .get(name)
            .and_then(|values| values.first())
            .map(|value| value.as_bytes().to_vec())
    }

    /// Признак присутствия атрибута (с любым непустым значением).
    ///
    /// Покрывает оба хранилища ldap3: бинарное значение, случайно
    /// оказавшееся валидным UTF-8, уносится драйвером в строковые
    /// атрибуты — `has` проверяет обе карты.
    pub fn has(&self, name: &str) -> bool {
        self.attrs
            .get(name)
            .is_some_and(|values| values.iter().any(|v| !v.trim().is_empty()))
            || self
                .bins
                .get(name)
                .is_some_and(|values| values.iter().any(|v| !v.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_entry_reads_attrs_case_insensitively() {
        let raw = SearchEntry {
            dn: "CN=WS01,DC=company,DC=ru".to_string(),
            attrs: HashMap::from([
                (
                    "dNSHostName".to_string(),
                    vec!["ws01.company.ru".to_string()],
                ),
                (
                    "msLAPS-Password".to_string(),
                    vec![r#"{"n":"Administrator"}"#.to_string()],
                ),
            ]),
            bin_attrs: HashMap::from([(
                "msLAPS-EncryptedPassword".to_string(),
                vec![vec![0xDE, 0xAD, 0xBE, 0xEF]],
            )]),
        };
        let entry = NormalizedEntry::from_search(raw);
        assert_eq!(
            entry.text("dnshostname").as_deref(),
            Some("ws01.company.ru")
        );
        assert!(entry.text("mslaps-password").is_some());
        assert!(entry.has("mslaps-encryptedpassword"));
        assert!(!entry.has("ms-mcs-admpwd"));
        assert!(!entry.has("objectguid"));
    }
}

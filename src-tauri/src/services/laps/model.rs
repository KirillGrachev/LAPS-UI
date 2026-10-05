//! Модель данных LAPS: вариант хранения пароля и результат поиска.

use serde::Serialize;

/// Вариант хранения пароля локального администратора.
///
/// AD может содержать несколько наборов атрибутов одновременно (мигрирующий
/// домен: агент старого LAPS ещё пишет `ms-Mcs-AdmPwd`, а Windows LAPS уже
/// пишет `msLAPS-Password`). Приоритет выбора — см. [`super::parse::pick_flavor`]:
/// свежий Windows LAPS важнее legacy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LapsFlavor {
    /// Классический LAPS (2016): пароль открытым текстом в `ms-Mcs-AdmPwd`.
    Legacy,
    /// Windows LAPS (2023+), открытое хранение: JSON в `msLAPS-Password`.
    WindowsPlain,
    /// Windows LAPS, шифрованное хранение: только `msLAPS-EncryptedPassword`.
    /// Расшифровка возможна лишь через RPC к контроллеру домена
    /// (`Get-LapsADPassword`) и нативным LDAP-чтением не выполняется.
    WindowsEncrypted,
    /// Компьютер найден, но ни один LAPS-атрибут не виден: не enrolled,
    /// пароль ещё не сгенерирован или нет прав чтения атрибутов
    /// (AD молча скрывает атрибуты без прав).
    None,
}

impl LapsFlavor {
    /// Стабильный идентификатор для журнала аудита.
    pub fn audit_label(&self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::WindowsPlain => "windows_plain",
            Self::WindowsEncrypted => "windows_encrypted",
            Self::None => "none",
        }
    }
}

/// Результат поиска компьютера: карточка LAPS для интерфейса.
///
/// Контракт фронтенда (`camelCase`). `password` отсутствует, когда пароль
/// недоступен для чтения текущей учётной записью (шифрованное хранение
/// или пустые атрибуты) — такие результаты возвращаются только вместе
/// с типизированной ошибкой, но структура допускает `None` для полноты.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LapsInfo {
    /// DN объекта компьютера — идентификатор для последующих операций
    /// (сброс срока истечения выполняется по DN, а не по имени).
    pub dn: String,
    /// Имя компьютера (CN / sAMAccountName без `$`).
    pub computer_name: String,
    pub dns_host_name: Option<String>,
    pub operating_system: Option<String>,
    /// Определённый вариант хранения пароля.
    pub flavor: LapsFlavor,
    /// Имя управляемой локальной учётки (из JSON Windows LAPS; для legacy
    /// хранится в политике и недоступно из атрибутов).
    pub account_name: Option<String>,
    /// Пароль локального администратора (отсутствует для
    /// `WindowsEncrypted` / `None`).
    pub password: Option<String>,
    /// Момент обновления пароля (RFC 3339, локальное время), если известен.
    pub updated_at: Option<String>,
    /// Срок истечения пароля (RFC 3339, локальное время).
    pub expires_at: Option<String>,
    /// Пароль получен расшифровкой `msLAPS-EncryptedPassword` через
    /// KDS/DPAPI-NG (Windows LAPS с шифрованным хранением).
    #[serde(default)]
    pub decrypted: bool,
    /// Как именно получен расшифрованный пароль: `kds` (нативный
    /// NCryptUnprotectSecret) или `laps-module` (системный модуль
    /// Windows LAPS, Get-LapsADPassword). Прозрачность происхождения.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decrypt_method: Option<String>,
    /// Какие наборы атрибутов физически присутствуют на объекте —
    /// для диагностики смешанных/мигрирующих доменов в UI.
    pub legacy_present: bool,
    pub windows_plain_present: bool,
    pub encrypted_present: bool,
    /// Срок истечения можно изменить (есть атрибут, в который мы умеем
    /// писать для этого варианта LAPS).
    pub rotation_possible: bool,
    /// Фактическое присутствие атрибутов срока истечения — внутренняя
    /// информация для построения LDAP-модификаций (не часть контракта UI).
    #[serde(skip)]
    pub win_expiration_present: bool,
    #[serde(skip)]
    pub legacy_expiration_present: bool,
}

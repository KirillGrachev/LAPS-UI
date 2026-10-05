//! Локальный журнал аудита операций с паролями LAPS.
//!
//! Принципиальная позиция: аудит пишется **из Rust-команд**, а не из
//! фронтенда. Журнал, который ведёт UI, тривиально обходится и не
//! выдерживает никакой проверки безопасником. Здесь каждая операция
//! чтения/сброса пароля журналируется на бэкенде до возврата результата.
//!
//! Формат файла — **читаемые человеком строки фиксированной ширины**
//! (журнал открывают в нотпаде и читают глазами, а не парсят скриптами):
//!
//! ```text
//! # время | действие | код | компьютер | тип LAPS | субъект | подробности
//! 2026-10-05 13:42:07.123 +03:00 | search           | OK               | WS01               | windows_plain     | KMARUDA\ivanov@HELP-01
//! 2026-10-05 13:44:02.008 +03:00 | search_failed    | COMPUTER_NOT_FOUND | ws99             | -                 | KMARUDA\ivanov@HELP-01 | Компьютер не найден…
//! ```
//!
//! Колонки разделены ` | `; подробности (могут содержать что угодно) —
//! всегда последняя колонка, при записи в них заменяется ` | ` на ` / `.
//! Старые строки и битые данные пропускаются при чтении без падения.
//!
//! Ротация в стиле Minecraft `latest.log`: каждый запуск приложения
//! пишет живой `audit.log`, а при старте файл прошлого запуска пакуется
//! в gzip-архив `audit-<метка времени>.log.gz (хранится не более 14
//! архивов). Чрезмерно большой живой файл внутри одного запуска
//! архивируется так же. Чтение и пагинация — сквозные по живому файлу
//! и всем архивам.
//!
//! Важно: в журнал **никогда** не попадает значение пароля — только факт
//! операции, целевой компьютер, субъект и результат.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::AppError;

/// Префикс/суффикс имени gzip-архива прошлых запусков:
/// `audit-2026-10-05-134207.log.gz` (как latest.log в Minecraft: текущий
/// запуск пишет живой файл, прошлые уходят в архив).
const ARCHIVE_PREFIX: &str = "audit-";
const ARCHIVE_SUFFIX: &str = ".log.gz";
/// Сколько архивов храним; старее — удаляем.
const MAX_ARCHIVES: usize = 14;
/// Защита от аномальных объёмов внутри одного запуска: превысив, файл
/// тоже уходит в архив и журнал продолжается с чистого листа.
const MID_RUN_MAX_BYTES: u64 = 10 * 1024 * 1024;
const LOG_FILE: &str = "audit.log";

/// Шапка файла: пишется один раз при создании, поясняет колонки.
const FILE_HEADER: &str = "# KMAruda LAPS audit log\n# время | действие | код | компьютер | тип LAPS | субъект | подробности";

/// Ширины колонок (символов) для ровного табличного вида.
const W_TS: usize = 30;
const W_ACTION: usize = 16;
const W_CODE: usize = 18;
const W_TARGET: usize = 18;
const W_FLAVOR: usize = 17;
const W_ACTOR: usize = 30;

/// Тип журналируемого действия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// Успешное чтение пароля LAPS.
    Search,
    /// Чтение завершилось ошибкой (не найден, нет прав, нет данных...).
    SearchFailed,
    /// Успешный сброс/продление срока истечения.
    Rotate,
    /// Ошибка сброса/продления срока.
    RotateFailed,
    /// Инженер скопировал пароль в буфер обмена.
    Copy,
    /// Сохранение конфигурации подключения.
    ConfigSaved,
    /// Успешный вход в приложение.
    Login,
    /// Выход (стирание сессии из памяти).
    Logout,
    /// Проверка подключения.
    ConnectionTest,
}

impl AuditAction {
    /// Стабильный токен для файла журнала и контракта IPC.
    pub fn token(&self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::SearchFailed => "search_failed",
            Self::Rotate => "rotate",
            Self::RotateFailed => "rotate_failed",
            Self::Copy => "copy",
            Self::ConfigSaved => "config_saved",
            Self::Login => "login",
            Self::Logout => "logout",
            Self::ConnectionTest => "connection_test",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        Some(match token {
            "search" => Self::Search,
            "search_failed" => Self::SearchFailed,
            "rotate" => Self::Rotate,
            "rotate_failed" => Self::RotateFailed,
            "copy" => Self::Copy,
            "config_saved" => Self::ConfigSaved,
            "login" => Self::Login,
            "logout" => Self::Logout,
            "connection_test" => Self::ConnectionTest,
            _ => return None,
        })
    }
}

/// Запись журнала (контракт IPC для таблицы в UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    /// Момент события, RFC 3339 с локальным смещением.
    pub ts: String,
    /// Субъект: `DOMAIN\user@WORKSTATION` (SSO) или учётная запись входа.
    pub actor: String,
    pub action: AuditAction,
    /// Целевой компьютер (имя или DN) либо `-`.
    pub target: String,
    /// Вариант LAPS, если определён (`legacy` / `windows_plain` / ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    /// Код результата: `OK` или стабильный код ошибки.
    pub code: String,
    /// Краткие подробности (текст ошибки, режим сброса). Без секретов.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Страница журнала для UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    /// Записи от новейших к старейшим.
    pub entries: Vec<AuditEvent>,
    /// Всего записей в журнале (для пагинации).
    pub total: usize,
}

/// RFC 3339 (`2026-10-05T13:42:07.123+03:00`) → читаемый вид
/// (`2026-10-05 13:42:07.123 +03:00`) с нормализацией дроби до
/// миллисекунд: ширина колонки времени всегда постоянна.
fn pretty_ts(rfc: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(rfc) {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M:%S%.3f %:z").to_string(),
        Err(_) => rfc.to_string(),
    }
}

/// Обратное преобразование: читаемый вид → RFC 3339 (миллисекунды)
/// для контракта UI. Нечитаемые строки → `None`.
/// Парсим в `DateTime<FixedOffset>`: спецификатор `%:z` требует
/// присутствия смещения в контексте форматирования.
fn machine_ts(pretty: &str) -> Option<String> {
    chrono::DateTime::parse_from_str(pretty, "%Y-%m-%d %H:%M:%S%.3f %:z")
        .ok()
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string())
}

/// Колонка фиксированной ширины: обрезка с «…» либо добивка пробелами.
fn column(value: &str, width: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() > width {
        let head: String = chars.into_iter().take(width - 1).collect();
        format!("{head}…")
    } else {
        format!("{value:<width$}")
    }
}

/// Сборка строки журнала.
/// Разделитель колонок не должен встречаться внутри подробностей.
fn format_line(event: &AuditEvent) -> String {
    let flavor = event.flavor.as_deref().unwrap_or("-");
    let mut line = format!(
        "{} | {} | {} | {} | {} | {}",
        column(&pretty_ts(&event.ts), W_TS),
        column(event.action.token(), W_ACTION),
        column(&event.code, W_CODE),
        column(&event.target, W_TARGET),
        column(flavor, W_FLAVOR),
        column(&event.actor, W_ACTOR),
    );
    if let Some(details) = event
        .details
        .as_deref()
        .map(str::trim)
        .filter(|details| !details.is_empty())
    {
        line.push_str(" | ");
        line.push_str(&details.replace(" | ", " / "));
    }
    line
}

/// Разбор строки журнала; битые/чужие строки → `None` (пропускаются).
/// Наследие rc-сборок: JSONL с тем же контрактом полей.
/// Старые записи продолжают читаться (и в UI, и в пагинации).
fn parse_line(line: &str) -> Option<AuditEvent> {
    let line = line.trim_end();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if line.starts_with('{') {
        return serde_json::from_str::<AuditEvent>(line).ok();
    }
    let mut parts = line.splitn(7, " | ").map(str::trim);
    let ts = machine_ts(parts.next()?)?;
    let action = AuditAction::from_token(parts.next()?)?;
    let code = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let flavor = parts
        .next()
        .map(|value| (!value.is_empty() && value != "-").then(|| value.to_string()))?;
    let actor = parts.next()?.to_string();
    let details = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if code.is_empty() || actor.is_empty() {
        return None;
    }
    Some(AuditEvent {
        ts,
        actor,
        action,
        target,
        flavor,
        code,
        details,
    })
}

#[derive(Clone)]
pub struct AuditLogger {
    dir: PathBuf,
}

impl AuditLogger {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    /// Дописывает событие. Ошибка записи не должна ронять основную
    /// операцию (чтение пароля важнее журнала) — только warn в трейсинг.
    pub fn append(&self, mut event: AuditEvent) {
        if event.ts.is_empty() {
            event.ts = Local::now().to_rfc3339();
        }
        if let Err(e) = self.append_inner(&event) {
            warn!(error = %e, "не удалось записать событие аудита");
        }
    }

    /// Конструктор события с текущей меткой времени.
    pub fn event(
        actor: String,
        action: AuditAction,
        target: String,
        code: impl Into<String>,
    ) -> AuditEvent {
        AuditEvent {
            ts: Local::now().to_rfc3339(),
            actor,
            action,
            target,
            flavor: None,
            code: code.into(),
            details: None,
        }
    }

    fn append_inner(&self, event: &AuditEvent) -> Result<(), AppError> {
        fs::create_dir_all(&self.dir)?;
        let path = self.log_path();
        self.rotate_if_needed(&path)?;

        let fresh = !path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| AppError::Audit(format!("не удалось открыть {LOG_FILE}: {e}")))?;
        if fresh {
            writeln!(file, "{FILE_HEADER}")
                .map_err(|e| AppError::Audit(format!("не удалось записать шапку: {e}")))?;
        }
        writeln!(file, "{}", format_line(event))
            .map_err(|e| AppError::Audit(format!("не удалось записать в {LOG_FILE}: {e}")))?;
        Ok(())
    }

    /// Если текущий файл перерос предел одного запуска — архивируем его
    /// gzip'ом и продолжаем с чистого листа.
    fn rotate_if_needed(&self, path: &Path) -> Result<(), AppError> {
        let Ok(meta) = fs::metadata(path) else {
            return Ok(());
        };
        if meta.len() < MID_RUN_MAX_BYTES {
            return Ok(());
        }
        self.archive_current("перерос предел запуска")
    }

    /// Архивация журнала ПРЕДЫДУЩЕГО запуска: вызывается при старте
    /// приложения. Живой `audit.log` пакуется в `audit-<метка>.log.gz`,
    /// новый запуск начинает пустой файл — читать актуальное легко,
    /// а история не раздувает дисковое количество текста.
    pub fn archive_previous_run(&self) {
        let path = self.log_path();
        let Ok(meta) = fs::metadata(&path) else {
            return;
        };
        if meta.len() == 0 {
            return;
        }
        if let Err(e) = self.archive_current("новый запуск приложения") {
            warn!(error = %e, "не удалось архивировать журнал прошлого запуска");
        }
    }

    /// Пакует текущий `audit.log` в gzip-архив с меткой времени и удаляет
    /// исходный файл; затем применяет политику хранения архивов.
    /// Маловероятный конфликт имён (две ротации в одну секунду): уточняем.
    fn archive_current(&self, reason: &str) -> Result<(), AppError> {
        let path = self.log_path();
        let stamp = Local::now().format("%Y-%m-%d-%H%M%S");
        let archive = self
            .dir
            .join(format!("{ARCHIVE_PREFIX}{stamp}{ARCHIVE_SUFFIX}"));
        let archive = if archive.exists() {
            self.dir.join(format!(
                "{ARCHIVE_PREFIX}{stamp}-{}{ARCHIVE_SUFFIX}",
                std::process::id()
            ))
        } else {
            archive
        };

        let source = File::open(&path).map_err(|e| {
            AppError::Audit(format!("не удалось открыть журнал для архивации: {e}"))
        })?;
        let out = File::create(&archive)
            .map_err(|e| AppError::Audit(format!("не удалось создать архив журнала: {e}")))?;
        let mut encoder = GzEncoder::new(out, Compression::default());
        std::io::copy(&mut BufReader::new(source), &mut encoder)
            .map_err(|e| AppError::Audit(format!("не удалось упаковать журнал: {e}")))?;
        encoder
            .finish()
            .map_err(|e| AppError::Audit(format!("не удалось завершить архив журнала: {e}")))?;
        fs::remove_file(&path)
            .map_err(|e| AppError::Audit(format!("не удалось удалить упакованный журнал: {e}")))?;

        tracing::info!(archive = %archive.display(), reason, "журнал аудита архивирован");
        self.prune_archives();
        Ok(())
    }

    /// Архивы от новых к старым (метка времени в имени сортируется лексически).
    fn archives(&self) -> Vec<PathBuf> {
        let Ok(read_dir) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut archives: Vec<PathBuf> = read_dir
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(ARCHIVE_PREFIX) && name.ends_with(ARCHIVE_SUFFIX))
                    .unwrap_or(false)
            })
            .collect();
        archives.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        archives
    }

    /// Политика хранения: не более `MAX_ARCHIVES` архивов.
    fn prune_archives(&self) {
        for stale in self.archives().into_iter().skip(MAX_ARCHIVES) {
            if let Err(e) = fs::remove_file(&stale) {
                warn!(path = %stale.display(), error = %e, "не удалось удалить старый архив журнала");
            }
        }
    }

    /// Читает страницу журнала (новые сверху) ПО ВСЕМ НОСИТЕЛЯМ: живой
    /// файл текущего запуска плюс gzip-архивы прошлых запусков. Пагинация
    /// сквозная: offset/limit считаются по объединённому списку.
    ///
    /// Журнал одного запуска невелик; архивы распаковываются потоково,
    /// битые строки пропускаются.
    /// Источники в порядке «старые генерации → новые»: архивы от
    /// старейшего к новейшему, затем живой файл текущего запуска.
    /// Внутри файла строки идут в порядке записи.
    /// Newest first: после реверса последовательности «старые→новые»
    /// стабильная сортировка по убыванию ts сохраняет для совпадающих
    /// меток порядок «новее записано — выше в выдаче».
    pub fn list(&self, limit: u32, offset: u32) -> Result<AuditPage, AppError> {
        let limit = limit.clamp(1, 500) as usize;
        let offset = offset as usize;

        let mut events = Vec::new();
        let mut sources: Vec<PathBuf> = self.archives();
        sources.reverse();
        sources.push(self.log_path());

        for path in sources {
            let reader: Box<dyn BufRead> = match path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with(ARCHIVE_SUFFIX))
                .unwrap_or(false)
            {
                true => {
                    let Ok(file) = File::open(&path) else {
                        continue;
                    };
                    Box::new(BufReader::new(GzDecoder::new(file)))
                }
                false => {
                    let Ok(file) = File::open(&path) else {
                        continue;
                    };
                    Box::new(BufReader::new(file))
                }
            };
            let mut skipped = 0u32;
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                match parse_line(&line) {
                    Some(event) => events.push(event),
                    None if !line.trim().is_empty() && !line.starts_with('#') => skipped += 1,
                    None => {}
                }
            }
            if skipped > 0 {
                warn!(
                    skipped,
                    path = %path.display(),
                    "пропущены нечитаемые строки журнала аудита"
                );
            }
        }

        events.reverse();
        events.sort_by(|a, b| b.ts.cmp(&a.ts));

        let total = events.len();
        let entries = events.into_iter().skip(offset).take(limit).collect();
        Ok(AuditPage { entries, total })
    }
}

/// Субъект операции для журнала.
///
/// Интегрированная аутентификация: домен и пользователь текущей сессии
/// Windows (`USERDOMAIN\USERNAME@COMPUTERNAME`) — те же сведения, что видит
/// контроллер домена в своих журналах, что позволяет коррелировать события.
/// Явный вход: учётная запись, введённая на экране входа.
pub fn current_actor(use_integrated_auth: bool, bind_dn: Option<&str>) -> String {
    if !use_integrated_auth {
        return bind_dn
            .map(str::to_string)
            .unwrap_or_else(|| "unknown-bind".to_string());
    }
    let user = match (env_or("USERDOMAIN"), env_or("USERNAME")) {
        (Some(domain), Some(name)) => format!("{domain}\\{name}"),
        (None, Some(name)) => name,
        _ => current_user_fallback(),
    };
    match env_or("COMPUTERNAME") {
        Some(machine) => format!("{user}@{machine}"),
        None => user,
    }
}

fn env_or(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

#[cfg(not(windows))]
fn current_user_fallback() -> String {
    env_or("USER").unwrap_or_else(|| "unknown".to_string())
}

/// На Windows USERDOMAIN/USERNAME практически всегда установлены;
/// страховка — имя из окружения процесса.
#[cfg(windows)]
fn current_user_fallback() -> String {
    env_or("USERNAME").unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kmaruda-laps-audit-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn appends_and_lists_events_newest_first() {
        let dir = temp_dir("list");
        let logger = AuditLogger::new(dir.clone());

        for i in 0..5 {
            logger.append(AuditLogger::event(
                "TEST\\user@PC".into(),
                AuditAction::Search,
                format!("WS{i:02}"),
                "OK",
            ));
        }
        let page = logger.list(10, 0).expect("страница");
        assert_eq!(page.total, 5);
        assert_eq!(page.entries.first().unwrap().target, "WS04");
        assert_eq!(page.entries.last().unwrap().target, "WS00");

        let page = logger.list(2, 2).expect("страница");
        assert_eq!(page.total, 5);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].target, "WS02");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// Шапка + одна запись.
    /// Читаемый вид: дата, время и смещение — раздельно, колонки ровные.
    /// Разделитель внутри подробностей нейтрализован.
    /// Круговой прогон: строка файла → событие → те же поля.
    fn file_lines_are_human_readable_and_roundtrip() {
        let dir = temp_dir("format");
        let logger = AuditLogger::new(dir.clone());
        logger.append(AuditEvent {
            ts: "2026-10-05T13:42:07.123+03:00".into(),
            actor: "KMARUDA\\ivanov@HELP-01".into(),
            action: AuditAction::SearchFailed,
            target: "ws99".into(),
            flavor: None,
            code: "COMPUTER_NOT_FOUND".into(),
            details: Some("Компьютер не найден | проверьте имя".into()),
        });

        let content = fs::read_to_string(logger.log_path()).expect("чтение");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with('#'));
        let line = lines[2];
        assert!(
            line.starts_with("2026-10-05 13:42:07.123 +03:00 | "),
            "{line}"
        );
        assert!(
            line.contains("| search_failed    | COMPUTER_NOT_FOUND | ws99"),
            "{line}"
        );
        assert!(!line.splitn(7, " | ").nth(6).unwrap().contains(" | "));

        let parsed = parse_line(line).expect("разбор");
        assert_eq!(parsed.ts, "2026-10-05T13:42:07.123+03:00");
        assert_eq!(parsed.action, AuditAction::SearchFailed);
        assert_eq!(parsed.code, "COMPUTER_NOT_FOUND");
        assert_eq!(parsed.target, "ws99");
        assert_eq!(parsed.flavor, None);
        assert_eq!(parsed.actor, "KMARUDA\\ivanov@HELP-01");
        assert_eq!(
            parsed.details.as_deref(),
            Some("Компьютер не найден / проверьте имя")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_corrupted_and_foreign_lines() {
        let dir = temp_dir("corrupt");
        let logger = AuditLogger::new(dir.clone());
        logger.append(AuditLogger::event(
            "A\\b@C".into(),
            AuditAction::Copy,
            "WS01".into(),
            "OK",
        ));
        let mut content = fs::read_to_string(logger.log_path()).unwrap();
        content.push_str("{ старый json из первой версии }\nслучайный мусор\n");
        fs::write(logger.log_path(), content).unwrap();

        let page = logger.list(10, 0).expect("страница");
        assert_eq!(
            page.total, 1,
            "битые строки пропускаются, валидная читается"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// Микросекунды нормализуются до миллисекунд, целые секунды
    /// получают явную дробь: ширина колонки всегда постоянна.
    /// Битый ввод не парсится.
    fn pretty_and_machine_timestamps_roundtrip_to_millis() {
        let rfc = "2026-01-31T09:00:00.005-05:00";
        assert_eq!(machine_ts(&pretty_ts(rfc)).as_deref(), Some(rfc));
        assert_eq!(
            machine_ts(&pretty_ts("2026-01-31T09:00:00+03:00")).as_deref(),
            Some("2026-01-31T09:00:00.000+03:00")
        );
        assert_eq!(
            pretty_ts("2026-01-31T09:00:00.123456+03:00"),
            "2026-01-31 09:00:00.123 +03:00"
        );
        assert_eq!(machine_ts("мусор"), None);
    }

    #[test]
    /// «Прошлый запуск»: три события в живом файле.
    /// Старт «нового запуска»: живой файл ушёл в gzip-архив.
    /// Архив пустой не читается как события, но старые доступны через list.
    /// Текущий запуск пишет новый живой файл; пагинация сквозная:
    /// сначала новые события живого файла, затем архивные.
    fn archives_previous_run_and_paginates_across_archives() {
        let dir = temp_dir("archive");
        let logger = AuditLogger::new(dir.clone());

        for i in 0..3 {
            logger.append(AuditLogger::event(
                "OLD\run@PC".into(),
                AuditAction::Search,
                format!("OLD{i}"),
                "OK",
            ));
        }
        assert!(logger.log_path().exists());

        logger.archive_previous_run();
        assert!(!logger.log_path().exists(), "живой файл упакован");
        let archives = logger.archives();
        assert_eq!(archives.len(), 1);
        assert!(archives[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".log.gz"));

        let page = logger.list(10, 0).expect("страница из архива");
        assert_eq!(page.total, 3);
        assert_eq!(page.entries[0].target, "OLD2");

        for i in 0..4 {
            logger.append(AuditLogger::event(
                "NEW\run@PC".into(),
                AuditAction::Copy,
                format!("NEW{i}"),
                "OK",
            ));
        }
        let page = logger.list(10, 0).expect("сквозная страница");
        assert_eq!(page.total, 7);
        assert_eq!(page.entries[0].target, "NEW3");
        assert_eq!(page.entries[6].target, "OLD0");

        let page = logger.list(3, 4).expect("смещение в архив");
        assert_eq!(page.entries[0].target, "OLD2");
        assert_eq!(page.entries[2].target, "OLD0");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn serializes_ipc_contract_in_camel_case() {
        let event = AuditEvent {
            ts: "2026-10-05T14:00:00+03:00".into(),
            actor: "KMARUDA\\ivanov@HELP-01".into(),
            action: AuditAction::RotateFailed,
            target: "WS01".into(),
            flavor: Some("windows_plain".into()),
            code: "PERMISSION_DENIED".into(),
            details: Some("rc=50".into()),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["action"], "rotate_failed");
        assert_eq!(json["code"], "PERMISSION_DENIED");
        assert!(json.get("details").is_some());

        let event = AuditLogger::event("a".into(), AuditAction::Search, "WS01".into(), "OK");
        let json = serde_json::to_value(&event).unwrap();
        assert!(json.get("flavor").is_none(), "None-поля не сериализуются");
        assert!(json.get("details").is_none());
    }

    #[test]
    /// Интегрированный режим: строка непустая в любом окружении.
    fn actor_uses_environment_or_bind_dn() {
        assert_eq!(
            current_actor(false, Some("CN=svc-laps,OU=Service,DC=company,DC=ru")),
            "CN=svc-laps,OU=Service,DC=company,DC=ru"
        );
        assert!(!current_actor(true, None).is_empty());
    }
}

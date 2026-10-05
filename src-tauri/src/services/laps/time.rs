//! Преобразования времени Active Directory.
//!
//! AD хранит моменты времени в формате **FILETIME**: число 100-наносекундных
//! интервалов с 1 января 1601 г. UTC. В LAPS встречаются три его представления:
//! * `ms-Mcs-AdmPwdExpirationTime` (legacy) и `msLAPS-PasswordExpirationTime`
//!   (Windows LAPS) — **десятичная** строка, например `133600000000000000`;
//! * поле `t` JSON-объекта `msLAPS-Password` — **шестнадцатеричная** строка
//!   без префикса, например `1d8161b41c41cde` (см. MS LAPS schema reference).
//!
//! Фронтенд получает/отправляет время в RFC 3339 с локальным смещением
//! (`2026-10-07T12:00:00+03:00`) и наивные локальные строки из datetime-пикера
//! (`2026-10-07T12:00`).

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};

/// Разница между эпохой FILETIME (1601-01-01) и Unix-эпохой в 100-нс интервалах
/// (11 644 473 600 секунд × 10⁷).
const FILETIME_UNIX_EPOCH_DIFF: i64 = 116_444_736_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

/// FILETIME -> UTC-момент. Отклоняет вырожденные значения (0, отрицательные,
/// за пределами представимого диапазона).
pub fn filetime_to_utc(filetime: i64) -> Option<DateTime<Utc>> {
    if filetime <= 0 {
        return None;
    }
    let secs = filetime
        .checked_sub(FILETIME_UNIX_EPOCH_DIFF)?
        .div_euclid(TICKS_PER_SECOND);
    let nanos = (filetime.rem_euclid(TICKS_PER_SECOND) * 100) as u32;
    DateTime::from_timestamp(secs, nanos)
}

/// UTC-момент -> FILETIME.
pub fn utc_to_filetime(datetime: DateTime<Utc>) -> i64 {
    datetime.timestamp() * TICKS_PER_SECOND
        + FILETIME_UNIX_EPOCH_DIFF
        + (datetime.timestamp_subsec_nanos() / 100) as i64
}

/// Разбор десятичного FILETIME из строковых атрибутов AD.
/// Пробелы и нецифровые хвосты не допускаются — только целое число.
pub fn parse_filetime_decimal(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    trimmed.parse::<i64>().ok()
}

/// Разбор шестнадцатеричного FILETIME (поле `t` в JSON `msLAPS-Password`).
pub fn parse_filetime_hex(raw: &str) -> Option<i64> {
    let trimmed = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    i64::from_str_radix(trimmed, 16).ok()
}

/// Наивная локальная строка (`2026-10-07T12:00[:00]`) -> FILETIME.
/// Неоднозначные/несуществующие локальные моменты (перевод часов) отклоняются.
pub fn parse_local_naive_to_filetime(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M"))
        .ok()?;
    let local = Local.from_local_datetime(&naive).single()?;
    Some(utc_to_filetime(local.with_timezone(&Utc)))
}

/// FILETIME -> RFC 3339 в локальном часовом поясе (контракт фронтенда).
pub fn filetime_to_rfc3339_local(filetime: i64) -> Option<String> {
    filetime_to_utc(filetime).map(|utc| utc.with_timezone(&Local).to_rfc3339())
}

/// Текущий момент в FILETIME.
pub fn now_filetime() -> i64 {
    utc_to_filetime(Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    /// 2023-04-11T00:00:00Z (день выхода Windows LAPS) — эталонное значение.
    fn converts_known_filetime_values() {
        let reference = Utc.with_ymd_and_hms(2023, 4, 11, 0, 0, 0).unwrap();
        let ft = utc_to_filetime(reference);
        assert_eq!(ft, 133_256_448_000_000_000);
        assert_eq!(filetime_to_utc(ft), Some(reference));
    }

    #[test]
    /// Точность FILETIME — 100 нс; наносекундный остаток усекается.
    fn roundtrips_subsecond_precision() {
        let moment = Utc.with_ymd_and_hms(2026, 10, 5, 12, 34, 56).unwrap()
            + chrono::Duration::nanoseconds(123_456_700);
        let ft = utc_to_filetime(moment);
        let back = filetime_to_utc(ft).expect("обратная конвертация");
        assert_eq!(back.timestamp(), moment.timestamp());
        assert!(
            (back.timestamp_subsec_nanos() as i64 - moment.timestamp_subsec_nanos() as i64).abs()
                < 100
        );
    }

    #[test]
    fn rejects_degenerate_filetimes() {
        assert_eq!(filetime_to_utc(0), None);
        assert_eq!(filetime_to_utc(-1), None);
    }

    #[test]
    fn parses_decimal_filetime_strings() {
        assert_eq!(
            parse_filetime_decimal("133600000000000000"),
            Some(133_600_000_000_000_000)
        );
        assert_eq!(
            parse_filetime_decimal(" 133600000000000000 "),
            Some(133_600_000_000_000_000)
        );
        assert_eq!(parse_filetime_decimal(""), None);
        assert_eq!(parse_filetime_decimal("133abc"), None);
        assert_eq!(parse_filetime_decimal("-133"), None);
        assert_eq!(parse_filetime_decimal("99999999999999999999999"), None);
    }

    #[test]
    /// Значение из документации MS LAPS: 1d8161b41c41cde.
    fn parses_hex_filetime_strings() {
        let ft = parse_filetime_hex("1d8161b41c41cde").expect("hex parse");
        assert!(ft > 130_000_000_000_000_000 && ft < 140_000_000_000_000_000);
        assert!(filetime_to_utc(ft).is_some());
        assert_eq!(
            parse_filetime_hex("0x1d8161b41c41cde"),
            parse_filetime_hex("1d8161b41c41cde")
        );
        assert_eq!(parse_filetime_hex("zzz"), None);
        assert_eq!(parse_filetime_hex(""), None);
    }

    #[test]
    /// Форматы datetime-пикера и ручного ввода.
    fn parses_local_naive_datetime_strings() {
        assert!(parse_local_naive_to_filetime("2026-10-07T12:00").is_some());
        assert!(parse_local_naive_to_filetime("2026-10-07T12:00:30").is_some());
        assert!(parse_local_naive_to_filetime("2026-10-07 12:00").is_some());
        assert!(parse_local_naive_to_filetime("07.10.2026 12:00").is_none());
        assert!(parse_local_naive_to_filetime("2026-13-45T99:99").is_none());
    }

    #[test]
    /// RFC 3339 с локальным смещением — date-fns parseISO читает его как есть.
    fn formats_expiration_for_frontend() {
        let ft = 133_256_448_000_000_000;
        let formatted = filetime_to_rfc3339_local(ft).expect("format");
        assert!(formatted.starts_with("2023-04-1"), "{formatted}");
        assert!(formatted.contains('T'));
    }
}

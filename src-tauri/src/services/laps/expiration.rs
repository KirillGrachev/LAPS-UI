//! Управление сроком истечения пароля LAPS (сброс/продление).
//!
//! Механика обоих поколений LAPS одинакова: агент на компьютере при
//! очередном применении политики сравнивает срок истечения с текущим
//! временем и генерирует новый пароль, если срок наступил. Поэтому:
//! * **«Сбросить сейчас»** — удаляем значение атрибута срока истечения
//!   (поведение Microsoft-командлетов `Reset-LapsADPasswordExpiration` /
//!   `Reset-AdmPwdExpiration`): при следующем цикле политики пароль
//!   будет заменён;
//! * **«Продлить/изменить срок»** — пишем новый FILETIME в атрибут.
//!
//! Атрибуты, в которые умеем писать:
//! * legacy LAPS — `ms-Mcs-AdmPwdExpirationTime`;
//! * Windows LAPS — `msLAPS-PasswordExpirationTime`.
//! В смешанном домене (присутствуют оба) обновляются оба, чтобы агенты
//! любого поколения увидели согласованное значение.
//!
//! Права: операция требует делегированного разрешения
//! «Reset msLAPS password expiration» (Windows LAPS) или Write на
//! `ms-Mcs-AdmPwdExpirationTime` (legacy). Отказ AD (rc=50) превращается
//! в [`AppError::PermissionDenied`] с инструкцией для администратора домена.

use std::collections::HashSet;

use ldap3::Mod;

use crate::error::AppError;

use super::model::LapsInfo;
use super::time;

/// Режим изменения срока истечения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationMode {
    /// Немедленный сброс: значение атрибута срока удаляется,
    /// агент сгенерирует новый пароль при следующем применении политики.
    Now,
    /// Запись конкретной даты (наивная локальная строка
    /// `YYYY-MM-DDTHH:MM[:SS]` из datetime-пикера интерфейса).
    At(String),
}

/// Верхняя граница разумности для пользовательской даты: +10 лет.
const MAX_FUTURE_YEARS: i64 = 10;

/// LDAP-имена атрибутов срока истечения в том регистре, в котором их
/// ожидает сервер (запись чувствительна к точному display-имени атрибута
/// не больше, чем поиск, но используем канонические имена из схемы).
const WRITABLE_WIN_EXPIRATION: &str = "msLAPS-PasswordExpirationTime";
const WRITABLE_LEGACY_EXPIRATION: &str = "ms-Mcs-AdmPwdExpirationTime";

/// Строит набор модификаций для сброса/продления срока по текущему
/// состоянию объекта (какие атрибуты физически присутствуют).
/// Точный источник правды — признак rotation_possible из парсера
/// (вычисляется по фактическому наличию атрибутов срока).
/// Разумность: не раньше 2000 года и не позже +10 лет.
pub fn build_rotation_mods(
    info: &LapsInfo,
    mode: &RotationMode,
) -> Result<Vec<Mod<String>>, AppError> {
    if !info.rotation_possible {
        return Err(AppError::NoLapsData(
            "На объекте компьютера нет атрибута срока истечения LAPS \
             (msLAPS-PasswordExpirationTime / ms-Mcs-AdmPwdExpirationTime). \
             Сброс срока невозможен: компьютер не управляется LAPS или атрибуты не видны \
             вашей учётной записи."
                .into(),
        ));
    }
    let targets = rotation_targets(info);
    if targets.is_empty() {
        return Err(AppError::NoLapsData(
            "Не удалось определить, каким поколением LAPS управляется компьютер: \
             атрибуты срока истечения отсутствуют или не видны."
                .into(),
        ));
    }

    let mods = match mode {
        RotationMode::Now => targets
            .into_iter()
            .map(|attr| Mod::Delete(attr.to_string(), HashSet::new()))
            .collect(),
        RotationMode::At(local) => {
            let filetime = time::parse_local_naive_to_filetime(local).ok_or_else(|| {
                AppError::Validation(format!(
                    "Некорректная дата/время: {local:?}. Ожидается формат ГГГГ-ММ-ДДTЧЧ:ММ."
                ))
            })?;
            let now = time::now_filetime();
            let min = time::parse_local_naive_to_filetime("2000-01-01T00:00").unwrap_or(0);
            let max = now + MAX_FUTURE_YEARS * 366 * 24 * 3600 * 10_000_000;
            if filetime < min || filetime > max {
                return Err(AppError::Validation(
                    "Дата вне допустимого диапазона (2000 год .. +10 лет от сегодня)".into(),
                ));
            }
            targets
                .into_iter()
                .map(|attr| Mod::Replace(attr.to_string(), HashSet::from([filetime.to_string()])))
                .collect()
        }
    };
    Ok(mods)
}

/// Определяет, в какие атрибуты писать, по фактическому присутствию
/// атрибутов срока истечения на объекте.
fn rotation_targets(info: &LapsInfo) -> Vec<&'static str> {
    let mut targets: Vec<&'static str> = Vec::new();
    if info.win_expiration_present {
        targets.push(WRITABLE_WIN_EXPIRATION);
    }
    if info.legacy_expiration_present {
        targets.push(WRITABLE_LEGACY_EXPIRATION);
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::laps::model::LapsFlavor;

    fn info(
        flavor: LapsFlavor,
        legacy: bool,
        plain: bool,
        encrypted: bool,
        rotation: bool,
    ) -> LapsInfo {
        LapsInfo {
            dn: "CN=WS01,DC=company,DC=ru".into(),
            computer_name: "WS01".into(),
            dns_host_name: None,
            operating_system: None,
            flavor,
            account_name: None,
            password: None,
            updated_at: None,
            expires_at: Some("2026-10-07T12:00:00+03:00".into()),
            legacy_present: legacy,
            windows_plain_present: plain,
            encrypted_present: encrypted,
            rotation_possible: rotation,
            decrypted: false,
            decrypt_method: None,
            win_expiration_present: rotation && (plain || encrypted),
            legacy_expiration_present: rotation && legacy,
        }
    }

    #[test]
    fn now_mode_deletes_present_expiration_attrs() {
        let mods = build_rotation_mods(
            &info(LapsFlavor::WindowsPlain, false, true, false, true),
            &RotationMode::Now,
        )
        .expect("модификации");
        assert_eq!(mods.len(), 1);
        assert!(
            matches!(&mods[0], Mod::Delete(attr, values) if attr == "msLAPS-PasswordExpirationTime" && values.is_empty())
        );
    }

    #[test]
    fn mixed_domain_updates_both_families() {
        let mods = build_rotation_mods(
            &info(LapsFlavor::WindowsPlain, true, true, false, true),
            &RotationMode::Now,
        )
        .expect("модификации");
        assert_eq!(mods.len(), 2);
    }

    #[test]
    /// Будущая дата: FILETIME больше текущего и совпадает
    /// с результатом прямого преобразования той же строки.
    fn at_mode_writes_filetime() {
        let mods = build_rotation_mods(
            &info(LapsFlavor::Legacy, true, false, false, true),
            &RotationMode::At("2030-01-15T09:30".into()),
        )
        .expect("модификации");
        assert_eq!(mods.len(), 1);
        match &mods[0] {
            Mod::Replace(attr, values) => {
                assert_eq!(attr, "ms-Mcs-AdmPwdExpirationTime");
                let value = values.iter().next().expect("одно значение");
                let ft: i64 = value.parse().expect("число");
                assert!(ft > time::now_filetime(), "{ft}");
                assert_eq!(
                    ft,
                    time::parse_local_naive_to_filetime("2030-01-15T09:30").unwrap()
                );
            }
            other => panic!("ожидался Replace, получен {other:?}"),
        }
    }

    #[test]
    fn rejects_broken_or_absurd_dates() {
        let base = info(LapsFlavor::WindowsPlain, false, true, false, true);
        assert!(matches!(
            build_rotation_mods(&base, &RotationMode::At("не дата".into())),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            build_rotation_mods(&base, &RotationMode::At("2999-01-01T00:00".into())),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn refuses_when_rotation_not_possible() {
        let base = info(LapsFlavor::None, false, false, false, false);
        assert!(matches!(
            build_rotation_mods(&base, &RotationMode::Now),
            Err(AppError::NoLapsData(_))
        ));
    }
}

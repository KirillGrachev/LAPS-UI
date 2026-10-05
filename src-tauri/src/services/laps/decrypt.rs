//! Дешифровка `msLAPS-EncryptedPassword` (Windows LAPS) средствами Windows.
//!
//! Зашифрованный пароль Windows LAPS — это DPAPI-NG-блок (защита ключом
//! KDS домена). Система предоставляет штатный API расшифровки —
//! `NCryptUnprotectSecret`: провайдер KDS прозрачно для клиента получает
//! необходимый ключевой материал с контроллера домена (так же работают
//! gMSA-пароли на рядовых машинах), а доступ регулируется правами AD
//! (чтение атрибута + расширенное право дешифровки у субъекта).
//!
//! Поэтому нативному клиенту не нужен ни PowerShell, ни ручной RPC:
//! достаточно передать сырой блок атрибута в `NCryptUnprotectSecret`.
//! Результат — UTF-16 JSON того же формата, что и `msLAPS-Password`
//! (`{"n","t","p"}`), — переиспользует парсер открытого атрибута.
//!
//! Формат обёртки атрибута документирован скупо, поэтому блок пробуется
//! с нескольких вероятных offset'ов заголовка (0/4/8/12): успешный ответ
//! API — единственный достоверный критерий.

use windows::Win32::Security::Cryptography::{
    NCryptFreeBuffer, NCryptUnprotectSecret, NCRYPT_FLAGS,
};

use super::parse::{parse_windows_plain_json, WindowsPlainPassword};

/// Смещения начала DPAPI-NG-блока внутри значения атрибута
/// (версия/флаги заголовка Windows LAPS).
const BLOB_OFFSETS: [usize; 4] = [0, 4, 8, 12];

/// Минимально правдоподобный размер DPAPI-NG-блока.
const MIN_BLOB_LEN: usize = 64;

/// Пытается расшифровать значение `msLAPS-EncryptedPassword`.
///
/// `None` — дешифровка недоступна в этом окружении (нет прав у субъекта,
/// нет маршрута к DC, нестандартная обёртка): вызывающий код показывает
/// карточку без пароля с инструкцией, а не ошибку.
pub fn decrypt_windows_laps_password(blob: &[u8]) -> Option<WindowsPlainPassword> {
    for offset in BLOB_OFFSETS {
        let Some(candidate) = blob.get(offset..).filter(|part| part.len() >= MIN_BLOB_LEN) else {
            continue;
        };
        if let Some(plain) = try_unprotect(candidate) {
            return Some(plain);
        }
    }
    None
}

/// Буфер может быть не выровнен по u16 — копируем байты перед декодом.
///
/// # Safety
/// NCryptUnprotectSecret — штатный Win32 API (ncrypt.dll);
/// передаём неизменяемый буфер кандидата слайсом, принимаем выделенный
/// системой буфер и освобождаем его NCryptFreeBuffer в любом исходе.
/// Дескриптор провайдера и параметры памяти не нужны (NULL), окно
/// интерактивных подсказок не требуется.
fn try_unprotect(candidate: &[u8]) -> Option<WindowsPlainPassword> {
    let mut plain_ptr: *mut u8 = std::ptr::null_mut();
    let mut plain_len: u32 = 0;

    let ok = unsafe {
        NCryptUnprotectSecret(
            None,
            NCRYPT_FLAGS(0),
            candidate,
            None,
            None,
            &mut plain_ptr,
            &mut plain_len,
        )
        .is_ok()
    };
    if !ok || plain_ptr.is_null() || plain_len == 0 {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(plain_ptr, plain_len as usize) };
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let _ = unsafe { NCryptFreeBuffer(plain_ptr as *mut std::ffi::c_void) };

    let text = String::from_utf16_lossy(&words);
    parse_windows_plain_json(text.trim())
}

//! Предпроверка WebView2 Runtime до создания окна Tauri.
//!
//! Без WebView2 интерфейс приложения создать нечем: процесс жив, а окно
//! не появляется вовсе. До инициализации Tauri опрашивается реестр (три
//! расположения ключа клиента EdgeUpdate: per-machine, WOW6432Node и
//! per-user), и при отсутствии рантайма показывается нативный MessageBox
//! с предложением открыть официальную страницу загрузки.
//!
//! Инсталлятор поставки несёт встроенный бутстраппер WebView2
//! (`webviewInstallMode: embedBootstrapper`), ручная проверка нужна для
//! нестандартных окружений. Отключение — `KMLAPS_SKIP_WEBVIEW_CHECK=1`.

use windows::core::w;
use windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONWARNING, MB_YESNO, SW_SHOWNORMAL,
};

/// GUID клиента WebView2 Runtime в ключе EdgeUpdate.
const WEBVIEW2_CLIENTS_SUBKEY: &str =
    r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// Читает строковое значение `pv` (версия рантайма) из ключа клиента.
fn read_version(root: HKEY, subkey: &str) -> Option<String> {
    unsafe {
        let mut key = HKEY::default();
        let subkey = windows::core::HSTRING::from(subkey);
        if RegOpenKeyExW(root, &subkey, None, KEY_READ, &mut key).is_err() {
            return None;
        }
        let value = windows::core::HSTRING::from("pv");
        let mut size = 0u32;
        let probed = RegGetValueW(
            key,
            None,
            &value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        );
        if probed.is_err() || size == 0 {
            let _ = RegCloseKey(key);
            return None;
        }
        let mut buffer: Vec<u16> = vec![0; size as usize / 2 + 1];
        let read = RegGetValueW(
            key,
            None,
            &value,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if read.is_err() {
            return None;
        }
        let len = buffer
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..len]))
    }
}

/// Признак установленного WebView2 Runtime (любое из трёх расположений
/// ключа содержит непустую версию).
pub fn webview2_runtime_installed() -> bool {
    const WOW_SUBKEY: &str =
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    [
        read_version(HKEY_LOCAL_MACHINE, WEBVIEW2_CLIENTS_SUBKEY),
        read_version(HKEY_LOCAL_MACHINE, WOW_SUBKEY),
        read_version(HKEY_CURRENT_USER, WEBVIEW2_CLIENTS_SUBKEY),
    ]
    .iter()
    .any(|version| {
        version
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty() && v != "0.0.0.0")
    })
}

/// Нативный диалог: рантайм не найден, предложение открыть страницу загрузки.
pub fn report_missing_runtime() {
    const DOWNLOAD_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

    let text = w!(
        "Не найден Microsoft Edge WebView2 Runtime — без него интерфейс приложения не отобразится.\n\n\
         Отправить вас на официальную страницу загрузки WebView2 Runtime?\n\n\
         (В корпоративной сети установите Evergreen Bootstrapper или offline-инсталлятор \
         из внутреннего репозитория ПО.)"
    );
    let caption = w!("KMAruda LAPS — отсутствует WebView2 Runtime");
    unsafe {
        let answer = MessageBoxW(None, text, caption, MB_YESNO | MB_ICONWARNING);
        if answer == IDYES {
            let url = windows::core::HSTRING::from(DOWNLOAD_URL);
            let operation = w!("open");
            let _ = ShellExecuteW(None, operation, &url, None, None, SW_SHOWNORMAL);
        }
    }
}

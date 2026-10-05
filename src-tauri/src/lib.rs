//! Сборка Tauri-приложения: плагины, состояние, ужесточение webview,
//! страховка показа окна и реестр IPC-команд.
//!
//! `main.rs` намеренно тонкий — вся композиция здесь, в библиотеке
//! `app_lib`, чтобы её можно было переиспользовать в тестах
//! (архитектура — как в `kmaruda-phonebook`).

mod commands;
mod error;
mod services;
mod state;

use std::time::Duration;

use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use state::AppState;

/// Метка главного окна из `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// Запуск приложения: композиция плагинов, состояния и подготовка окна.
///
/// До инициализации Tauri проверяется наличие WebView2 Runtime — без него
/// окно создать нечем; при отсутствии показывается нативный диалог, проверка
/// отключается переменной `KMLAPS_SKIP_WEBVIEW_CHECK=1`. Плагин единственного
/// экземпляра регистрируется первым (требование плагина на Windows): второй
/// экземпляр не запускается — активируется существующее окно. Видимость окна
/// намеренно не восстанавливается из прошлой сессии: окно показывает фронтенд
/// после гидрации, а при сбое JS срабатывает страховочный таймер показа.
/// Плагин opener открывает внешние ссылки и папку журнала из Rust-команд
/// (фронтенд не имеет прямых разрешений плагинов — минимум поверхности).
/// В `setup`: разумный размер окна после аварийного восстановления,
/// отключение веб-поведений WebView (продукт — приложение, а не сайт),
/// бренд-иконка окна и страховка принудительного показа.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    if std::env::var_os("KMLAPS_SKIP_WEBVIEW_CHECK").is_none()
        && !services::webview_prereq::webview2_runtime_installed()
    {
        services::webview_prereq::report_missing_runtime();
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            info!("запуск второго экземпляра — активирую существующее окно");
            show_main_window(app);
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            init_tracing();
            info!(
                version = env!("CARGO_PKG_VERSION"),
                "запуск KMAruda LAPS (native LDAP)"
            );

            let state = AppState::init(app.handle())?;
            app.manage(state);

            sanitize_main_window_size(app);
            harden_main_webview(app);
            set_main_window_icon(app);
            spawn_window_failsafe(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::logout,
            commands::auth::session_status,
            commands::config::load_config,
            commands::config::save_config,
            commands::config::test_ldap_connection,
            commands::laps::search_computer,
            commands::laps::rotate_password,
            commands::laps::note_password_copied,
            commands::audit::list_audit,
            commands::audit::open_audit_folder,
            commands::system::app_environment,
            commands::system::open_external,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Метка времени логов в локальном поясе: `2026-10-05 13:42:07.682 +03:00`.
/// Дефолтный RFC3339-UTC с микросекундами неудобен глазами и не совпадает
/// с часами инженера и метками журнала аудита.
struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(
        &self,
        writer: &mut tracing_subscriber::fmt::format::Writer<'_>,
    ) -> std::fmt::Result {
        write!(
            writer,
            "{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z")
        )
    }
}

/// Подписчик логов: консоль и файл без ANSI-эскейпов (журнал читают и в
/// перенаправленном файле вида `kmaruda-laps.exe > log.txt`, где эскейпы —
/// мусор); модуль-источник пишется только в отладочной сборке.
fn init_tracing() {
    let default_filter = if cfg!(debug_assertions) {
        "app_lib=debug,info"
    } else {
        "info"
    };
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)))
        .with(
            tracing_subscriber::fmt::layer()
                .with_timer(LocalTimer)
                .with_ansi(false)
                .with_target(cfg!(debug_assertions)),
        )
        .init();
}

/// Минимальный и стандартный размеры главного окна (логические пиксели).
/// Держим синхронно с `minWidth/minHeight` и `width/height` в tauri.conf.json.
const MIN_WINDOW_WIDTH: u32 = 900;
const MIN_WINDOW_HEIGHT: u32 = 640;
const DEFAULT_WINDOW_WIDTH: u32 = 980;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

/// Приводит окно к допустимым размерам после восстановления состояния.
///
/// Плагин window-state может восстановить размер, сохранённый в сессии,
/// когда ограничения ещё не действовали (или сохранённый в физических
/// пикселях на другом DPI): ОС не применяет минимум к восстановленному
/// размеру, и окно открывается меньше допустимого, а первая же попытка
/// ресайза «выстреливает» его к минимуму без возможности вернуться.
/// Поэтому: (1) минимум выставляется рантаймом сразу, (2) восстановленный
/// размер сравнивается с минимумом В ЛОГИЧЕСКИХ пикселях (через scale
/// factor) и при нарушении заменяется стандартным с центрированием.
/// Окно всегда без системной рамки: управление — только наш TitleBar.
fn sanitize_main_window_size(app: &tauri::App) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let _ = window.set_decorations(false);
    let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize {
        width: MIN_WINDOW_WIDTH as f64,
        height: MIN_WINDOW_HEIGHT as f64,
    })));

    let scale = window.scale_factor().unwrap_or(1.0);
    if let Ok(size) = window.inner_size() {
        let logical_width = (size.width as f64 / scale).round() as u32;
        let logical_height = (size.height as f64 / scale).round() as u32;
        if logical_width < MIN_WINDOW_WIDTH || logical_height < MIN_WINDOW_HEIGHT {
            tracing::warn!(
                logical_width,
                logical_height,
                "восстановлен размер окна меньше допустимого — применяю стандартный"
            );
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: DEFAULT_WINDOW_WIDTH as f64,
                height: DEFAULT_WINDOW_HEIGHT as f64,
            }));
            let _ = window.center();
        }
    }
}

/// Отключение встроенных веб-поведений WebView2 (см. kmaruda-phonebook):
/// автозаполнение форм/паролей, зум, браузерные акселераторы, свайп-навигация,
/// статус-бар ссылок, контекстное меню браузера; DevTools — только в debug.
///
/// Trait `Interface` для `.cast()` берётся из `windows-core` той же версии (0.62),
/// на которой собран webview2-com-sys 0.39 (его использует wry/tauri). Импорт из
/// crate `windows` 0.61 дал бы ДРУГОЙ экземпляр трейта `Interface` —
/// и метод `.cast()` не нашёлся бы (ошибка E0599).
///
/// # Safety
/// Замыкание выполняется на главном потоке, пока WebView жив;
/// controller/CoreWebView2/Settings возвращают валидные COM-объекты,
/// вызовы только выставляют настройки интерфейса. Приведения к
/// Settings3..6 — проверенные QueryInterface, каждый результат
/// проверяется перед использованием.
#[cfg(target_os = "windows")]
fn harden_main_webview(app: &tauri::App) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Settings3, ICoreWebView2Settings4, ICoreWebView2Settings5,
        ICoreWebView2Settings6,
    };
    use windows_core::Interface;

    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        warn!("главное окно не найдено — веб-поведения не ужесточены");
        return;
    };
    let result = window.with_webview(|webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let Ok(settings) = core.Settings() else {
            return;
        };
        let _ = settings.SetIsStatusBarEnabled(false);
        let _ = settings.SetAreDefaultContextMenusEnabled(false);
        let _ = settings.SetIsZoomControlEnabled(false);
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings3>() {
            let _ = settings.SetAreBrowserAcceleratorKeysEnabled(false);
        }
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings4>() {
            let _ = settings.SetIsGeneralAutofillEnabled(false);
            let _ = settings.SetIsPasswordAutosaveEnabled(false);
        }
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings5>() {
            let _ = settings.SetIsPinchZoomEnabled(false);
        }
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings6>() {
            let _ = settings.SetIsSwipeNavigationEnabled(false);
        }
        #[cfg(not(debug_assertions))]
        let _ = settings.SetAreDevToolsEnabled(false);
    });
    if let Err(e) = result {
        warn!(error = %e, "не удалось ужесточить настройки webview");
    }
}

#[cfg(not(target_os = "windows"))]
fn harden_main_webview(_app: &tauri::App) {}

/// Иконка окна и кнопки таскбара — из бренд-файла, встроенного в бинарник.
fn set_main_window_icon(app: &tauri::App) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        warn!("главное окно не найдено — иконка не установлена");
        return;
    };
    let Ok(decoded) = image::load_from_memory(include_bytes!("../icons/icon.png")) else {
        warn!("встроенная иконка окна не читается");
        return;
    };
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();
    let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
    if let Err(e) = window.set_icon(icon) {
        warn!(error = %e, "не удалось установить иконку окна");
    }
}

fn spawn_window_failsafe(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(6));
        if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
            match window.is_visible() {
                Ok(false) => {
                    warn!("фронтенд не показал окно за 6 секунд — показываю принудительно");
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                Ok(true) => {}
                Err(e) => warn!(error = %e, "не удалось проверить видимость окна"),
            }
        }
    });
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

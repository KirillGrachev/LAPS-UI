//! Тонкая точка входа: вся композиция — в библиотеке `app_lib` (lib.rs).
//! `windows_subsystem = "windows"` прячет консоль в релизной сборке.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run()
}

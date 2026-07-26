#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    AppWindow::new()?.run()
}

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> Result<(), slint::PlatformError> {
    mhgu_editor::run()
}

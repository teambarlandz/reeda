//! Desktop binary entry point. The app logic lives in `crate::run()` (lib.rs).

// GUI app: no console window in release builds (keep it in debug for logs).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    reeda_ui::run();
}

//! Desktop diagnostic logger — writes to `%TEMP%\reeda.log`.
//!
//! Every startup step and key event is appended as a timestamped line.
//! A panic hook writes the panic message before the process aborts.
//! All writes are open-append-close (no shared file handle) so the panic
//! hook can write safely from any thread without lock poisoning concerns.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Initialise the desktop logger: resolve path, write header, install panic hook.
/// Safe to call once at `run()` entry; no-ops if the temp dir is unavailable.
pub fn init() {
    let path = std::env::temp_dir().join("reeda.log");
    let _ = LOG_PATH.set(path);

    log("--- startup ---");

    // Install panic hook that logs before aborting.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any>".into()
        };
        let loc = info.location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        log(format!("PANIC: {msg}{loc}"));
        // Also eprintln so debug builds still see it.
        eprintln!("PANIC: {msg}{loc}");
        default_hook(info);
    }));
}

/// Append a timestamped line to the log file.
pub fn log(msg: impl std::fmt::Display) {
    let Some(path) = LOG_PATH.get() else { return };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

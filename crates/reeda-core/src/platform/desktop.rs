use super::{LogLevel, Platform, PlatformError, PlatformResult};

/// Desktop stub platform for host-side development.
///
/// All operations return stubbed values or `NotSupported` so the workspace
/// compiles and runs on desktop without Android dependencies.
pub struct DesktopPlatform;

impl Default for DesktopPlatform {
    fn default() -> Self {
        Self
    }
}

impl Platform for DesktopPlatform {
    fn pick_file(&self, _mime_type: &str) -> PlatformResult<String> {
        // On desktop, we could use a native file dialog in the future.
        // For now, stub it.
        Err(PlatformError::NotSupported)
    }

    fn get_intent_data(&self) -> PlatformResult<Option<String>> {
        Ok(None)
    }

    fn request_permission(&self, _permission: &str) -> PlatformResult<bool> {
        // Desktop: all permissions granted by default.
        Ok(true)
    }

    fn start_narration_service(&self) -> PlatformResult<()> {
        Err(PlatformError::NotSupported)
    }

    fn stop_narration_service(&self) -> PlatformResult<()> {
        Err(PlatformError::NotSupported)
    }

    fn request_wake_lock(&self, _enable: bool) -> PlatformResult<()> {
        Err(PlatformError::NotSupported)
    }

    fn log(&self, level: LogLevel, tag: &str, message: &str) {
        let prefix = match level {
            LogLevel::Verbose => "V",
            LogLevel::Debug => "D",
            LogLevel::Info => "I",
            LogLevel::Warn => "W",
            LogLevel::Error => "E",
        };
        eprintln!("[{prefix}/{tag}] {message}");
    }
}

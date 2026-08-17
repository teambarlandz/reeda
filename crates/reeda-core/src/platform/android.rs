use super::{LogLevel, Platform, PlatformError, PlatformResult};

/// Android platform implementation (via JNI).
///
/// This is a placeholder for the real JNI-backed platform that will be
/// implemented in M0.6 / M5. For now it mirrors `DesktopPlatform` behavior
/// so the workspace compiles on all targets.
///
/// The real implementation bridges to:
/// - `android.speech.tts.TextToSpeech` (TTS)
/// - `Storage Access Framework` (file picker)
/// - `startForeground()` (narration service)
/// - `PowerManager.WakeLock` (screen-on)
///
/// See [docs/PLATFORM.md](../../docs/PLATFORM.md) and [ADR-008](../../docs/ADR.md).
pub struct AndroidPlatform;

impl Default for AndroidPlatform {
    fn default() -> Self {
        Self
    }
}

impl Platform for AndroidPlatform {
    fn pick_file(&self, _mime_type: &str) -> PlatformResult<String> {
        // TODO: JNI call to SAF ACTION_OPEN_DOCUMENT
        Err(PlatformError::NotSupported)
    }

    fn get_intent_data(&self) -> PlatformResult<Option<String>> {
        // TODO: JNI call to read intent data
        Ok(None)
    }

    fn request_permission(&self, _permission: &str) -> PlatformResult<bool> {
        // TODO: JNI call to requestPermissions
        Ok(true)
    }

    fn start_narration_service(&self) -> PlatformResult<()> {
        // TODO: JNI call to startForegroundService
        Ok(())
    }

    fn stop_narration_service(&self) -> PlatformResult<()> {
        // TODO: JNI call to stopService
        Ok(())
    }

    fn request_wake_lock(&self, _enable: bool) -> PlatformResult<()> {
        // TODO: JNI call to PowerManager.WakeLock
        Ok(())
    }

    fn log(&self, level: LogLevel, tag: &str, message: &str) {
        let prefix = match level {
            LogLevel::Verbose => "V",
            LogLevel::Debug => "D",
            LogLevel::Info => "I",
            LogLevel::Warn => "W",
            LogLevel::Error => "E",
        };
        // On Android this would go to __android_log_print / logcat.
        // For now, eprintln suffices for compilation testing.
        eprintln!("[{prefix}/{tag}] {message}");
    }
}

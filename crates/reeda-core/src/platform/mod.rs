/// Platform abstraction layer.
///
/// Each platform (Android, desktop stub) implements the `Platform` trait.
/// The UI and core code interact with platform services exclusively through
/// this trait, keeping platform-specific code isolated.
///
/// See [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) §6.
pub mod android;
pub mod desktop;

/// Result type for platform operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Errors that can occur from platform operations.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// The user cancelled the operation (e.g., SAF picker).
    #[error("cancelled by user")]
    Cancelled,

    /// The platform does not support this operation.
    #[error("not supported on this platform")]
    NotSupported,

    /// A permission was denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// An underlying platform error.
    #[error("platform error: {0}")]
    Other(String),
}

/// Platform services that the core and UI depend on.
///
/// Implemented by each platform (Android via JNI, desktop stub).
/// The trait is object-safe so `App` can hold a `Box<dyn Platform>`.
pub trait Platform {
    /// Open the SAF file picker and return the selected file URI.
    fn pick_file(&self, mime_type: &str) -> PlatformResult<String>;

    /// Get the URI data from an incoming intent (share/open).
    fn get_intent_data(&self) -> PlatformResult<Option<String>>;

    /// Request a runtime permission. Returns whether it was granted.
    fn request_permission(&self, permission: &str) -> PlatformResult<bool>;

    /// Start the narration foreground service.
    fn start_narration_service(&self) -> PlatformResult<()>;

    /// Stop the narration foreground service.
    fn stop_narration_service(&self) -> PlatformResult<()>;

    /// Request a wake lock (keep screen on during narration).
    fn request_wake_lock(&self, enable: bool) -> PlatformResult<()>;

    /// Log a message to the platform logcat / console.
    fn log(&self, level: LogLevel, tag: &str, message: &str);
}

/// Log levels matching Android logcat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Verbose / debug.
    Verbose,
    /// Debug messages.
    Debug,
    /// Informational.
    Info,
    /// Warnings.
    Warn,
    /// Errors.
    Error,
}

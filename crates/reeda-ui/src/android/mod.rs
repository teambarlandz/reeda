//! Android UI bridge stubs for reeda-ui.
//!
//! These are thin wrappers that the Slint UI layer calls on Android.
//! They delegate to `reeda_core::platform::Platform` (which currently
//! returns stubs). Real JNI integration lands in M5.
//!
//! All items in this module are gated behind `#[cfg(feature = "platform-android")]`.

use reeda_core::platform::{AndroidPlatform, Platform, PlatformResult};

/// Pick a file via SAF (Storage Access Framework).
///
/// Opens `ACTION_OPEN_DOCUMENT` and returns the content URI on success.
pub fn pick_file(mime_type: &str) -> PlatformResult<String> {
    AndroidPlatform::default().pick_file(mime_type)
}

/// Read the URI from an incoming intent (share / open-with).
///
/// Returns `None` if no intent data is available.
pub fn get_intent_data() -> PlatformResult<Option<String>> {
    AndroidPlatform::default().get_intent_data()
}

/// Request a runtime permission from the user.
///
/// Returns `true` if granted, `false` if denied.
pub fn request_permission(permission: &str) -> PlatformResult<bool> {
    AndroidPlatform::default().request_permission(permission)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_file_returns_not_supported() {
        assert!(pick_file("application/epub+zip").is_err());
    }

    #[test]
    fn get_intent_data_returns_none() {
        assert!(get_intent_data().unwrap().is_none());
    }

    #[test]
    fn request_permission_returns_true() {
        assert!(request_permission("android.permission.READ_EXTERNAL_STORAGE").unwrap());
    }
}

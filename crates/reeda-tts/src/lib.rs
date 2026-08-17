//! `reeda-tts` — read-aloud for Reeda.
//!
//! Planned modules (docs/TTS_SPEC.md): chunking with CFI mapping, the
//! narration state machine, audio focus, foreground-service media
//! notification, and the Android TextToSpeech JNI bridge (ADR-008).
//!
//! Current state: skeleton. The `platform-android` feature gates the JNI
//! bridge; `platform-desktop` (default) provides a stub host so the
//! workspace builds without the Android NDK.

#![deny(missing_docs)]

/// Returns the current reeda-tts crate version.
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_version_is_parseable_semver() {
        let v = super::crate_version();
        assert_eq!(v.split('.').count(), 3, "expected semver, got {v}");
    }
}

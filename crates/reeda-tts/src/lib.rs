//! `reeda-tts` — read-aloud for Reeda.
//!
//! Modules (docs/TTS_SPEC.md): chunking with CFI mapping (`chunk`, M5.1), the
//! narration state machine + host trait (`engine`, M5.2), and the Android
//! TextToSpeech JNI bridge (`android_bridge`, M5.5 — feature-gated).

#![deny(missing_docs)]

/// Narration chunking: sentence splitting + CFI-anchored chunks.
pub mod chunk;

/// Narration engine: state machine + host trait (`TtsHost`, `FakeTtsHost`).
pub mod engine;

/// Android TextToSpeech JNI bridge (`AndroidTtsHost`, feature-gated).
#[cfg(feature = "platform-android")]
pub mod android_bridge;

/// Returns the current reeda-tts crate version.
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

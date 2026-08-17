//! `reeda-core` — the application core of Reeda.
//!
//! Owns the domain models, the application state, the command bus
//! (UI → core) and the event stream (core → UI), plus the service layer
//! (library, reader sessions, annotations, import pipeline) and storage.
//!
//! The UI crate talks **only** to this crate (see docs/ARCHITECTURE.md §4.2).
//!
//! Current state: skeleton. Real services land per milestone
//! (docs/ROADMAP.md M0–M7).

#![deny(missing_docs)]

/// Returns the current reeda-core crate version.
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

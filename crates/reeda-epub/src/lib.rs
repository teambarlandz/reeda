//! `reeda-epub` — the EPUB parsing & rendering engine of Reeda.
//!
//! Planned modules (docs/EPUB_SPEC.md, docs/TECHNICAL_DESIGN.md §2.2):
//! container, OPF/nav parsing, XHTML → document model, CSS subset engine,
//! deterministic pagination, and CFI position handling.
//!
//! Current state: skeleton. Dependencies (zip, html5ever, roxmltree,
//! cssparser, …) are added when M1 work starts.

#![deny(missing_docs)]

/// Returns the current reeda-epub crate version.
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

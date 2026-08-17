//! `reeda-search` — full-text search for Reeda (Tantivy — ADR-009).
//!
//! Planned modules (docs/SEARCH_SPEC.md): index schema, analyzer selection
//! per language, background indexer with debounce, ranked queries with
//! per-book grouping and locator payloads.
//!
//! Current state: skeleton. `tantivy` is added in M4.

#![deny(missing_docs)]

/// Returns the current reeda-search crate version.
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

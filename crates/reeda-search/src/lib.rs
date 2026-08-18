//! `reeda-search` — full-text search for Reeda (Tantivy — ADR-009).
//!
//! Modules (docs/SEARCH_SPEC.md): index schema + lifecycle (`index`), query
//! layer with per-language analysis (`query`, M4.2), background indexer with
//! debounce (`indexer`, M4.3).

#![deny(missing_docs)]

pub mod index;

pub use index::{IndexManager, IndexedBlock, SearchHit, SearchResult, INDEX_VERSION};

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

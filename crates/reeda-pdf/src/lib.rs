//! `reeda-pdf` — the PDF engine of Reeda (PDFium via `pdfium-render`).
//!
//! Planned modules (docs/PDF_SPEC.md): document open/count, page
//! rasterization with LRU cache, outline extraction, theme filters.
//!
//! Current state: skeleton. `pdfium-render` and the libpdfium binary
//! strategy (docs/PDF_SPEC.md §7) are wired in M6.

#![deny(missing_docs)]

/// Returns the current reeda-pdf crate version.
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

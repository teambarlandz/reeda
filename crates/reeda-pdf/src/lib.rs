//! `reeda-pdf` — the PDF engine of Reeda (PDFium via `pdfium-render`).
//!
//! Modules (docs/PDF_SPEC.md): document model (`document`, M6.1),
//! page rasterization with LRU cache (`render` + `cache`, M6.2),
//! outline extraction and theme filters (M6.5).

#![deny(missing_docs)]

/// PDF document model: open, page count, page sizes (PDF_SPEC §2).
pub mod document;

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

//! PDF document model (PDF_SPEC §2): open, page count, page sizes.
//!
//! Opens a PDF via `pdfium-render` (PDFium), extracts metadata, and drops
//! the document handle. Rendering is handled separately by [`crate::render`].

use std::path::{Path, PathBuf};

use pdfium_render::prelude::Pdfium;

/// Errors from PDF document operations.
#[derive(Debug, Clone)]
pub enum PdfError {
    /// PDFium library could not be loaded (binary not on system path or
    /// `PDFIUM_LIBRARY_PATH`). Run `scripts/fetch_pdfium.ps1` to fetch the
    /// prebuilt binary.
    PdfiumUnavailable(String),
    /// PDFium loaded but the file could not be opened (corrupt, missing,
    /// encrypted with password, etc.).
    OpenFailed(String),
    /// The file opened but a page could not be rasterized (out-of-range
    /// page index, renderer error, etc.).
    RenderFailed(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PdfiumUnavailable(msg) => write!(f, "PDFium unavailable: {msg}"),
            Self::OpenFailed(msg) => write!(f, "PDF open failed: {msg}"),
            Self::RenderFailed(msg) => write!(f, "PDF render failed: {msg}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Process-wide PDFium session (PDFium is a global singleton in
/// `pdfium-render`, so it is initialized exactly once per process).
///
/// PDFium is located by:
/// 1. the `PDFIUM_LIBRARY_PATH` environment variable (if set), else
/// 2. the system library search path (`pdfium.dll` / `libpdfium.so`).
static PDFIUM: std::sync::OnceLock<Result<Pdfium, PdfError>> = std::sync::OnceLock::new();
static INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Get the process-wide [`Pdfium`] instance, initializing it on first call.
///
/// Never panics: unlike [`Pdfium::default`], failure to load the library is
/// reported as [`PdfError::PdfiumUnavailable`] instead of an unwrap panic.
/// Concurrent callers are serialized so `Pdfium::new`'s singleton assert
/// cannot be triggered by a race.
pub fn pdfium() -> Result<&'static Pdfium, &'static PdfError> {
    if let Some(cached) = PDFIUM.get() {
        return cached.as_ref();
    }
    let _guard = INIT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(cached) = PDFIUM.get() {
        return cached.as_ref();
    }
    PDFIUM.get_or_init(init_pdfium).as_ref()
}

fn init_pdfium() -> Result<Pdfium, PdfError> {
    // 1. Explicit path from the environment (scripts/fetch_pdfium.ps1
    //    prints the export line to use).
    if let Ok(path) = std::env::var("PDFIUM_LIBRARY_PATH") {
        if let Ok(bindings) = Pdfium::bind_to_library(&path) {
            return Ok(Pdfium::new(bindings));
        }
    }

    // 2. System library search path.
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => Err(PdfError::PdfiumUnavailable(format!(
            "could not load PDFium: {e}. Run scripts/fetch_pdfium.ps1 to fetch the prebuilt binary."
        ))),
    }
}

/// Metadata for an opened PDF document.
///
/// Stores page count and per-page sizes (in PDF points, 1 pt = 1/72 in).
/// The underlying PDFium document handle is dropped after extraction; for
/// rendering, use [`crate::render::render_page`].
#[derive(Debug, Clone)]
pub struct PdfDocument {
    /// Filesystem path to the PDF.
    path: PathBuf,
    /// Total number of pages.
    page_count: usize,
    /// Per-page `(width, height)` in PDF points (72 dpi).
    page_sizes: Vec<(f32, f32)>,
}

impl PdfDocument {
    /// Open a PDF file and extract page metadata.
    ///
    /// Requires the PDFium shared library to be available (see [`pdfium`]).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PdfError> {
        let pdfium = pdfium().map_err(Clone::clone)?;

        let doc = pdfium
            .load_pdf_from_file(path.as_ref(), None)
            .map_err(|e| PdfError::OpenFailed(e.to_string()))?;

        let pages = doc.pages();
        let page_count = pages.len() as usize;
        let page_sizes: Vec<(f32, f32)> = pages
            .iter()
            .map(|p| (p.width().value, p.height().value))
            .collect();

        Ok(Self {
            path: path.as_ref().to_path_buf(),
            page_count,
            page_sizes,
        })
    }

    /// Filesystem path to the PDF.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Total number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// `(width, height)` of page `index` in PDF points (72 dpi), or `None`
    /// if the index is out of range.
    pub fn page_size(&self, index: usize) -> Option<(f32, f32)> {
        self.page_sizes.get(index).copied()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal valid single-page PDF (US Letter, 612×792 pt).
    /// Byte offsets in the xref table have been verified.
    pub(crate) const ONE_PAGE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj
xref
0 4
0000000000 65535 f 
0000000010 00000 n 
0000000062 00000 n 
0000000121 00000 n 
trailer
<< /Size 4 /Root 1 0 R >>
startxref
193
%%EOF";

    /// Minimal valid two-page PDF (US Letter + A4 pages).
    pub(crate) const TWO_PAGE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj
4 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>
endobj
xref
0 5
0000000000 65535 f 
0000000010 00000 n 
0000000062 00000 n 
0000000121 00000 n 
0000000188 00000 n 
trailer
<< /Size 5 /Root 1 0 R >>
startxref
255
%%EOF";

    fn write_fixture(name: &str, bytes: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(bytes).expect("write");
        f.sync_all().expect("sync");
        dir
    }

    #[test]
    fn open_one_page_pdf() {
        let dir = write_fixture("one.pdf", ONE_PAGE_PDF);
        match PdfDocument::open(dir.path().join("one.pdf")) {
            Ok(doc) => {
                assert_eq!(doc.page_count(), 1);
                let (w, h) = doc.page_size(0).unwrap();
                assert!((w - 612.0).abs() < 1.0, "width ~612 pt, got {w}");
                assert!((h - 792.0).abs() < 1.0, "height ~792 pt, got {h}");
                assert!(doc.page_size(1).is_none());
            }
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping (run scripts/fetch_pdfium.ps1)");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn open_two_page_pdf() {
        let dir = write_fixture("two.pdf", TWO_PAGE_PDF);
        match PdfDocument::open(dir.path().join("two.pdf")) {
            Ok(doc) => {
                assert_eq!(doc.page_count(), 2);
                let (w1, h1) = doc.page_size(0).unwrap();
                assert!((w1 - 612.0).abs() < 1.0);
                assert!((h1 - 792.0).abs() < 1.0);
                let (w2, h2) = doc.page_size(1).unwrap();
                assert!((w2 - 595.0).abs() < 1.0, "A4 width ~595, got {w2}");
                assert!((h2 - 842.0).abs() < 1.0, "A4 height ~842, got {h2}");
            }
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn open_nonexistent_file_returns_error() {
        match PdfDocument::open("/nonexistent/file.pdf") {
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — cannot test open failure path");
            }
            Err(PdfError::OpenFailed(_)) => { /* expected */ }
            Err(PdfError::RenderFailed(e)) => panic!("unexpected render failure: {e}"),
            Ok(_) => panic!("should have failed"),
        }
    }

    #[test]
    fn open_empty_file_returns_error() {
        let dir = write_fixture("empty.pdf", b"");
        match PdfDocument::open(dir.path().join("empty.pdf")) {
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
            }
            Err(PdfError::OpenFailed(_)) => { /* expected */ }
            Err(PdfError::RenderFailed(e)) => panic!("unexpected render failure: {e}"),
            Ok(_) => panic!("should have failed"),
        }
    }
}

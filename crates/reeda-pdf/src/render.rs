//! Page rasterization (PDF_SPEC §3).
//!
//! Pages are rendered to RGBA bitmaps at `scale × 96 dpi` pixel
//! dimensions, capped at [`MAX_RENDER_DIMENSION`] pixels per axis.
//! The theme filter is applied at render time.

use std::path::Path;

use pdfium_render::prelude::*;

use crate::document::{pdfium, PdfError};
use crate::theme::{apply_theme_filter, Theme};

/// Memory guard: no bitmap axis may exceed this many pixels (PDF_SPEC §3).
pub const MAX_RENDER_DIMENSION: u32 = 4096;

/// Base resolution: 96 px per inch on a 72 pt PDF coordinate system.
pub const PIXELS_PER_POINT: f32 = 96.0 / 72.0;

/// A rasterized page in RGBA, row-major, top-to-bottom.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    /// Pixel width of the bitmap.
    pub width: u32,
    /// Pixel height of the bitmap.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA pixel data.
    pub rgba: Vec<u8>,
}

impl RenderedPage {
    /// Total memory footprint of the pixel buffer.
    pub fn size_bytes(&self) -> usize {
        self.rgba.len()
    }
}

/// Render `page_index` of the PDF at `path` at the given scale factor.
///
/// `scale` is the combined zoom factor (device pixel ratio × zoom ×
/// fit-to-width factor, PDF_SPEC §3) applied on top of the 96 dpi base
/// resolution. The resulting bitmap is theme-filtered before returning.
///
/// The document is (re)opened for each render; callers should cache the
/// resulting [`RenderedPage`]s (see [`crate::cache::RasterCache`]).
pub fn render_page(
    path: &Path,
    page_index: u32,
    scale: f32,
    theme: Theme,
) -> Result<RenderedPage, PdfError> {
    let pdfium = pdfium().map_err(Clone::clone)?;
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| PdfError::OpenFailed(e.to_string()))?;

    let page = doc
        .pages()
        .get(page_index as i32)
        .map_err(|e| PdfError::RenderFailed(format!("page {page_index}: {e}")))?;

    let (w_pt, h_pt) = (page.width().value, page.height().value);
    let (mut width, mut height) = (
        (w_pt * PIXELS_PER_POINT * scale).round().max(1.0) as u32,
        (h_pt * PIXELS_PER_POINT * scale).round().max(1.0) as u32,
    );

    // Aspect-preserving cap on the longest axis.
    let cap = MAX_RENDER_DIMENSION as f32;
    let shrink = (cap / width.max(height) as f32).min(1.0);
    width = (width as f32 * shrink).round() as u32;
    height = (height as f32 * shrink).round() as u32;

    let config = PdfRenderConfig::new()
        .set_target_width(width as i32)
        .set_target_height(height as i32)
        .set_reverse_byte_order(true);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| PdfError::RenderFailed(e.to_string()))?;

    let mut rgba = bitmap.as_rgba_bytes();
    debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
    apply_theme_filter(&mut rgba, theme);

    Ok(RenderedPage {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_PAGE_PDF: &[u8] = crate::document::tests::ONE_PAGE_PDF;

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("reeda-pdf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn renders_page_at_96_dpi_base_scale() {
        let path = write_temp("render-96dpi.pdf", ONE_PAGE_PDF);
        let page = match render_page(&path, 0, 1.0, Theme::Normal) {
            Ok(page) => page,
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
                return;
            }
            Err(e) => panic!("render failed: {e}"),
        };
        // US Letter: 612 x 792 pt -> 816 x 1056 px at 96 dpi.
        assert_eq!(page.width, 816);
        assert_eq!(page.height, 1056);
        assert_eq!(page.rgba.len(), (816 * 1056 * 4) as usize);
    }

    #[test]
    fn caps_render_dimension_at_4096_px() {
        let path = write_temp("render-cap.pdf", ONE_PAGE_PDF);
        let page = match render_page(&path, 0, 100.0, Theme::Normal) {
            Ok(page) => page,
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
                return;
            }
            Err(e) => panic!("render failed: {e}"),
        };
        assert_eq!(page.width.max(page.height), MAX_RENDER_DIMENSION);
        assert!(page.width <= MAX_RENDER_DIMENSION);
        assert!(page.height <= MAX_RENDER_DIMENSION);
    }

    #[test]
    fn night_theme_darkens_output() {
        let path = write_temp("render-night.pdf", ONE_PAGE_PDF);
        let normal = match render_page(&path, 0, 1.0, Theme::Normal) {
            Ok(page) => page,
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
                return;
            }
            Err(e) => panic!("render failed: {e}"),
        };
        let night = render_page(&path, 0, 1.0, Theme::Night).unwrap();
        assert_eq!(normal.rgba.len(), night.rgba.len());
        // A mostly-white page must get strictly darker under the night filter.
        assert!(
            night.rgba.iter().map(|&b| b as u64).sum::<u64>()
                < normal.rgba.iter().map(|&b| b as u64).sum::<u64>()
        );
    }

    #[test]
    fn missing_page_returns_render_failed() {
        let path = write_temp("render-missing.pdf", ONE_PAGE_PDF);
        match render_page(&path, 99, 1.0, Theme::Normal) {
            Err(PdfError::RenderFailed(_)) => {}
            Ok(_) => panic!("expected RenderFailed"),
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}

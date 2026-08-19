//! Document outline extraction (PDF_SPEC §2.2, M6.5).
//!
//! The PDF bookmarks tree is flattened into a pre-order list of
//! [`OutlineItem`]s carrying the section title, nesting depth, and target
//! page (when resolvable). Rendering and page navigation use the same
//! PDFium session as [`crate::document`].

use std::path::Path;

use pdfium_render::prelude::PdfBookmarks;

use crate::document::{pdfium, PdfError};

/// One entry in the document outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem {
    /// Section title.
    pub title: String,
    /// Zero-based target page index, or `None` if the bookmark's
    /// destination does not resolve to a page (e.g. an external link).
    pub page_index: Option<u32>,
    /// Nesting depth (0 = top-level section).
    pub depth: u32,
}

/// Extract the document outline (bookmarks tree) of the PDF at `path`.
///
/// Bookmarks are visited in pre-order: parents before their children,
/// siblings in document order. Bookmarks without a title are skipped;
/// bookmarks whose destination cannot be resolved to a page keep a `None`
/// target. A PDF with no bookmarks yields an empty list.
///
/// Requires the PDFium shared library (see [`pdfium`]).
pub fn extract_outline(path: impl AsRef<Path>) -> Result<Vec<OutlineItem>, PdfError> {
    let pdfium = pdfium().map_err(Clone::clone)?;
    let doc = pdfium
        .load_pdf_from_file(path.as_ref(), None)
        .map_err(|e| PdfError::OpenFailed(e.to_string()))?;

    Ok(flatten(doc.bookmarks()))
}

/// Depth-first pre-order walk of the bookmark tree (iterative, so deeply
/// nested outlines cannot overflow the stack).
fn flatten(bookmarks: &PdfBookmarks<'_>) -> Vec<OutlineItem> {
    let mut result = Vec::new();
    let mut stack = Vec::new();
    if let Some(root) = bookmarks.root() {
        stack.push((root, 0u32));
    }

    while let Some((bookmark, depth)) = stack.pop() {
        let page_index = bookmark
            .destination()
            .and_then(|dest| dest.page_index().ok())
            .map(|idx| idx as u32);

        if let Some(title) = bookmark.title() {
            result.push(OutlineItem {
                title,
                page_index,
                depth,
            });
        }

        // Depth-first pre-order: siblings are pushed first so the first
        // child (popped last) is visited before them.
        if let Some(sibling) = bookmark.next_sibling() {
            stack.push((sibling, depth));
        }
        if let Some(child) = bookmark.first_child() {
            stack.push((child, depth + 1));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("reeda-pdf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(bytes).expect("write");
        f.sync_all().expect("sync");
        path
    }

    /// Build a minimal valid PDF from `(object_number, content)` pairs,
    /// computing the xref table offsets programmatically.
    fn build_pdf(objects: &[(u32, &str)]) -> Vec<u8> {
        let mut out = b"%PDF-1.4\n".to_vec();
        let max = objects.iter().map(|(n, _)| *n).max().unwrap_or(0);
        let mut offsets = vec![0usize; max as usize + 1];
        for (num, content) in objects {
            offsets[*num as usize] = out.len();
            out.extend_from_slice(format!("{num} 0 obj\n{content}\nendobj\n").as_bytes());
        }
        let xref_offset = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", max + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF",
                max + 1
            )
            .as_bytes(),
        );
        out
    }

    fn outline_pdf() -> Vec<u8> {
        build_pdf(&[
            (1, "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>"),
            (2, "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (4, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (5, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (6, "<< /Type /Outlines /First 7 0 R /Last 11 0 R /Count 3 >>"),
            (
                7,
                "<< /Title (Chapter One) /Parent 6 0 R /Next 8 0 R /First 9 0 R /Last 10 0 R /Count 2 /Dest [3 0 R /Fit] >>",
            ),
            (
                8,
                "<< /Title (Chapter Two) /Parent 6 0 R /Prev 7 0 R /Next 11 0 R /Dest [4 0 R /Fit] >>",
            ),
            (
                9,
                "<< /Title (Section 1.1) /Parent 7 0 R /Next 10 0 R /Dest [4 0 R /Fit] >>",
            ),
            (10, "<< /Title (Section 1.2) /Parent 7 0 R /Prev 9 0 R >>"),
            (11, "<< /Title (Appendix) /Parent 6 0 R /Prev 8 0 R >>"),
        ])
    }

    #[test]
    fn extracts_preorder_outline_with_depth_and_pages() {
        let path = write_temp("outline.pdf", &outline_pdf());
        let items = match extract_outline(&path) {
            Ok(items) => items,
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        assert_eq!(
            items,
            vec![
                OutlineItem {
                    title: "Chapter One".into(),
                    page_index: Some(0),
                    depth: 0,
                },
                OutlineItem {
                    title: "Section 1.1".into(),
                    page_index: Some(1),
                    depth: 1,
                },
                OutlineItem {
                    title: "Section 1.2".into(),
                    page_index: None,
                    depth: 1,
                },
                OutlineItem {
                    title: "Chapter Two".into(),
                    page_index: Some(1),
                    depth: 0,
                },
                OutlineItem {
                    title: "Appendix".into(),
                    page_index: None,
                    depth: 0,
                },
            ]
        );
    }

    #[test]
    fn pdf_without_outline_returns_empty() {
        let path = write_temp("plain.pdf", crate::document::tests::TWO_PAGE_PDF);
        let items = match extract_outline(&path) {
            Ok(items) => items,
            Err(PdfError::PdfiumUnavailable(_)) => {
                eprintln!("PDFium not available — skipping");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!(items.is_empty());
    }
}

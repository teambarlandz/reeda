/// Markdown export of highlights & notes.
///
/// Format per HIGHLIGHTS_SPEC.md §4:
/// ```markdown
/// # Book Title — Highlights & Notes
/// ## Chapter 3
/// > "Passage text…"  *[p. 42]*
/// > **Note:** my thought
/// ```
///
/// Entries are grouped by chapter (spine order) and ordered by position.
use reeda_epub::cfi::{Cfi, CfiRange as EpubCfiRange};
use reeda_epub::document::DocumentModel;
use reeda_epub::selection::GlobalRange;

use crate::models::{Annotation, AnnotationKind, Book};

/// Build the Markdown export for a book's highlights and notes.
///
/// Bookmarks and deleted annotations are excluded. Chapters with no
/// annotations are skipped.
pub fn export_markdown(book: &Book, doc: &DocumentModel, annotations: &[Annotation]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {} — Highlights & Notes\n\n", book.title));

    // Group annotations by chapter title, ordered by first position.
    let mut groups: Vec<(String, Vec<&Annotation>, usize)> = Vec::new();
    for ann in annotations {
        if ann.deleted_at.is_some() || ann.kind == AnnotationKind::Bookmark {
            continue;
        }
        let (chapter, pos) = chapter_title_of(doc, ann);
        match groups.iter_mut().find(|(t, _, _)| *t == chapter) {
            Some((_, list, min)) => {
                *min = (*min).min(pos);
                list.push(ann);
            }
            None => groups.push((chapter, vec![ann], pos)),
        }
    }
    groups.sort_by_key(|(_, _, pos)| *pos);

    for (chapter, list, _) in &groups {
        out.push_str(&format!(
            "## {}\n\n",
            if chapter.is_empty() { "Book" } else { chapter }
        ));
        for ann in list {
            if let Some(snippet) = ann.snippet.as_ref() {
                if !snippet.is_empty() {
                    out.push_str(&format!("> \"{snippet}\"\n"));
                }
            }
            if let Some(note) = ann.text.as_ref() {
                if !note.is_empty() {
                    out.push_str(&format!("> **Note:** {note}\n"));
                }
            }
            out.push('\n');
        }
    }

    let empty_header = format!("# {} — Highlights & Notes\n\n", book.title);
    if out == empty_header {
        out.push_str("_No highlights or notes yet._\n");
    }

    out
}

/// Resolve the chapter title containing an annotation (best-effort).
/// Returns the title and the global block position of the annotation.
fn chapter_title_of(doc: &DocumentModel, ann: &Annotation) -> (String, usize) {
    let Some(cfi) = ann.cfi.as_ref() else {
        return (String::new(), usize::MAX);
    };
    let range = EpubCfiRange {
        start: Cfi(cfi.start.clone()),
        end: Cfi(cfi.end.clone()),
    };
    let Some(gr) = GlobalRange::from_cfi(&range, doc.chapters.len() as u32) else {
        return (String::new(), usize::MAX);
    };
    let title = doc
        .block_at(gr.block_start)
        .map(|(ch, _, _)| ch.title.clone())
        .unwrap_or_default();
    (title, gr.block_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BookFormat, CfiRange, HighlightColor};

    fn test_book() -> Book {
        Book::new(
            "Test Book".into(),
            BookFormat::Epub,
            "books/test/book.epub".into(),
            "abc".into(),
        )
    }

    fn test_doc() -> DocumentModel {
        use reeda_epub::document::{Block, Chapter, Inline};
        DocumentModel {
            chapters: vec![
                Chapter {
                    spine_index: 0,
                    title: "Chapter 1".into(),
                    href: "ch1.xhtml".into(),
                    blocks: vec![
                        Block::Paragraph(vec![Inline::Text("one".into())]),
                        Block::Paragraph(vec![Inline::Text("two".into())]),
                    ],
                },
                Chapter {
                    spine_index: 1,
                    title: "Chapter 2".into(),
                    href: "ch2.xhtml".into(),
                    blocks: vec![
                        Block::Paragraph(vec![Inline::Text("three".into())]),
                        Block::Paragraph(vec![Inline::Text("four".into())]),
                    ],
                },
            ],
        }
    }

    fn hl(chapter: usize, block: usize, snippet: &str) -> Annotation {
        // Each test chapter has 2 blocks → global block = chapter*2 + block.
        let global = chapter * 2 + block;
        Annotation::new_highlight(
            crate::models::BookId::new(),
            CfiRange::new(
                format!(
                    "epubcfi(/6/{}/!/4/{}:0)",
                    4 + chapter as u32 * 2,
                    2 + global as u32 * 2
                ),
                format!(
                    "epubcfi(/6/{}/!/4/{}:5)",
                    4 + chapter as u32 * 2,
                    2 + global as u32 * 2
                ),
            ),
            HighlightColor::Yellow,
            Some(snippet.to_string()),
        )
    }

    #[test]
    fn export_empty_book() {
        let doc = test_doc();
        let md = export_markdown(&test_book(), &doc, &[]);
        assert!(md.contains("# Test Book — Highlights & Notes"));
        assert!(md.contains("No highlights or notes yet"));
    }

    #[test]
    fn export_groups_by_chapter() {
        let doc = test_doc();
        let anns = vec![
            hl(1, 0, "second chapter snippet"),
            hl(0, 1, "first chapter snippet"),
        ];
        let md = export_markdown(&test_book(), &doc, &anns);
        let ch1 = md.find("## Chapter 1").unwrap();
        let ch2 = md.find("## Chapter 2").unwrap();
        assert!(ch1 < ch2, "chapters should be in spine order");
        assert!(md.contains("first chapter snippet"));
        assert!(md.contains("second chapter snippet"));
    }

    #[test]
    fn export_includes_notes() {
        let doc = test_doc();
        let mut ann = hl(0, 0, "passage");
        ann.text = Some("my thought".into());
        let md = export_markdown(&test_book(), &doc, &[ann]);
        assert!(md.contains("> \"passage\""));
        assert!(md.contains("**Note:** my thought"));
    }

    #[test]
    fn export_skips_bookmarks_and_deleted() {
        let doc = test_doc();
        let mut deleted = hl(0, 1, "gone");
        deleted.deleted_at = Some(chrono::Utc::now());
        let bm = Annotation::new_bookmark(crate::models::BookId::new(), "/6/4".into());
        let md = export_markdown(&test_book(), &doc, &[hl(0, 0, "kept"), deleted, bm]);
        assert!(md.contains("kept"));
        assert!(!md.contains("gone"));
    }
}

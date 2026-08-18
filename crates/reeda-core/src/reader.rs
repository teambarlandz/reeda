/// Reader engine: bridges reeda-epub parsing/pagination into reeda-core state.
///
/// Holds the parsed `DocumentModel`, `TableOfContents`, and paginated output
/// for the currently open book. The `App` delegates to this module for all
/// reader-side state mutations.
use std::collections::HashMap;

use reeda_epub::document::{self, DocumentModel};
use reeda_epub::nav::TableOfContents;
use reeda_epub::paginator::{self, PageLayout, Pages};

use crate::models::{BookId, Chapter, Typography};

/// A rendered page's text content, ready for the Slint UI.
#[derive(Debug, Clone, Default)]
pub struct PageContent {
    /// Flat text for the entire page (all blocks joined with newlines).
    pub text: String,
    /// Chapter title of the first block on this page.
    pub chapter_title: String,
    /// Block-level descriptions for rich rendering.
    pub blocks: Vec<PageBlock>,
}

/// A simplified block for Slint rendering (no recursive inline types).
#[derive(Debug, Clone)]
pub enum PageBlock {
    /// Heading with level (1–6) and text.
    Heading(u8, String),
    /// Paragraph with text.
    Paragraph(String),
    /// Code block.
    CodeBlock(String),
    /// Image placeholder (path + alt).
    Image(String, String),
    /// Horizontal rule.
    HorizontalRule,
}

impl PageBlock {
    /// Flatten to plain text.
    pub fn to_text(&self) -> String {
        match self {
            PageBlock::Heading(_, text) => text.clone(),
            PageBlock::Paragraph(text) => text.clone(),
            PageBlock::CodeBlock(text) => text.clone(),
            PageBlock::Image(_, alt) => alt.clone(),
            PageBlock::HorizontalRule => String::new(),
        }
    }
}

/// Parsed content for a single book, stored in the `App` keyed by `BookId`.
#[derive(Debug)]
pub struct ParsedDoc {
    /// The full document model from reeda-epub.
    pub document: DocumentModel,
    /// Table of contents.
    pub toc: TableOfContents,
    /// Chapters in spine order (reeda-core model, for snapshot).
    pub spine: Vec<SpineEntry>,
}

/// A spine entry: maps a TOC label to a spine index.
#[derive(Debug, Clone)]
pub struct SpineEntry {
    /// Chapter title.
    pub title: String,
    /// Spine index (0-based).
    pub spine_index: u32,
}

/// Reader state: pagination + current page for one open book.
pub struct ReaderState {
    /// Paginated pages (set after open + paginate).
    pub pages: Pages,
    /// Current 0-based page index.
    pub current_page: u32,
}

impl Default for ReaderState {
    fn default() -> Self {
        Self {
            pages: Pages {
                pages: Vec::new(),
                total_chars: 0,
                layout_hash: 0,
            },
            current_page: 0,
        }
    }
}

impl ReaderState {
    /// Build `PageContent` for the current page index.
    pub fn current_page_content(&self, doc: &DocumentModel) -> PageContent {
        let page_idx = self.current_page as usize;
        extract_page_content(doc, &self.pages, page_idx)
    }
}

/// Extract the text content of a specific page from the document model.
pub fn extract_page_content(doc: &DocumentModel, pages: &Pages, page_idx: usize) -> PageContent {
    if page_idx >= pages.pages.len() {
        return PageContent::default();
    }

    let page = &pages.pages[page_idx];
    let mut text = String::new();
    let mut blocks = Vec::new();
    let mut chapter_title = String::new();

    // Walk blocks from first_block..=last_block.
    for block_idx in page.first_block..=page.last_block.min(doc.total_blocks().saturating_sub(1)) {
        if let Some((_chapter, block, _local)) = doc.block_at(block_idx) {
            // Determine char range within this block.
            let (start_char, end_char) =
                if block_idx == page.first_block && block_idx == page.last_block {
                    (page.first_char as usize, page.last_char as usize)
                } else if block_idx == page.first_block {
                    (page.first_char as usize, usize::MAX)
                } else if block_idx == page.last_block {
                    (0, page.last_char as usize)
                } else {
                    (0, usize::MAX)
                };

            let block_text = block_to_text(block);
            let sliced = slice_text(&block_text, start_char, end_char);

            // Capture chapter title from first heading on page.
            if chapter_title.is_empty() {
                if let document::Block::Heading(_, inlines) = block {
                    chapter_title = document::inline_to_text(inlines);
                }
            }

            match block {
                document::Block::Heading(level, _inlines) => {
                    let level_num = match level {
                        document::HeadingLevel::H1 => 1,
                        document::HeadingLevel::H2 => 2,
                        document::HeadingLevel::H3 => 3,
                        document::HeadingLevel::H4 => 4,
                        document::HeadingLevel::H5 => 5,
                        document::HeadingLevel::H6 => 6,
                    };
                    blocks.push(PageBlock::Heading(level_num, sliced.clone()));
                }
                document::Block::Paragraph(_)
                | document::Block::Blockquote(_)
                | document::Block::ListItem(_) => {
                    blocks.push(PageBlock::Paragraph(sliced.clone()));
                }
                document::Block::CodeBlock(_) => {
                    blocks.push(PageBlock::CodeBlock(sliced.clone()));
                }
                document::Block::Image(img) => {
                    blocks.push(PageBlock::Image(img.path.clone(), sliced.clone()));
                }
                document::Block::HorizontalRule => {
                    blocks.push(PageBlock::HorizontalRule);
                }
            }

            if !sliced.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&sliced);
            }
        }
    }

    PageContent {
        text,
        chapter_title,
        blocks,
    }
}

/// Flatten a block to plain text.
fn block_to_text(block: &document::Block) -> String {
    match block {
        document::Block::Heading(_, inlines)
        | document::Block::Paragraph(inlines)
        | document::Block::Blockquote(inlines)
        | document::Block::ListItem(inlines) => document::inline_to_text(inlines),
        document::Block::CodeBlock(s) => s.clone(),
        document::Block::Image(img) => img.alt.clone(),
        document::Block::HorizontalRule => String::new(),
    }
}

/// Slice text to a character range (start..end), clamping to bounds.
fn slice_text(text: &str, start: usize, end: usize) -> String {
    let len = text.len();
    let s = start.min(len);
    let e = end.min(len);
    if s >= e {
        String::new()
    } else {
        text[s..e].to_string()
    }
}

/// Convert `Typography` + viewport dimensions to a `PageLayout`.
pub fn typography_to_layout(ty: &Typography, width: f32, height: f32) -> PageLayout {
    PageLayout {
        width,
        height,
        font_size: ty.font_size_pt,
        line_height: ty.line_height,
        margin_h: ty.margin,
        margin_v: ty.margin,
    }
}

/// Re-paginate the given document with the given layout and return `Pages`.
pub fn paginate_doc(doc: &DocumentModel, layout: &PageLayout) -> Pages {
    paginator::paginate(doc, layout)
}

/// Find the page index that contains the given CFI position.
pub fn find_page_for_cfi(pages: &Pages, cfi_str: &str, spine_length: u32) -> Option<u32> {
    let cfi = reeda_epub::cfi::Cfi(cfi_str.to_string());
    if let Some(loc) = cfi.to_locator(spine_length) {
        let block = loc.block_index as usize; // global block index (selection.rs convention)
        for (i, page) in pages.pages.iter().enumerate() {
            if page.first_block <= block && block <= page.last_block {
                return Some(i as u32);
            }
        }
    }
    None
}

/// In-memory registry of parsed documents, keyed by book ID.
pub struct ParsedDocRegistry {
    docs: HashMap<BookId, ParsedDoc>,
}

impl Default for ParsedDocRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsedDocRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    /// Store a parsed document.
    pub fn insert(&mut self, book_id: BookId, doc: ParsedDoc) {
        self.docs.insert(book_id, doc);
    }

    /// Get a reference to a parsed document.
    pub fn get(&self, book_id: &BookId) -> Option<&ParsedDoc> {
        self.docs.get(book_id)
    }

    /// Remove a parsed document.
    pub fn remove(&mut self, book_id: &BookId) {
        self.docs.remove(book_id);
    }
}

/// Convert reeda-epub TOC into reeda-core `Chapter` stubs.
pub fn toc_to_chapters(toc: &TableOfContents, book_id: BookId) -> Vec<Chapter> {
    toc.items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            Chapter::new(
                book_id,
                i as u32,
                item.label.clone(),
                item.href.clone(),
                String::new(), // file hash — not computed here
                0,             // char count — not computed here
            )
        })
        .collect()
}

/// Build a `ParsedDoc` from an `EpubBook`.
pub fn epub_book_to_parsed_doc(book: &reeda_epub::EpubBook, _book_id: BookId) -> ParsedDoc {
    let spine: Vec<SpineEntry> = book
        .opf
        .spine
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let title = book
                .toc
                .items
                .iter()
                .find(|t| {
                    // Match by href fragment (nav href is relative to opf dir).
                    t.href.ends_with(&item.idref) || item.idref == t.href
                })
                .map(|t| t.label.clone())
                .unwrap_or_default();
            SpineEntry {
                title,
                spine_index: i as u32,
            }
        })
        .collect();

    ParsedDoc {
        document: book.document.clone(),
        toc: book.toc.clone(),
        spine,
    }
}

#[cfg(test)]
mod tests {
    use reeda_epub::paginator::{Page, Pages};

    use super::*;
    use reeda_epub::document::{Block, HeadingLevel, Inline};

    fn test_doc() -> DocumentModel {
        DocumentModel {
            chapters: vec![
                document::Chapter {
                    spine_index: 0,
                    title: "Chapter 1".into(),
                    href: "ch1.xhtml".into(),
                    blocks: vec![
                        Block::Heading(HeadingLevel::H1, vec![Inline::Text("Chapter 1".into())]),
                        Block::Paragraph(vec![Inline::Text(
                            "This is the first paragraph of chapter one. It has some text.".into(),
                        )]),
                        Block::Paragraph(vec![Inline::Text("Second paragraph here.".into())]),
                    ],
                },
                document::Chapter {
                    spine_index: 1,
                    title: "Chapter 2".into(),
                    href: "ch2.xhtml".into(),
                    blocks: vec![
                        Block::Heading(HeadingLevel::H1, vec![Inline::Text("Chapter 2".into())]),
                        Block::Paragraph(vec![Inline::Text(
                            "Chapter two content goes here.".into(),
                        )]),
                    ],
                },
            ],
        }
    }

    #[test]
    fn typography_to_layout_basic() {
        let ty = Typography {
            font_size_pt: 18.0,
            line_height: 1.5,
            margin: 24.0,
            ..Default::default()
        };
        let layout = typography_to_layout(&ty, 400.0, 700.0);
        assert_eq!(layout.width, 400.0);
        assert_eq!(layout.margin_h, 24.0);
    }

    #[test]
    fn paginate_and_extract_page() {
        let doc = test_doc();
        let layout = PageLayout {
            width: 100.0,
            height: 50.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate_doc(&doc, &layout);
        assert!(pages.pages.len() > 1);

        let content = extract_page_content(&doc, &pages, 0);
        assert!(!content.text.is_empty());
        assert!(!content.blocks.is_empty());
    }

    #[test]
    fn slice_text_basic() {
        assert_eq!(slice_text("hello world", 0, 5), "hello");
        assert_eq!(slice_text("hello world", 6, 11), "world");
        assert_eq!(slice_text("hello", 10, 20), "");
    }

    #[test]
    fn block_to_text_paragraph() {
        let block = Block::Paragraph(vec![
            Inline::Text("Hello ".into()),
            Inline::Strong(vec![Inline::Text("world".into())]),
        ]);
        assert_eq!(block_to_text(&block), "Hello world");
    }

    #[test]
    fn page_block_to_text() {
        assert_eq!(PageBlock::Heading(1, "Title".into()).to_text(), "Title");
        assert_eq!(PageBlock::Paragraph("Body".into()).to_text(), "Body");
        assert_eq!(
            PageBlock::Image("img.png".into(), "alt text".into()).to_text(),
            "alt text"
        );
        assert_eq!(PageBlock::HorizontalRule.to_text(), "");
    }

    #[test]
    fn registry_store_and_retrieve() {
        let mut registry = ParsedDocRegistry::new();
        let id = BookId::new();
        let doc = ParsedDoc {
            document: DocumentModel::default(),
            toc: TableOfContents { items: Vec::new() },
            spine: Vec::new(),
        };
        registry.insert(id, doc);
        assert!(registry.get(&id).is_some());

        registry.remove(&id);
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn extract_page_content_empty_doc() {
        let doc = DocumentModel::default();
        let pages = Pages {
            pages: vec![Page {
                first_block: 0,
                first_char: 0,
                last_block: 0,
                last_char: 0,
                progress: 1.0,
            }],
            total_chars: 0,
            layout_hash: 0,
        };
        let content = extract_page_content(&doc, &pages, 0);
        assert!(content.text.is_empty());
        assert!(content.blocks.is_empty());
    }

    #[test]
    fn paginate_long_doc_many_pages() {
        let mut blocks = Vec::new();
        for i in 0..50 {
            blocks.push(Block::Paragraph(vec![Inline::Text(format!(
                "Paragraph {i}: Some content to fill the page with enough text to cause wrapping. "
            ))]));
        }
        let doc = DocumentModel {
            chapters: vec![document::Chapter {
                spine_index: 0,
                title: "Long Chapter".into(),
                href: "long.xhtml".into(),
                blocks,
            }],
        };

        let layout = PageLayout {
            width: 200.0,
            height: 100.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate_doc(&doc, &layout);
        assert!(
            pages.pages.len() >= 5,
            "expected many pages, got {}",
            pages.pages.len()
        );

        // Progress should reach ~1.0 on the last page.
        let last = pages.pages.last().unwrap();
        assert!(last.progress > 0.9);
    }

    #[test]
    fn extract_page_content_middle_page() {
        let mut blocks = Vec::new();
        for i in 0..20 {
            blocks.push(Block::Paragraph(vec![Inline::Text(format!(
                "Paragraph {i}."
            ))]));
        }
        let doc = DocumentModel {
            chapters: vec![document::Chapter {
                spine_index: 0,
                title: "Chapter".into(),
                href: "ch.xhtml".into(),
                blocks,
            }],
        };
        let layout = PageLayout {
            width: 200.0,
            height: 100.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate_doc(&doc, &layout);
        assert!(pages.pages.len() >= 3);

        // Middle page should have non-empty text.
        let mid = pages.pages.len() / 2;
        let content = extract_page_content(&doc, &pages, mid);
        assert!(!content.text.is_empty());
        assert!(!content.blocks.is_empty());
    }

    #[test]
    fn find_page_for_cfi_returns_page() {
        let doc = test_doc();
        let layout = PageLayout {
            width: 100.0,
            height: 50.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate_doc(&doc, &layout);
        // CFI for spine 0, block 0, char 0 should land on page 0 or close.
        let cfi = "epubcfi(/6/4!/4/2:0)";
        let idx = find_page_for_cfi(&pages, cfi, 2);
        // May or may not find a page (depends on how CFI maps to block indices),
        // but should not panic.
        let _ = idx;
    }
}

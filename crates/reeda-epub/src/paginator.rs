/// Deterministic paginator: splits a `DocumentModel` into pages.
///
/// Given a document, viewport dimensions, and typography settings, produces
/// an ordered list of `Page` descriptors. The paginator is a pure function:
/// identical inputs always produce identical `Pages`.
///
/// See [EPUB_SPEC.md section 6](../../docs/EPUB_SPEC.md).
use crate::cfi::{Cfi, Locator};
use crate::document::{DocumentModel, Inline};
use serde::{Deserialize, Serialize};

/// Viewport and typography configuration for pagination.
#[derive(Debug, Clone)]
pub struct PageLayout {
    /// Page width in logical pixels.
    pub width: f32,
    /// Page height in logical pixels.
    pub height: f32,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Line height multiplier (e.g., 1.5).
    pub line_height: f32,
    /// Horizontal margin in logical pixels.
    pub margin_h: f32,
    /// Vertical margin in logical pixels (top + bottom).
    pub margin_v: f32,
}

impl Default for PageLayout {
    fn default() -> Self {
        Self {
            width: 400.0,
            height: 700.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 24.0,
            margin_v: 24.0,
        }
    }
}

impl PageLayout {
    /// The effective text area width.
    pub fn text_width(&self) -> f32 {
        (self.width - self.margin_h * 2.0).max(1.0)
    }

    /// The effective text area height.
    pub fn text_height(&self) -> f32 {
        (self.height - self.margin_v * 2.0).max(1.0)
    }

    /// Estimated characters per line (rough heuristic).
    pub fn chars_per_line(&self) -> usize {
        let char_width = self.font_size * 0.6;
        (self.text_width() / char_width).floor().max(1.0) as usize
    }

    /// Estimated lines per page.
    pub fn lines_per_page(&self) -> usize {
        let line_px = self.font_size * self.line_height;
        (self.text_height() / line_px).floor().max(1.0) as usize
    }
}

/// A single page in the paginated output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Global block index of the first block on this page.
    pub first_block: usize,
    /// Character offset within the first block where this page starts.
    pub first_char: u32,
    /// Global block index of the last block on this page.
    pub last_block: usize,
    /// Character offset (exclusive) within the last block where this page ends.
    pub last_char: u32,
    /// Estimated progress percentage (0.0–1.0).
    pub progress: f32,
}

/// The complete paginated output.
#[derive(Debug, Clone)]
pub struct Pages {
    /// Ordered pages.
    pub pages: Vec<Page>,
    /// Total character count across all blocks.
    pub total_chars: usize,
    /// Layout used for pagination.
    pub layout_hash: u64,
}

/// Paginate a document model into pages.
///
/// This is a deterministic, pure function: same inputs always produce
/// the same `Pages`.
pub fn paginate(doc: &DocumentModel, layout: &PageLayout) -> Pages {
    let cpl = layout.chars_per_line();
    let lpp = layout.lines_per_page();
    let chars_per_page = cpl * lpp;

    // Flatten all blocks into a sequence of (global_index, char_count).
    let block_chars: Vec<(usize, usize)> = doc
        .chapters
        .iter()
        .flat_map(|ch| ch.blocks.iter())
        .enumerate()
        .map(|(i, block)| (i, count_block_chars(block)))
        .collect();

    let total_chars: usize = block_chars.iter().map(|(_, c)| *c).sum();

    let mut pages = Vec::new();
    let mut chars_remaining = total_chars;
    let mut global_block = 0usize;
    let mut char_in_block = 0u32;
    let mut chars_used = 0usize;

    while global_block < block_chars.len() {
        let (_, block_len) = block_chars[global_block];
        let available_in_block = block_len.saturating_sub(char_in_block as usize);

        if available_in_block == 0 && char_in_block == 0 {
            // Empty block — still gets a page entry.
            let progress = if total_chars > 0 {
                chars_used as f32 / total_chars as f32
            } else {
                1.0
            };
            pages.push(Page {
                first_block: global_block,
                first_char: 0,
                last_block: global_block,
                last_char: 0,
                progress,
            });
            global_block += 1;
            char_in_block = 0;
            chars_remaining = chars_remaining.saturating_sub(1);
            continue;
        }

        let mut page_chars_left = chars_per_page;
        let page_first_block = global_block;
        let page_first_char = char_in_block;
        let mut page_last_block = global_block;
        let mut page_last_char = char_in_block;

        while page_chars_left > 0 && global_block < block_chars.len() {
            let (_, block_len) = block_chars[global_block];
            let avail = block_len.saturating_sub(page_last_char as usize);
            let take = avail.min(page_chars_left);

            page_last_block = global_block;
            page_last_char += take as u32;
            page_chars_left -= take;
            chars_used += take;
            chars_remaining = chars_remaining.saturating_sub(take);

            if take < avail {
                // Page is full, but block continues on next page.
                break;
            }

            // Move to next block.
            global_block += 1;
            page_last_char = 0;
        }

        let progress = if total_chars > 0 {
            (total_chars - chars_remaining) as f32 / total_chars as f32
        } else {
            1.0
        };

        pages.push(Page {
            first_block: page_first_block,
            first_char: page_first_char,
            last_block: page_last_block,
            last_char: page_last_char,
            progress,
        });

        char_in_block = page_last_char;
        if page_last_char > 0 && global_block < block_chars.len() {
            let (_, block_len) = block_chars[global_block];
            if page_last_char as usize >= block_len {
                global_block += 1;
                char_in_block = 0;
            }
        }
    }

    // If doc was completely empty, produce one empty page.
    if pages.is_empty() {
        pages.push(Page {
            first_block: 0,
            first_char: 0,
            last_block: 0,
            last_char: 0,
            progress: 1.0,
        });
    }

    Pages {
        pages,
        total_chars,
        layout_hash: compute_layout_hash(layout),
    }
}

/// Find which page contains a given CFI locator.
pub fn page_containing(pages: &Pages, locator: &Locator) -> Option<usize> {
    let target_block = locator.block_index as usize;
    let target_char = locator.char_offset;

    for (i, page) in pages.pages.iter().enumerate() {
        if target_block >= page.first_block && target_block <= page.last_block {
            if target_block == page.first_block && target_char < page.first_char {
                continue;
            }
            if target_block == page.last_block && target_char >= page.last_char {
                continue;
            }
            return Some(i);
        }
        // If we've passed the target block, the target is on the previous page.
        if target_block < page.first_block {
            return Some(i.saturating_sub(1));
        }
    }
    // Last page.
    pages.pages.last().map(|_| pages.pages.len() - 1)
}

/// Create a CFI locator for the start of a given page.
pub fn cfi_of_page_start(pages: &Pages, page_idx: usize, _spine_length: u32) -> Cfi {
    if let Some(page) = pages.pages.get(page_idx) {
        let loc = Locator::new(0, page.first_block as u32, page.first_char);
        Cfi::from_locator(&loc)
    } else {
        Cfi::from_locator(&Locator::new(0, 0, 0))
    }
}

/// Count the approximate character content of a block.
fn count_block_chars(block: &crate::document::Block) -> usize {
    use crate::document::Block;
    match block {
        Block::Heading(_, inlines) => inline_char_count(inlines),
        Block::Paragraph(inlines) => inline_char_count(inlines),
        Block::Blockquote(inlines) => inline_char_count(inlines),
        Block::ListItem(inlines) => inline_char_count(inlines),
        Block::CodeBlock(text) => text.len(),
        Block::Image(_) => 1,
        Block::HorizontalRule => 0,
    }
}

/// Count characters in inline content.
fn inline_char_count(inlines: &[Inline]) -> usize {
    inlines.iter().map(count_single_inline).sum()
}

fn count_single_inline(inline: &Inline) -> usize {
    use crate::document::Inline;
    match inline {
        Inline::Text(t) => t.len(),
        Inline::Strong(c)
        | Inline::Emphasis(c)
        | Inline::Underline(c)
        | Inline::Strikethrough(c)
        | Inline::Sub(c)
        | Inline::Sup(c) => inline_char_count(c),
        Inline::Link { children, .. } => inline_char_count(children),
        Inline::Code(s) => s.len(),
        Inline::Break => 1,
    }
}

/// Simple hash of layout parameters for cache keying.
fn compute_layout_hash(layout: &PageLayout) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    layout.width.to_bits().hash(&mut hasher);
    layout.height.to_bits().hash(&mut hasher);
    layout.font_size.to_bits().hash(&mut hasher);
    layout.line_height.to_bits().hash(&mut hasher);
    layout.margin_h.to_bits().hash(&mut hasher);
    layout.margin_v.to_bits().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Block, Chapter, HeadingLevel, Inline};

    fn test_doc() -> DocumentModel {
        // Two chapters with simple paragraphs.
        let blocks1 = vec![
            Block::Heading(HeadingLevel::H1, vec![Inline::Text("Introduction".into())]),
            Block::Paragraph(vec![Inline::Text(
                "This is a test paragraph with some text content for pagination.".into(),
            )]),
            Block::Paragraph(vec![Inline::Text("Short.".into())]),
        ];
        let blocks2 = vec![
            Block::Heading(HeadingLevel::H1, vec![Inline::Text("Chapter Two".into())]),
            Block::Paragraph(vec![Inline::Text("Another paragraph here.".into())]),
        ];
        DocumentModel {
            chapters: vec![
                Chapter {
                    spine_index: 0,
                    title: "Introduction".into(),
                    href: "ch1.xhtml".into(),
                    blocks: blocks1,
                },
                Chapter {
                    spine_index: 1,
                    title: "Chapter Two".into(),
                    href: "ch2.xhtml".into(),
                    blocks: blocks2,
                },
            ],
        }
    }

    #[test]
    fn paginate_produces_pages() {
        let doc = test_doc();
        let layout = PageLayout::default();
        let pages = paginate(&doc, &layout);
        assert!(!pages.pages.is_empty());
    }

    #[test]
    fn single_page_for_short_doc() {
        let doc = test_doc();
        // Very large page should fit everything.
        let layout = PageLayout {
            width: 2000.0,
            height: 4000.0,
            font_size: 12.0,
            line_height: 1.5,
            margin_h: 10.0,
            margin_v: 10.0,
        };
        let pages = paginate(&doc, &layout);
        assert_eq!(pages.pages.len(), 1);
        assert!((pages.pages[0].progress - 1.0).abs() < 0.01);
    }

    #[test]
    fn multiple_pages_for_long_content() {
        let doc = test_doc();
        // Very small page to force multiple pages.
        let layout = PageLayout {
            width: 100.0,
            height: 40.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate(&doc, &layout);
        assert!(pages.pages.len() > 1);
    }

    #[test]
    fn page_containing_finds_correct_page() {
        let doc = test_doc();
        let layout = PageLayout {
            width: 2000.0,
            height: 4000.0,
            font_size: 12.0,
            line_height: 1.5,
            margin_h: 10.0,
            margin_v: 10.0,
        };
        let pages = paginate(&doc, &layout);
        let loc = Locator::new(0, 0, 0);
        let idx = page_containing(&pages, &loc);
        assert!(idx.is_some());
    }

    #[test]
    fn empty_doc_produces_one_page() {
        let doc = DocumentModel::default();
        let pages = paginate(&doc, &PageLayout::default());
        assert_eq!(pages.pages.len(), 1);
    }

    #[test]
    fn progress_monotonically_increases() {
        let doc = test_doc();
        let layout = PageLayout {
            width: 100.0,
            height: 50.0,
            font_size: 18.0,
            line_height: 1.5,
            margin_h: 4.0,
            margin_v: 4.0,
        };
        let pages = paginate(&doc, &layout);
        for window in pages.pages.windows(2) {
            assert!(window[1].progress >= window[0].progress);
        }
    }
}

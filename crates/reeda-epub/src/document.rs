/// Document model: the intermediate representation of EPUB content.
///
/// After XHTML parsing and CSS cascade, content is represented as a
/// `DocumentModel` — an ordered list of chapters, each containing a
/// sequence of typed blocks with optional inline markup.
///
/// See [EPUB_SPEC.md §3](../../docs/EPUB_SPEC.md).
use serde::{Deserialize, Serialize};

/// A complete document (one per open book).
#[derive(Debug, Clone, Default)]
pub struct DocumentModel {
    /// Ordered chapters (matches spine order).
    pub chapters: Vec<Chapter>,
}

/// A single chapter's content.
#[derive(Debug, Clone, Default)]
pub struct Chapter {
    /// Spine index (0-based).
    pub spine_index: u32,
    /// Chapter title (from TOC or first heading).
    pub title: String,
    /// The resolved href of the XHTML source.
    pub href: String,
    /// Block-level elements in reading order.
    pub blocks: Vec<Block>,
}

/// A block-level element.
#[derive(Debug, Clone)]
pub enum Block {
    /// A paragraph or heading.
    Heading(HeadingLevel, Vec<Inline>),
    /// A paragraph of text.
    Paragraph(Vec<Inline>),
    /// A blockquote.
    Blockquote(Vec<Inline>),
    /// A list item.
    ListItem(Vec<Inline>),
    /// A code block (pre).
    CodeBlock(String),
    /// An image.
    Image(ImageRef),
    /// A horizontal rule.
    HorizontalRule,
}

/// Heading levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingLevel {
    /// h1
    H1,
    /// h2
    H2,
    /// h3
    H3,
    /// h4
    H4,
    /// h5
    H5,
    /// h6
    H6,
}

/// An inline element (runs of text with styling).
#[derive(Debug, Clone)]
pub enum Inline {
    /// Plain text.
    Text(String),
    /// Bold text.
    Strong(Vec<Inline>),
    /// Italic text.
    Emphasis(Vec<Inline>),
    /// Underlined text.
    Underline(Vec<Inline>),
    /// Strikethrough.
    Strikethrough(Vec<Inline>),
    /// Inline code.
    Code(String),
    /// A hyperlink.
    Link {
        /// Target href.
        href: String,
        /// Link text content.
        children: Vec<Inline>,
    },
    /// Subscript.
    Sub(Vec<Inline>),
    /// Superscript.
    Sup(Vec<Inline>),
    /// A line break.
    Break,
}

/// A reference to an image resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRef {
    /// Resolved path within the EPUB container.
    pub path: String,
    /// Optional alt text.
    pub alt: String,
    /// CSS-specified width (if any).
    pub width: Option<String>,
    /// CSS-specified height (if any).
    pub height: Option<String>,
}

impl DocumentModel {
    /// Total number of blocks across all chapters.
    pub fn total_blocks(&self) -> usize {
        self.chapters.iter().map(|c| c.blocks.len()).sum()
    }

    /// Get a block by its global (document-wide) index.
    pub fn block_at(&self, index: usize) -> Option<(&Chapter, &Block, usize)> {
        let mut offset = 0;
        for chapter in &self.chapters {
            let end = offset + chapter.blocks.len();
            if index < end {
                let local = index - offset;
                return Some((chapter, &chapter.blocks[local], local));
            }
            offset = end;
        }
        None
    }

    /// Plain text of the block at the global index (used for search indexing).
    pub fn block_text(&self, index: usize) -> Option<String> {
        self.block_at(index)
            .map(|(_chapter, block, _local)| block_text(block))
    }
}

/// Extract plain text from a block (headings, paragraphs, alt text, code).
pub fn block_text(block: &Block) -> String {
    match block {
        Block::Heading(_, inlines)
        | Block::Paragraph(inlines)
        | Block::Blockquote(inlines)
        | Block::ListItem(inlines) => inline_to_text(inlines),
        Block::CodeBlock(s) => s.clone(),
        Block::Image(img) => img.alt.clone(),
        Block::HorizontalRule => String::new(),
    }
}

/// Extract plain text from a sequence of inline elements.
///
/// Adjacent word-like runs (e.g. `More <em>content</em> here.`) get a
/// separating space; punctuation boundaries stay tight.
pub fn inline_to_text(inline: &[Inline]) -> String {
    let mut result = String::new();
    for item in inline {
        let piece = match item {
            Inline::Text(t) => t.clone(),
            Inline::Strong(children)
            | Inline::Emphasis(children)
            | Inline::Underline(children)
            | Inline::Strikethrough(children)
            | Inline::Sub(children)
            | Inline::Sup(children) => inline_to_text(children),
            Inline::Link { children, .. } => inline_to_text(children),
            Inline::Code(s) => s.clone(),
            Inline::Break => "\n".into(),
        };
        let prev_word = result
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric);
        let next_word = piece.chars().next().is_some_and(char::is_alphanumeric);
        if prev_word && next_word {
            result.push(' ');
        }
        result.push_str(&piece);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_total_blocks() {
        let doc = DocumentModel {
            chapters: vec![
                Chapter {
                    spine_index: 0,
                    title: "Ch1".into(),
                    href: "ch1.xhtml".into(),
                    blocks: vec![Block::Paragraph(vec![Inline::Text("Hello".into())])],
                },
                Chapter {
                    spine_index: 1,
                    title: "Ch2".into(),
                    href: "ch2.xhtml".into(),
                    blocks: vec![Block::Paragraph(vec![]), Block::Paragraph(vec![])],
                },
            ],
        };
        assert_eq!(doc.total_blocks(), 3);
    }

    #[test]
    fn block_at_index() {
        let doc = DocumentModel {
            chapters: vec![
                Chapter {
                    spine_index: 0,
                    title: "Ch1".into(),
                    href: "ch1.xhtml".into(),
                    blocks: vec![Block::Heading(
                        HeadingLevel::H1,
                        vec![Inline::Text("Title".into())],
                    )],
                },
                Chapter {
                    spine_index: 1,
                    title: "Ch2".into(),
                    href: "ch2.xhtml".into(),
                    blocks: vec![Block::Paragraph(vec![Inline::Text("Body".into())])],
                },
            ],
        };
        let (ch, block, local) = doc.block_at(1).unwrap();
        assert_eq!(ch.spine_index, 1);
        assert_eq!(local, 0);
        assert!(matches!(block, Block::Paragraph(_)));
    }

    #[test]
    fn inline_to_text_basic() {
        let inlines = vec![
            Inline::Text("Hello ".into()),
            Inline::Strong(vec![Inline::Text("world".into())]),
        ];
        assert_eq!(inline_to_text(&inlines), "Hello world");
    }
}

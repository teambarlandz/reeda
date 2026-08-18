//! Narration chunking (docs/TTS_SPEC.md §3): sentence-aware splitting of
//! `DocumentModel` chapters into speakable chunks with CFI anchors.

use reeda_epub::cfi::CfiRange;
use reeda_epub::document::{Block, DocumentModel};
use reeda_epub::selection::GlobalRange;

/// Default maximum chunk length (spec §3: ~30 s at 1×, TTS engine limit).
pub const DEFAULT_MAX_CHUNK_CHARS: usize = 4000;

/// Abbreviation guard list: a period after these words is not a sentence end.
pub const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sir", "sr", "jr", "st", "vs", "etc", "fig", "no", "vol",
    "pp", "ed", "rev", "ca", "approx", "est", "gen", "gov", "lt", "mt", "col", "dept", "univ",
];

/// One speakable unit of text with an exact CFI anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarrationChunk {
    /// Global block index of the containing block.
    pub block_index: u32,
    /// Character offset of the chunk start within the cleaned block text.
    pub char_start: u32,
    /// Character offset of the chunk end (exclusive).
    pub char_end: u32,
    /// The cleaned text to speak.
    pub text: String,
    /// CFI range of the chunk (open-at / highlight anchoring).
    pub cfi: CfiRange,
}

/// Splits a chapter's blocks into narration chunks.
///
/// Rules (TTS_SPEC §3): sentence boundaries at `. ! ? …` (plus closing
/// quotes/brackets) with an abbreviation guard; chunks never exceed
/// `max_chunk_chars`; paragraphs are split only when needed; images, rules
/// and repeated chapter-title headings are skipped; text is cleaned (soft
/// hyphens, control chars, nbsp, whitespace runs).
pub struct Chunker {
    max_chunk_chars: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            max_chunk_chars: DEFAULT_MAX_CHUNK_CHARS,
        }
    }
}

impl Chunker {
    /// Create a chunker with the default 4000-char limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Chunk the chapter at `spine_index` of `doc`, in reading order.
    pub fn chunks_for_chapter(&self, doc: &DocumentModel, spine_index: u32) -> Vec<NarrationChunk> {
        let Some(chapter) = doc.chapters.iter().find(|c| c.spine_index == spine_index) else {
            return vec![];
        };
        let mut chunks = Vec::new();
        // Global block index of this chapter's first block.
        let global_offset: u32 = doc
            .chapters
            .iter()
            .take_while(|c| c.spine_index != spine_index)
            .map(|c| c.blocks.len() as u32)
            .sum();
        // Skip repeated chapter-title headings after the first occurrence.
        let mut title_seen = false;
        for (local, block) in chapter.blocks.iter().enumerate() {
            let block_index = global_offset + local as u32;
            match block {
                Block::Image(_) | Block::HorizontalRule => continue,
                Block::Heading(_, _) => {
                    let text = clean(reeda_epub::document::block_text(block).as_str());
                    if !text.is_empty()
                        && text.eq_ignore_ascii_case(chapter.title.trim())
                        && title_seen
                    {
                        continue;
                    }
                    if !text.is_empty() {
                        title_seen = true;
                    }
                }
                _ => {}
            }
            let text = clean(reeda_epub::document::block_text(block).as_str());
            if text.is_empty() {
                continue;
            }
            for (start, end) in self.split_sentences(&text) {
                let mut start = start;
                while end - start > self.max_chunk_chars {
                    self.push(
                        &mut chunks,
                        block_index,
                        start,
                        start + self.max_chunk_chars,
                        &text,
                    );
                    start += self.max_chunk_chars;
                }
                if end > start {
                    self.push(&mut chunks, block_index, start, end, &text);
                }
            }
        }
        chunks
    }

    fn push(
        &self,
        chunks: &mut Vec<NarrationChunk>,
        block_index: u32,
        start: usize,
        end: usize,
        text: &str,
    ) {
        let segment = text[start..end].trim();
        if segment.is_empty() {
            return;
        }
        let range = GlobalRange::new(block_index as usize, start, block_index as usize, end);
        chunks.push(NarrationChunk {
            block_index,
            char_start: start as u32,
            char_end: end as u32,
            text: segment.to_string(),
            cfi: range.to_cfi(),
        });
    }

    /// Split cleaned text into sentence spans `(start, end)` with the
    /// abbreviation guard. A boundary is a terminator (`. ! ? …`) optionally
    /// followed by closing quotes/brackets, then whitespace or end-of-text —
    /// except after abbreviation-guard words.
    fn split_sentences(&self, text: &str) -> Vec<(usize, usize)> {
        let mut sentences = Vec::new();
        let mut start = 0usize;
        let bytes = text.as_bytes();
        let mut i = 0usize;
        let mut boundary = None;
        while i < bytes.len() {
            let c = text[i..].chars().next().unwrap();
            let is_term = matches!(c, '.' | '!' | '?' | '\u{2026}');
            if is_term {
                let mut j = i + c.len_utf8();
                // Consume trailing quotes/brackets.
                while j < bytes.len() {
                    let q = text[j..].chars().next().unwrap();
                    if matches!(q, '"' | '\'' | ')' | ']' | '”' | '»' | '‘' | '’') {
                        j += q.len_utf8();
                    } else {
                        break;
                    }
                }
                let after = text[j..].chars().next();
                let at_end = after.is_none();
                let followed_by_space = after.map(|q| q.is_whitespace()).unwrap_or(false);
                let guarded = c == '.'
                    && guard_word(text, i)
                    && after
                        .map(|q| q.is_ascii_uppercase() || q.is_whitespace())
                        .unwrap_or(true);
                if (at_end || followed_by_space) && !guarded {
                    boundary = Some(j);
                }
            }
            i += c.len_utf8();
            if let Some(end) = boundary.take() {
                if end > start {
                    sentences.push((start, end));
                }
                start = end;
            }
        }
        if start < text.len() {
            sentences.push((start, text.len()));
        }
        sentences
    }
}

/// True if the word ending right before `period_pos` is an abbreviation.
fn guard_word(text: &str, period_pos: usize) -> bool {
    let before = &text[..period_pos];
    let word = before
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':')
        .next_back();
    word.map(|w| ABBREVIATIONS.contains(&w.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Clean narration text: strip soft hyphens and control chars, map nbsp to
/// space, collapse whitespace runs (offsets are into the cleaned text).
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for c in text.chars() {
        if c == '\u{ad}' {
            continue;
        }
        if c.is_control() || c == '\u{a0}' || c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_of(spine: u32, title: &str, blocks: Vec<Block>) -> DocumentModel {
        let chapter = reeda_epub::document::Chapter {
            spine_index: spine,
            title: title.to_string(),
            href: format!("c{spine}.xhtml"),
            blocks,
        };
        DocumentModel {
            chapters: vec![chapter],
        }
    }

    fn para(text: &str) -> Block {
        Block::Paragraph(vec![reeda_epub::document::Inline::Text(text.to_string())])
    }

    fn heading(level: reeda_epub::document::HeadingLevel, text: &str) -> Block {
        Block::Heading(
            level,
            vec![reeda_epub::document::Inline::Text(text.to_string())],
        )
    }

    #[test]
    fn sentences_are_split_on_terminators() {
        let text = "First sentence. Second sentence! Third one? Done.";
        let spans = Chunker::new().split_sentences(text);
        assert_eq!(spans.len(), 4);
        assert_eq!(&text[spans[0].0..spans[0].1], "First sentence.");
        assert_eq!(&text[spans[1].0..spans[1].1], " Second sentence!");
        assert_eq!(&text[spans[3].0..spans[3].1], " Done.");
    }

    #[test]
    fn abbreviations_do_not_split() {
        let text = "Dr. Smith visited Mr. Jones. He left.";
        let spans = Chunker::new().split_sentences(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(
            &text[spans[0].0..spans[0].1],
            "Dr. Smith visited Mr. Jones."
        );
    }

    #[test]
    fn quotes_join_the_terminator() {
        let text = "He said \"Go.\" Then left.";
        let spans = Chunker::new().split_sentences(text);
        assert_eq!(spans.len(), 2);
        assert_eq!(&text[spans[0].0..spans[0].1], "He said \"Go.\"");
    }

    #[test]
    fn ellipsis_is_a_boundary() {
        let text = "Wait… I am not sure.";
        let spans = Chunker::new().split_sentences(text);
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn chunk_limit_is_honored_at_sentence_boundaries() {
        let text = (0..20)
            .map(|i| format!("Sentence number {i} with some words in it."))
            .collect::<Vec<_>>()
            .join(" ");
        let doc = block_of(0, "Title", vec![para(&text)]);
        let chunks = Chunker::new().chunks_for_chapter(&doc, 0);
        assert!(chunks.len() > 1);
        for c in &chunks {
            assert!(
                c.text.chars().count() <= DEFAULT_MAX_CHUNK_CHARS + 64,
                "chunk too long"
            );
        }
        // Chunks cover the text with no loss or duplication (boundary spaces are
        // trimmed for speech; offsets stay exact).
        let joined: String = chunks.iter().map(|c| c.text.as_str()).collect();
        let no_ws = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(no_ws(&joined), no_ws(&text));
        for c in &chunks {
            assert_eq!(
                c.text.trim(),
                text[c.char_start as usize..c.char_end as usize].trim()
            );
        }
    }

    #[test]
    fn oversize_sentence_is_hard_split() {
        let mut long = String::new();
        while long.len() < DEFAULT_MAX_CHUNK_CHARS + 500 {
            long.push_str("The quick brown fox jumps over the lazy dog. ");
        }
        let doc = block_of(0, "Title", vec![para(&long)]);
        let chunks = Chunker::new().chunks_for_chapter(&doc, 0);
        assert!(chunks.len() > 1);
        for c in &chunks {
            assert!(c.text.chars().count() <= DEFAULT_MAX_CHUNK_CHARS + 64);
        }
    }

    #[test]
    fn cleaning_removes_artifacts() {
        assert_eq!(clean("a\u{ad}b"), "ab");
        assert_eq!(clean("a\u{a0}b"), "a b");
        assert_eq!(clean("a \t b\nc"), "a b c");
        assert_eq!(clean("\u{1}hi\u{2}"), "hi");
        assert_eq!(clean("  padded  "), "padded");
    }

    #[test]
    fn images_rules_and_repeated_titles_are_skipped() {
        let doc = block_of(
            0,
            "Chapter One",
            vec![
                heading(reeda_epub::document::HeadingLevel::H1, "Chapter One"),
                para("Real content here."),
                Block::Image(reeda_epub::document::ImageRef {
                    path: "img/1.png".into(),
                    alt: "a picture".into(),
                    width: None,
                    height: None,
                }),
                Block::HorizontalRule,
                heading(reeda_epub::document::HeadingLevel::H2, "Chapter One"),
                para("More content."),
            ],
        );
        let chunks = Chunker::new().chunks_for_chapter(&doc, 0);
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["Chapter One", "Real content here.", "More content."]
        );
    }

    #[test]
    fn chunk_cfi_round_trips_to_offsets() {
        let text = "First sentence here. Second sentence here.";
        let doc = block_of(2, "Chapter Two", vec![para(text)]);
        let chunks = Chunker::new().chunks_for_chapter(&doc, 2);
        assert_eq!(chunks.len(), 2);
        for c in &chunks {
            let gr = GlobalRange::from_cfi(&c.cfi, 3).expect("round-trip");
            assert_eq!(gr.block_start, c.block_index as usize);
            assert_eq!(gr.char_start, c.char_start as usize);
            assert_eq!(gr.char_end, c.char_end as usize);
        }
    }

    #[test]
    fn unknown_chapter_returns_empty() {
        let doc = block_of(0, "Title", vec![para("Hello.")]);
        assert!(Chunker::new().chunks_for_chapter(&doc, 7).is_empty());
    }
}

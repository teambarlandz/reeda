//! M7d performance benchmarks for EPUB pagination (PERFORMANCE.md §1, §3).
//!
//! Measures the desktop-measurable re-pagination budget on synthetic
//! documents (avg chapter < 150 ms p95, long chapter < 400 ms p95 on device).
//! Run with `cargo test --release -p reeda-epub --test perf_bench -- --nocapture`
//! for realistic numbers; debug builds stay under generous smoke thresholds.

use std::time::{Duration, Instant};

use reeda_epub::document::{Block, Chapter, DocumentModel, Inline};
use reeda_epub::paginator::{paginate, PageLayout};

const WORDS: &[&str] = &[
    "journey",
    "river",
    "forest",
    "morning",
    "window",
    "silence",
    "shadow",
    "stone",
    "garden",
    "letter",
    "winter",
    "summer",
    "bridge",
    "mirror",
    "candle",
    "lantern",
    "village",
    "market",
    "sailor",
    "compass",
    "harbor",
    "coast",
    "valley",
    "mountain",
    "cloud",
    "thunder",
    "grain",
    "harvest",
    "wheat",
    "orchard",
    "cellar",
    "attic",
    "parlor",
    "courtyard",
    "fountain",
    "statue",
    "portrait",
    "canvas",
    "palette",
    "melody",
    "violin",
    "chorus",
    "ballad",
    "sonnet",
    "prologue",
    "epilogue",
    "oath",
];

const COMMON_WORDS: &[&str] = &[
    "the", "and", "of", "to", "in", "that", "was", "with", "for", "upon", "from", "into", "under",
    "across", "beyond", "through", "before", "after",
];

#[cfg(debug_assertions)]
mod profile {
    pub const AVG_PARAGRAPHS: usize = 200;
    pub const LONG_PARAGRAPHS: usize = 800;
    pub const SAMPLES: usize = 8;
    pub const MAX_AVG_P95_MS: u64 = 1_500;
    pub const MAX_LONG_P95_MS: u64 = 5_000;
}

#[cfg(not(debug_assertions))]
mod profile {
    pub const AVG_PARAGRAPHS: usize = 600;
    pub const LONG_PARAGRAPHS: usize = 4_000;
    pub const SAMPLES: usize = 15;
    pub const MAX_AVG_P95_MS: u64 = 200;
    pub const MAX_LONG_P95_MS: u64 = 600;
}

/// Deterministic xorshift PRNG (no external deps for a test fixture).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn below(&mut self, n: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % n as u64) as usize
    }
}

/// A chapter of `paragraphs` synthetic paragraphs (~70 words each).
fn synthetic_chapter(seed: u64, title: &str, paragraphs: usize) -> Chapter {
    let mut rng = Rng::new(seed);
    let mut blocks = Vec::with_capacity(paragraphs + 1);
    blocks.push(Block::Heading(
        reeda_epub::document::HeadingLevel::H1,
        vec![Inline::Text(title.to_string())],
    ));
    for _ in 0..paragraphs {
        let mut words = String::with_capacity(70 * 6);
        for i in 0..70 {
            if i > 0 {
                words.push(' ');
            }
            let word = if rng.below(5) == 0 {
                WORDS[rng.below(WORDS.len())]
            } else {
                COMMON_WORDS[rng.below(COMMON_WORDS.len())]
            };
            words.push_str(word);
        }
        words.push('.');
        blocks.push(Block::Paragraph(vec![Inline::Text(words)]));
    }
    Chapter {
        spine_index: 0,
        title: title.to_string(),
        href: "ch.xhtml".into(),
        blocks,
    }
}

fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    let idx = (samples.len() as f64 * 0.95).ceil() as usize;
    samples[idx.min(samples.len() - 1)]
}

#[test]
fn paginate_avg_and_long_chapter_p95() {
    let avg = DocumentModel {
        chapters: vec![synthetic_chapter(
            1,
            "Average Chapter",
            profile::AVG_PARAGRAPHS,
        )],
    };
    let long = DocumentModel {
        chapters: vec![synthetic_chapter(
            2,
            "Long Chapter",
            profile::LONG_PARAGRAPHS,
        )],
    };
    let layout = PageLayout::default();

    let avg_chars: usize = avg
        .chapters
        .iter()
        .flat_map(|c| &c.blocks)
        .map(|b| match b {
            Block::Paragraph(items) | Block::Heading(_, items) => items
                .iter()
                .map(|i| match i {
                    Inline::Text(s) | Inline::Code(s) => s.len(),
                    _ => 0,
                })
                .sum::<usize>(),
            _ => 0,
        })
        .sum();
    assert!(avg_chars > 50_000, "avg fixture too small: {avg_chars}");

    let mut avg_samples = Vec::with_capacity(profile::SAMPLES);
    for _ in 0..profile::SAMPLES {
        let start = Instant::now();
        let pages = paginate(&avg, &layout);
        avg_samples.push(start.elapsed());
        assert!(!pages.pages.is_empty());
    }
    let avg_p95 = p95(avg_samples);
    assert!(
        avg_p95 < Duration::from_millis(profile::MAX_AVG_P95_MS),
        "avg-chapter pagination p95 too slow: {avg_p95:?} (budget < {} ms)",
        profile::MAX_AVG_P95_MS
    );

    let long_chars: usize = long
        .chapters
        .iter()
        .flat_map(|c| &c.blocks)
        .map(|b| match b {
            Block::Paragraph(items) | Block::Heading(_, items) => items
                .iter()
                .map(|i| match i {
                    Inline::Text(s) | Inline::Code(s) => s.len(),
                    _ => 0,
                })
                .sum::<usize>(),
            _ => 0,
        })
        .sum();
    assert!(long_chars > 300_000, "long fixture too small: {long_chars}");

    let mut long_samples = Vec::with_capacity(profile::SAMPLES);
    for _ in 0..profile::SAMPLES {
        let start = Instant::now();
        let pages = paginate(&long, &layout);
        long_samples.push(start.elapsed());
        assert!(!pages.pages.is_empty());
    }
    let long_p95 = p95(long_samples);
    assert!(
        long_p95 < Duration::from_millis(profile::MAX_LONG_P95_MS),
        "long-chapter pagination p95 too slow: {long_p95:?} (budget < {} ms)",
        profile::MAX_LONG_P95_MS
    );

    println!(
        "avg-chapter pagination p95: {avg_p95:?} ({avg_chars} chars, budget < {} ms)\n\
         long-chapter pagination p95: {long_p95:?} ({long_chars} chars, budget < {} ms)",
        profile::MAX_AVG_P95_MS,
        profile::MAX_LONG_P95_MS
    );
}

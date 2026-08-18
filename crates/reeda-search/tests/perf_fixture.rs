//! M4.7 — synthetic 50-book corpus + performance smoke test (TODO M4.7).
//!
//! Generates a deterministic multi-language corpus (diacritics, one long book,
//! one empty book) and asserts the M4 exit criterion: index-build < 10 s / 100
//! books and query p95 < 1 s. Timings are measured on a 60-book / ~80 k-word
//! corpus; run with `cargo test --release -p reeda-search --test perf_fixture`
//! for realistic numbers (debug builds are ~5–10x slower but stay under the
//! generous smoke thresholds).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use reeda_search::{IndexManager, IndexedBlock};

const COMMON_WORDS: &[&str] = &[
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
    "chapter",
    "paragraph",
    "sentence",
    "whisper",
    "echo",
    "horizon",
    "sunrise",
    "dusk",
    "midnight",
    "amber",
    "velvet",
    "satin",
    "bronze",
    "iron",
    "ivory",
    "sparrow",
    "falcon",
    "otter",
    "beacon",
    "haven",
    "border",
    "threshold",
    "pathway",
    "staircase",
    "keyhole",
    "lock",
    "diary",
    "chronicle",
    "archive",
    "manuscript",
    "folio",
    "scroll",
];

const FR_WORDS: &[&str] = &["république", "déjà", "crépuscule", "forêt", "plume"];
const DE_WORDS: &[&str] = &["über", "straße", "schön", "blumen", "fenster"];
const ES_WORDS: &[&str] = &["señor", "niño", "mañana", "corazón", "luz"];
const NL_WORDS: &[&str] = &["gezicht", "vogel", "haven", "gisteren", "zomer"];

const LANGUAGES: &[&str] = &["en", "fr", "de", "es", "nl"];

// The strict M4 exit criterion (< 10 s / 100 books, query p95 < 1 s) is
// measured in release; debug builds are 10–20x slower and run a smaller
// corpus with generous smoke thresholds so `cargo test` stays fast.
#[cfg(debug_assertions)]
mod profile {
    pub const BOOK_COUNT: usize = 12;
    pub const LONG_BOOK_CHAPTERS: usize = 8;
    pub const LONG_BOOK_PARAGRAPHS: usize = 12;
    pub const PARAGRAPHS_PER_CHAPTER: usize = 8;
    pub const MIN_WORDS: usize = 15_000;
    pub const MAX_BUILD_MS_PER_100: u64 = 120_000;
    pub const MAX_QUERY_P95_MS: u64 = 5_000;
}

#[cfg(not(debug_assertions))]
mod profile {
    pub const BOOK_COUNT: usize = 50;
    pub const LONG_BOOK_CHAPTERS: usize = 30;
    pub const LONG_BOOK_PARAGRAPHS: usize = 40;
    pub const PARAGRAPHS_PER_CHAPTER: usize = 8;
    pub const MIN_WORDS: usize = 80_000;
    pub const MAX_BUILD_MS_PER_100: u64 = 10_000;
    pub const MAX_QUERY_P95_MS: u64 = 1_000;
}

/// Deterministic xorshift PRNG (no external deps for a test fixture).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A paragraph of `words` synthetic prose in the given language.
fn paragraph(rng: &mut Rng, words: usize, language: &str) -> String {
    let mut out = String::with_capacity(words * 8);
    let local: &[&str] = match language {
        "fr" => FR_WORDS,
        "de" => DE_WORDS,
        "es" => ES_WORDS,
        "nl" => NL_WORDS,
        _ => &[],
    };
    for i in 0..words {
        let word = if !local.is_empty() && rng.below(10) == 0 {
            local[rng.below(local.len())]
        } else {
            COMMON_WORDS[rng.below(COMMON_WORDS.len())]
        };
        if i > 0 {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.push('.');
    out
}

/// A book with `chapters` chapters, each containing `paragraphs` paragraphs of
/// `words_per_paragraph` words. Every book includes the term "journey" at least
/// once (stable cross-book query target).
fn synthetic_book(
    seed: u64,
    title: &str,
    chapters: usize,
    words_per_paragraph: usize,
    language: &str,
) -> Vec<IndexedBlock> {
    let mut rng = Rng::new(seed);
    let mut blocks = Vec::new();
    let mut block_index: u32 = 0;
    for chapter in 0..chapters {
        let chapter_title = format!("Chapter {} — {title}", chapter + 1);
        blocks.push(IndexedBlock {
            book_id: format!("fixture-{seed}"),
            spine_index: chapter as u32,
            block_index,
            char_offset: 0,
            title: chapter_title.clone(),
            body: format!("{} {title}", chapter_title),
            chapter_title,
            language: language.to_string(),
        });
        block_index += 1;
        for _ in 0..words_per_paragraph {
            let body = paragraph(&mut rng, 60, language);
            blocks.push(IndexedBlock {
                book_id: format!("fixture-{seed}"),
                spine_index: chapter as u32,
                block_index,
                char_offset: 0,
                title: String::new(),
                body,
                chapter_title: String::new(),
                language: language.to_string(),
            });
            block_index += 1;
        }
    }
    blocks
}

fn temp_index_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reeda-perf-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    let idx = (samples.len() as f64 * 0.95).ceil() as usize;
    samples[idx.min(samples.len() - 1)]
}

#[test]
fn fifty_book_corpus_index_build_and_query_p95() {
    let dir = temp_index_dir();
    let mut manager = IndexManager::open(&dir).unwrap();

    // Mixed-language books + 1 long book + 1 empty book.
    let mut books: Vec<(String, Vec<IndexedBlock>)> = Vec::new();
    for i in 0..profile::BOOK_COUNT {
        let language = LANGUAGES[i % LANGUAGES.len()];
        let blocks = synthetic_book(
            i as u64,
            &format!("Book {i}"),
            6,
            profile::PARAGRAPHS_PER_CHAPTER,
            language,
        );
        books.push((format!("fixture-{i}"), blocks));
    }
    let long_blocks = synthetic_book(
        1000,
        "The Long Chronicle",
        profile::LONG_BOOK_CHAPTERS,
        profile::LONG_BOOK_PARAGRAPHS,
        "en",
    );
    books.push(("fixture-1000".to_string(), long_blocks));
    books.push(("fixture-empty".to_string(), Vec::new())); // must be a no-op

    let total_blocks: usize = books.iter().map(|(_, b)| b.len()).sum();
    let total_words: usize = books
        .iter()
        .flat_map(|(_, b)| b.iter())
        .map(|b| b.body.split_whitespace().count())
        .sum();
    assert!(
        total_blocks > 400,
        "corpus too small: {total_blocks} blocks"
    );
    assert!(
        total_words > profile::MIN_WORDS,
        "corpus too small: {total_words} words"
    );

    // Bulk index (single commit) then measure.
    let all_blocks: Vec<Vec<IndexedBlock>> = books.iter().map(|(_, b)| b.clone()).collect();
    let start = Instant::now();
    manager.index_many(&all_blocks).unwrap();
    let build_elapsed = start.elapsed();

    // M4 exit criterion: < 10 s / 100 books (release).
    let per_100_ms = build_elapsed.as_millis() as u64 * 100 / books.len() as u64;
    assert!(
        per_100_ms < profile::MAX_BUILD_MS_PER_100,
        "index build too slow: {build_elapsed:?} for {} books (~{}ms/100)",
        books.len(),
        per_100_ms
    );

    // Query p95 < 1 s (release): 100 mixed queries (common + per-language + diacritic).
    let queries: Vec<String> = (0..100)
        .map(|i| match i % 5 {
            0 => "journey".to_string(),
            1 => FR_WORDS[i % FR_WORDS.len()].to_string(),
            2 => DE_WORDS[i % DE_WORDS.len()].to_string(),
            3 => ES_WORDS[i % ES_WORDS.len()].to_string(),
            _ => NL_WORDS[i % NL_WORDS.len()].to_string(),
        })
        .collect();
    let mut samples = Vec::with_capacity(queries.len());
    let mut hits = 0usize;
    for q in &queries {
        let q_start = Instant::now();
        let result = manager.search(q, None, 200).unwrap();
        samples.push(q_start.elapsed());
        hits += result.hits.len();
    }
    let p95 = p95(samples);
    assert!(
        p95 < Duration::from_millis(profile::MAX_QUERY_P95_MS),
        "query p95 too slow: {p95:?}"
    );
    assert!(hits > 0, "expected hits across the corpus");

    // Re-index one book via the per-book path: replacement must not duplicate.
    let before = manager
        .search("journey", Some("fixture-1"), 10)
        .unwrap()
        .total;
    manager.index_book(&books[1].1).unwrap();
    let after = manager
        .search("journey", Some("fixture-1"), 10)
        .unwrap()
        .total;
    assert_eq!(before, after, "re-index must replace, not duplicate");

    std::fs::remove_dir_all(&dir).ok();
}

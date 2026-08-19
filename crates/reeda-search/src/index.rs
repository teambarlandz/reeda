//! Tantivy-backed full-text index for Reeda (docs/SEARCH_SPEC.md §2).
//!
//! Schema:
//! - `book_id` (raw term) — per-book filtering and deletion
//! - `spine_index`, `block_index`, `char_offset` (u64) — locator reconstruction
//! - `title` (TEXT, boosted at query time) — chapter title
//! - `body` (TEXT) — block text
//! - `chapter_title` (stored) — display grouping
//! - `language` (stored) — OPF `dc:language` (analyzer selection deferred, P2)

use std::fs;
use std::path::{Path, PathBuf};

use reeda_epub::cfi::{Cfi, CfiRange as EpubCfiRange};
use reeda_epub::selection::GlobalRange;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Schema, TextOptions, Value, INDEXED, STORED};
use tantivy::{doc, Index, IndexReader, IndexWriter, TantivyDocument, Term};

/// Schema version. Bump when the schema/analysis changes to force a rebuild.
pub const INDEX_VERSION: u32 = 2;

const VERSION_FILE: &str = "version.txt";
const DEFAULT_QUERY_LIMIT: usize = 200;
/// Tokenizer name for the English analyzer (lowercase + stopword filtering).
pub const EN_TOKENIZER: &str = "en";

/// English stopwords (SEARCH_SPEC §3, en first).
const EN_STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from", "had", "has", "have",
    "he", "her", "his", "i", "in", "into", "is", "it", "its", "me", "my", "not", "of", "on", "or",
    "our", "she", "so", "that", "the", "their", "them", "then", "there", "these", "they", "this",
    "those", "to", "was", "we", "were", "what", "when", "where", "which", "who", "will", "with",
    "you", "your",
];

/// Build the English text analyzer: simple segmentation → lowercase → stopwords.
pub fn en_analyzer() -> tantivy::tokenizer::TextAnalyzer {
    use tantivy::tokenizer::{LowerCaser, SimpleTokenizer, StopWordFilter, TextAnalyzer};
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .filter(StopWordFilter::remove(
            EN_STOPWORDS.iter().map(|w| w.to_string()),
        ))
        .build()
}

/// A single block of text queued for indexing (SEARCH_SPEC §2 document model).
#[derive(Debug, Clone)]
pub struct IndexedBlock {
    /// Book UUID as string.
    pub book_id: String,
    /// Spine position of the containing chapter.
    pub spine_index: u32,
    /// Global block index within the book (selection.rs convention).
    pub block_index: u32,
    /// Character offset of this block's text within the block (0 for full blocks).
    pub char_offset: u32,
    /// Chapter title (display + searchable, boosted).
    pub title: String,
    /// The block's plain text content.
    pub body: String,
    /// Chapter title for display grouping (duplicate of `title`, stored).
    pub chapter_title: String,
    /// OPF `dc:language` (stored only; per-language analysis is deferred, P2).
    pub language: String,
}

/// A ranked search hit with everything needed to open the match.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Book UUID as string.
    pub book_id: String,
    /// Global block index of the hit.
    pub block_index: u32,
    /// Character offset of the hit within the block.
    pub char_offset: u32,
    /// Book/chapter display title.
    pub title: String,
    /// Chapter title for display grouping.
    pub chapter_title: String,
    /// Snippet with `<mark>`-wrapped matches (HTML-escaped text).
    pub snippet: String,
    /// CFI range of the first match (open-at-match, SEA-03).
    pub cfi: EpubCfiRange,
    /// Length in chars of the first matching query term (for transient highlight).
    pub term_len: u32,
}

/// Result of a search query: hits + total count (before the cap).
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// Ranked hits, capped at the internal `DEFAULT_QUERY_LIMIT` (200).
    pub hits: Vec<SearchHit>,
    /// Total number of matching documents (uncapped).
    pub total: usize,
}

#[derive(Clone, Copy)]
struct Fields {
    book_id: Field,
    spine_index: Field,
    block_index: Field,
    char_offset: Field,
    title: Field,
    body: Field,
    chapter_title: Field,
    language: Field,
}

impl Fields {
    fn new(schema: &Schema) -> Self {
        Self {
            book_id: schema.get_field("book_id").expect("schema field"),
            spine_index: schema.get_field("spine_index").expect("schema field"),
            block_index: schema.get_field("block_index").expect("schema field"),
            char_offset: schema.get_field("char_offset").expect("schema field"),
            title: schema.get_field("title").expect("schema field"),
            body: schema.get_field("body").expect("schema field"),
            chapter_title: schema.get_field("chapter_title").expect("schema field"),
            language: schema.get_field("language").expect("schema field"),
        }
    }
}

fn build_schema() -> Schema {
    use tantivy::schema::TextFieldIndexing;
    let mut b = tantivy::schema::SchemaBuilder::new();
    b.add_text_field(
        "book_id",
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("raw")
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    b.add_u64_field("spine_index", INDEXED | STORED);
    b.add_u64_field("block_index", INDEXED | STORED);
    b.add_u64_field("char_offset", INDEXED | STORED);
    b.add_text_field(
        "title",
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(EN_TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    b.add_text_field(
        "body",
        TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(EN_TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    b.add_text_field("chapter_title", TextOptions::default().set_stored());
    b.add_text_field("language", TextOptions::default().set_stored());
    b.build()
}

/// Commit the writer, retrying transient IO errors (Windows file-lock races).
fn commit_with_retry(writer: &mut IndexWriter<TantivyDocument>) -> tantivy::Result<()> {
    for attempt in 0..3 {
        match writer.commit() {
            Ok(_) => return Ok(()),
            Err(e) if attempt < 2 => {
                std::thread::sleep(std::time::Duration::from_millis(150 * (attempt + 1)));
                let _ = e;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn add_book_docs(
    fields: Fields,
    writer: &mut IndexWriter<TantivyDocument>,
    docs: &[IndexedBlock],
) -> tantivy::Result<()> {
    let Some(first) = docs.first() else {
        return Ok(());
    };
    let book_id = first.book_id.clone();
    // Replace: delete any previously indexed documents for this book.
    writer.delete_term(Term::from_field_text(fields.book_id, &book_id));
    for block in docs {
        writer.add_document(doc!(
            fields.book_id => block.book_id.clone(),
            fields.spine_index => block.spine_index as u64,
            fields.block_index => block.block_index as u64,
            fields.char_offset => block.char_offset as u64,
            fields.title => block.title.clone(),
            fields.body => block.body.clone(),
            fields.chapter_title => block.chapter_title.clone(),
            fields.language => block.language.clone(),
        ))?;
    }
    Ok(())
}

/// Owns the Tantivy index: creation, incremental updates, deletes, queries.
pub struct IndexManager {
    path: PathBuf,
    index: Index,
    fields: Fields,
    writer: IndexWriter<TantivyDocument>,
    reader: IndexReader,
}

impl IndexManager {
    /// Open (or create) the index at `path`, enforcing [`INDEX_VERSION`].
    ///
    /// If the version stamp is missing or stale, the index is rebuilt from
    /// scratch (derived data — safe to lose).
    pub fn open(path: impl AsRef<Path>) -> tantivy::Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;
        let version_path = path.join(VERSION_FILE);
        let stamp_matches = fs::read_to_string(&version_path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            == Some(INDEX_VERSION);

        if !stamp_matches {
            for entry in fs::read_dir(&path)? {
                let entry = entry?;
                let _ = fs::remove_file(entry.path());
            }
            fs::write(&version_path, INDEX_VERSION.to_string())?;
        }

        let index = match Index::open_in_dir(&path) {
            Ok(index) => index,
            Err(_) => {
                // Missing or stale: create a fresh index.
                let index = Index::create_in_dir(path.clone(), build_schema())?;
                fs::write(&version_path, INDEX_VERSION.to_string())?;
                index
            }
        };
        let schema = index.schema();
        let fields = Fields::new(&schema);
        // Register the English analyzer for title/body (must exist at both
        // index and query time; open_in_dir restores names from meta.json).
        index.tokenizers().register(EN_TOKENIZER, en_analyzer());
        let writer: IndexWriter<TantivyDocument> = index.writer(64 * 1024 * 1024)?;
        let reader = index.reader()?;
        Ok(Self {
            path,
            index,
            fields,
            writer,
            reader,
        })
    }

    /// Path of the index directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Index a batch of blocks, replacing any existing documents for the book.
    ///
    /// Commits the batch, making results immediately queryable.
    pub fn index_book(&mut self, docs: &[IndexedBlock]) -> tantivy::Result<()> {
        add_book_docs(self.fields, &mut self.writer, docs)?;
        commit_with_retry(&mut self.writer)?;
        self.reader.reload()?;
        Ok(())
    }

    /// Index many books in a single commit (bulk import / rebuild path).
    ///
    /// Empty book lists are skipped; one commit + reload covers the whole batch.
    pub fn index_many(&mut self, books: &[Vec<IndexedBlock>]) -> tantivy::Result<()> {
        let mut any = false;
        for docs in books {
            if !docs.is_empty() {
                add_book_docs(self.fields, &mut self.writer, docs)?;
                any = true;
            }
        }
        if any {
            commit_with_retry(&mut self.writer)?;
            self.reader.reload()?;
        }
        Ok(())
    }

    /// Remove all documents belonging to a book (SEARCH_SPEC §4 lifecycle).
    pub fn delete_book(&mut self, book_id: &str) -> tantivy::Result<()> {
        self.writer
            .delete_term(Term::from_field_text(self.fields.book_id, book_id));
        commit_with_retry(&mut self.writer)?;
        self.reader.reload()?;
        Ok(())
    }

    /// Run a ranked full-text query.
    ///
    /// `book_filter` restricts to a single book (search-within-book, SEA-05).
    /// Results are BM25-ranked; `title` matches are boosted 2.0 (spec §5).
    /// Empty/whitespace queries and unparseable queries yield empty results.
    pub fn search(
        &self,
        query_str: &str,
        book_filter: Option<&str>,
        limit: usize,
    ) -> tantivy::Result<SearchResult> {
        if query_str.trim().is_empty() {
            return Ok(SearchResult::default());
        }
        let limit = if limit == 0 {
            DEFAULT_QUERY_LIMIT
        } else {
            limit.clamp(1, 500)
        };
        let searcher = self.reader.searcher();

        let mut parser =
            QueryParser::for_index(&self.index, vec![self.fields.body, self.fields.title]);
        // AND semantics between terms (SEARCH_SPEC §5).
        parser.set_conjunction_by_default();
        parser.set_field_boost(self.fields.title, 2.0);
        let Ok(parsed) = parser.parse_query(query_str) else {
            return Ok(SearchResult::default());
        };

        let query: Box<dyn tantivy::query::Query> = match book_filter {
            Some(book_id) => Box::new(BooleanQuery::new(vec![
                (Occur::Must, parsed),
                (
                    Occur::Must,
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.fields.book_id, book_id),
                        IndexRecordOption::Basic,
                    )),
                ),
            ])),
            None => parsed,
        };

        let top = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
        let mut hits = Vec::with_capacity(top.len());
        for (_, addr) in top {
            let tantivy_doc = searcher.doc::<TantivyDocument>(addr)?;
            hits.push(self.hit_from_doc(&searcher, &query, &tantivy_doc)?);
        }
        let total = hits.len();

        Ok(SearchResult { hits, total })
    }

    fn hit_from_doc(
        &self,
        searcher: &tantivy::Searcher,
        query: &dyn tantivy::query::Query,
        doc: &TantivyDocument,
    ) -> tantivy::Result<SearchHit> {
        let s = |f: Field| -> String {
            doc.get_first(f)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_default()
        };
        let u = |f: Field| -> u64 { doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0) };

        let block_index = u(self.fields.block_index) as u32;
        let char_offset = u(self.fields.char_offset) as u32;
        let body = s(self.fields.body);

        // Snippet with <mark>-wrapped matches.
        let snippet = {
            let generator =
                tantivy::snippet::SnippetGenerator::create(searcher, query, self.fields.body)?;
            let snip = generator.snippet_from_doc(doc);
            snippet_with_marks(snip.fragment(), snip.highlighted())
        };

        // Locator: first query term gives the match width within the block.
        let term_chars = first_query_term_chars(query).unwrap_or(0);
        let range = GlobalRange::new(
            block_index as usize,
            char_offset as usize,
            block_index as usize,
            (char_offset as usize).saturating_add(term_chars),
        )
        .to_cfi();
        let cfi = EpubCfiRange {
            start: Cfi(range.start.0),
            end: Cfi(range.end.0),
        };

        let _ = body;
        Ok(SearchHit {
            book_id: s(self.fields.book_id),
            block_index,
            char_offset,
            title: s(self.fields.title),
            chapter_title: s(self.fields.chapter_title),
            snippet,
            cfi,
            term_len: term_chars as u32,
        })
    }
}

/// Wrap the highlighted ranges of a snippet fragment in `<mark>` tags.
fn snippet_with_marks(fragment: &str, ranges: &[std::ops::Range<usize>]) -> String {
    let mut out = String::new();
    let mut pos = 0;
    for range in ranges {
        let start = range.start.min(fragment.len());
        let end = range.end.min(fragment.len());
        if start < pos || start > end {
            continue;
        }
        out.push_str(&escape_html(&fragment[pos..start]));
        out.push_str("<mark>");
        out.push_str(&escape_html(&fragment[start..end]));
        out.push_str("</mark>");
        pos = end;
    }
    out.push_str(&escape_html(&fragment[pos..]));
    out
}

/// Escape HTML special characters for safe snippet display.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// First term text (decoded) of a parsed query, as characters.
fn first_query_term_chars(query: &dyn tantivy::query::Query) -> Option<usize> {
    let mut first: Option<String> = None;
    query.query_terms(&mut |term, _| {
        if first.is_none() {
            if let Some(s) = term.value().as_str() {
                first = Some(s.to_string());
            }
        }
    });
    first.map(|s| s.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_index() -> (tempfile::TempDir, IndexManager) {
        let dir = tempfile::tempdir().unwrap();
        let mgr = IndexManager::open(dir.path().join("index")).unwrap();
        (dir, mgr)
    }

    fn sample_docs() -> Vec<IndexedBlock> {
        vec![
            IndexedBlock {
                book_id: "11111111-1111-1111-1111-111111111111".into(),
                spine_index: 0,
                block_index: 0,
                char_offset: 0,
                title: "Chapter 1".into(),
                body: "The quick brown fox jumps over the lazy dog.".into(),
                chapter_title: "Chapter 1".into(),
                language: "en".into(),
            },
            IndexedBlock {
                book_id: "11111111-1111-1111-1111-111111111111".into(),
                spine_index: 0,
                block_index: 1,
                char_offset: 0,
                title: "Chapter 1".into(),
                body: "A second paragraph with the quick fox again.".into(),
                chapter_title: "Chapter 1".into(),
                language: "en".into(),
            },
            IndexedBlock {
                book_id: "22222222-2222-2222-2222-222222222222".into(),
                spine_index: 0,
                block_index: 0,
                char_offset: 0,
                title: "Chapter 2".into(),
                body: "Gardening tips for tulips and roses.".into(),
                chapter_title: "Chapter 2".into(),
                language: "en".into(),
            },
        ]
    }

    #[test]
    fn search_finds_ranked_hits() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        let res = mgr.search("fox", None, 10).unwrap();
        assert_eq!(res.total, 2);
        assert!(res
            .hits
            .iter()
            .all(|h| h.snippet.contains("<mark>fox</mark>")));
    }

    #[test]
    fn bm25_ranks_repeated_terms_first() {
        let (_dir, mut mgr) = temp_index();
        let docs = vec![
            IndexedBlock {
                book_id: "1".into(),
                spine_index: 0,
                block_index: 0,
                char_offset: 0,
                title: "Chapter".into(),
                body: "fox fox".into(),
                chapter_title: "Chapter".into(),
                language: "en".into(),
            },
            IndexedBlock {
                book_id: "1".into(),
                spine_index: 0,
                block_index: 1,
                char_offset: 0,
                title: "Chapter".into(),
                body: "only one fox appears in this much longer sentence".into(),
                chapter_title: "Chapter".into(),
                language: "en".into(),
            },
        ];
        mgr.index_book(&docs).unwrap();

        let res = mgr.search("fox", None, 10).unwrap();
        assert_eq!(res.total, 2);
        assert_eq!(res.hits[0].block_index, 0);
    }

    #[test]
    fn search_honors_book_filter() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        let res = mgr
            .search("fox", Some("11111111-1111-1111-1111-111111111111"), 10)
            .unwrap();
        assert_eq!(res.total, 2);
        let res = mgr
            .search("fox", Some("22222222-2222-2222-2222-222222222222"), 10)
            .unwrap();
        assert_eq!(res.total, 0);
    }

    #[test]
    fn phrase_query_works() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        let res = mgr.search("\"quick brown fox\"", None, 10).unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.hits[0].block_index, 0);

        let res = mgr.search("\"brown fox quick\"", None, 10).unwrap();
        assert_eq!(res.total, 0);
    }

    #[test]
    fn delete_book_removes_documents() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        mgr.delete_book("11111111-1111-1111-1111-111111111111")
            .unwrap();
        let res = mgr.search("fox", None, 10).unwrap();
        assert_eq!(res.total, 0);
        // Other book's documents are untouched.
        let res = mgr.search("tulips", None, 10).unwrap();
        assert_eq!(res.total, 1);
    }

    #[test]
    fn reindex_replaces_documents() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        // Re-import: same book, different content.
        let mut replaced = sample_docs();
        replaced.truncate(1);
        replaced[0].body = "Completely different text about astronomy.".into();
        mgr.index_book(&replaced).unwrap();

        let res = mgr.search("fox", None, 10).unwrap();
        assert_eq!(res.total, 0);
        let res = mgr.search("astronomy", None, 10).unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.hits[0].book_id, replaced[0].book_id);
    }

    #[test]
    fn stale_version_rebuilds_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index");
        {
            let mut mgr = IndexManager::open(&path).unwrap();
            mgr.index_book(&sample_docs()).unwrap();
        }
        // Corrupt the stamp → next open must rebuild (empty index).
        fs::write(path.join(VERSION_FILE), "999").unwrap();
        let mgr = IndexManager::open(&path).unwrap();
        let res = mgr.search("fox", None, 10).unwrap();
        assert_eq!(res.total, 0);
    }

    #[test]
    fn hit_cfi_locator_round_trips() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();

        let res = mgr.search("fox", None, 10).unwrap();
        let hit = &res.hits[0];
        assert!(hit.cfi.start.0.starts_with("epubcfi("));
        // Locator decodes back to the stored position.
        let gr = GlobalRange::from_cfi(
            &EpubCfiRange {
                start: Cfi(hit.cfi.start.0.clone()),
                end: Cfi(hit.cfi.end.0.clone()),
            },
            1,
        )
        .unwrap();
        assert_eq!(gr.block_start as u32, hit.block_index);
        assert_eq!(gr.char_start as u32, hit.char_offset);
    }

    #[test]
    fn empty_and_whitespace_query_returns_empty() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        let res = mgr.search("   ", None, 10).unwrap();
        assert_eq!(res.total, 0);
    }

    #[test]
    fn dbg_inspect_stored_fields() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        let res = mgr.search("fox", None, 10).unwrap();
        for h in &res.hits {
            eprintln!(
                "HIT book={} title={} ch={} snippet={}",
                h.book_id, h.title, h.chapter_title, h.snippet
            );
        }
    }

    #[test]
    fn cap_limits_results() {
        let (_dir, mut mgr) = temp_index();
        let mut docs = Vec::new();
        for i in 0..30 {
            docs.push(IndexedBlock {
                book_id: "1".into(),
                spine_index: 0,
                block_index: i as u32,
                char_offset: 0,
                title: "Chapter".into(),
                body: "common word appears everywhere".into(),
                chapter_title: "Chapter".into(),
                language: "en".into(),
            });
        }
        mgr.index_book(&docs).unwrap();
        let res = mgr.search("common", None, 5).unwrap();
        assert_eq!(res.hits.len(), 5);
    }

    #[test]
    fn stopword_only_query_returns_empty() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        // "the" is filtered out by the analyzer → no terms → no results.
        let res = mgr.search("the", None, 10).unwrap();
        assert_eq!(res.total, 0);
        let res = mgr.search("the quick", None, 10).unwrap();
        assert_eq!(res.total, 2);
    }

    #[test]
    fn query_is_case_insensitive() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        let res = mgr.search("FOX", None, 10).unwrap();
        assert_eq!(res.total, 2);
        // "Brown" only appears in block 0.
        let res = mgr.search("Brown", None, 10).unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.hits[0].block_index, 0);
    }

    #[test]
    fn multi_term_query_uses_and_semantics() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        // Both terms must appear: "fox" in blocks 0+1, "gardening" in book 2.
        let res = mgr.search("fox gardening", None, 10).unwrap();
        assert_eq!(res.total, 0);
        // Both in the same block.
        let res = mgr.search("quick brown", None, 10).unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.hits[0].block_index, 0);
    }

    #[test]
    fn phrase_with_stopwords_works() {
        let (_dir, mut mgr) = temp_index();
        mgr.index_book(&sample_docs()).unwrap();
        // Stopwords inside phrases are filtered; phrase still matches.
        let res = mgr.search("\"brown fox jumps\"", None, 10).unwrap();
        assert_eq!(res.total, 1);
    }
}

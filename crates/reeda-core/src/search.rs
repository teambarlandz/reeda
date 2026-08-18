/// Full-text search service (SEARCH_SPEC).
///
/// Owns the Tantivy index on disk and translates between reeda-core
/// concepts (`BookId`, `ParsedDoc`) and search index documents.
use std::path::Path;

use reeda_search::index::{IndexManager, SearchResult};

use crate::models::BookId;
use crate::reader::ParsedDoc;

/// Errors from the search service.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// Tantivy/index error.
    #[error("index error: {0}")]
    Index(#[from] tantivy::TantivyError),
    /// IO error while opening the index directory.
    #[error("index io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for search operations.
pub type SearchResultT<T> = Result<T, SearchError>;

/// Full-text search over the library, backed by Tantivy.
pub struct SearchService {
    /// Underlying Tantivy index manager.
    mgr: IndexManager,
}

impl SearchService {
    /// Open (or create) the search index under `root/search`.
    pub fn open(root: &Path) -> SearchResultT<Self> {
        let dir = root.join("search");
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            mgr: IndexManager::open(&dir)?,
        })
    }

    /// Replace the index content for a book with its parsed document.
    pub fn index_book(&mut self, book_id: BookId, parsed: &ParsedDoc) -> SearchResultT<()> {
        let mut docs = Vec::with_capacity(parsed.document.total_blocks());
        for block_index in 0..parsed.document.total_blocks() {
            let Some((chapter, _block, _local)) = parsed.document.block_at(block_index) else {
                continue;
            };
            let Some(body) = parsed.document.block_text(block_index) else {
                continue;
            };
            if body.is_empty() {
                continue;
            }
            docs.push(reeda_search::index::IndexedBlock {
                book_id: book_id.to_string(),
                spine_index: chapter.spine_index,
                block_index: block_index as u32,
                char_offset: 0,
                title: chapter.title.clone(),
                body,
                chapter_title: chapter.title.clone(),
                language: "en".into(),
            });
        }
        self.mgr.index_book(&docs)?;
        Ok(())
    }

    /// Remove all index documents for a book.
    pub fn delete_book(&mut self, book_id: BookId) -> SearchResultT<()> {
        self.mgr.delete_book(&book_id.to_string())?;
        Ok(())
    }

    /// Search across all indexed books.
    ///
    /// Returns `None` when the index is unavailable (e.g. no store configured).
    pub fn search(&mut self, query: &str, limit: Option<usize>) -> SearchResultT<SearchResult> {
        Ok(self.mgr.search(query, None, limit.unwrap_or(200))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::epub_book_to_parsed_doc;
    use reeda_epub::open_epub;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "reeda-search-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_epub() -> Vec<u8> {
        crate::app::tests::make_test_epub_bytes()
    }

    #[test]
    fn indexes_and_searches_book() {
        let dir = temp_dir();
        let mut svc = SearchService::open(&dir).unwrap();
        let book_id = BookId::new();
        let parsed = epub_book_to_parsed_doc(&open_epub(&test_epub()).unwrap(), book_id);
        svc.index_book(book_id, &parsed).unwrap();

        let res = svc.search("Hello", None).unwrap();
        assert_eq!(res.total, 1);
        let hit = &res.hits[0];
        assert_eq!(hit.book_id, book_id.to_string());
        assert!(hit.snippet.contains("Hello"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_book_removes_hits() {
        let dir = temp_dir();
        let mut svc = SearchService::open(&dir).unwrap();
        let book_id = BookId::new();
        let parsed = epub_book_to_parsed_doc(&open_epub(&test_epub()).unwrap(), book_id);
        svc.index_book(book_id, &parsed).unwrap();
        svc.delete_book(book_id).unwrap();

        let res = svc.search("Hello", None).unwrap();
        assert_eq!(res.total, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn app_import_indexes_and_delete_unindexes() {
        use crate::App;
        let dir = temp_dir();
        let mut app = App::new();
        app.set_search(SearchService::open(&dir).unwrap());

        let events = app.import_from_bytes(test_epub(), "test.epub".into());
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::Event::ImportFinished { .. })));

        let res = app.search_books("hello", None).unwrap();
        assert_eq!(res.total, 1);
        assert!(res.hits[0].snippet.contains("Hello"));

        let book_id = app.snapshot().library[0].id;
        let events = app.delete_book(book_id);
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::Event::LibraryChanged)));
        let res = app.search_books("hello", None).unwrap();
        assert_eq!(res.total, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dispatch_search_emits_results_and_snapshot() {
        use crate::{App, Command};
        let dir = temp_dir();
        let mut app = App::new();
        app.set_search(SearchService::open(&dir).unwrap());
        app.import_from_bytes(test_epub(), "test.epub".into());

        let events = app.dispatch(Command::Search {
            query: "hello".into(),
        });
        assert!(
            events
                .iter()
                .any(|e| matches!(e, crate::Event::SearchResults { .. })),
            "expected SearchResults event, got {events:?}"
        );

        let snap = app.snapshot();
        let search = snap.last_search.expect("search results in snapshot");
        assert_eq!(search.total, 1);
        assert_eq!(search.hits.len(), 1);
        assert!(search.hits[0].snippet.contains("Hello"));
        assert!(!search.hits[0].cfi.is_empty());
        assert!(search.hits[0].term_len > 0);
        assert_eq!(search.hits[0].book_title, "Integration Test Book");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dispatch_search_no_results() {
        use crate::{App, Command};
        let dir = temp_dir();
        let mut app = App::new();
        app.set_search(SearchService::open(&dir).unwrap());
        app.import_from_bytes(test_epub(), "test.epub".into());

        let events = app.dispatch(Command::Search {
            query: "zzzzyyyx".into(),
        });
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::Event::SearchNoResults)));
        let snap = app.snapshot();
        assert_eq!(snap.last_search.as_ref().unwrap().total, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_search_hit_jumps_and_sets_transient_highlight() {
        use crate::{App, Command};
        let dir = temp_dir();
        let mut app = App::new();
        app.set_search(SearchService::open(&dir).unwrap());
        app.import_from_bytes(test_epub(), "test.epub".into());

        app.dispatch(Command::Search {
            query: "hello".into(),
        });
        let snap = app.snapshot();
        let hit = snap.last_search.as_ref().unwrap().hits[0].clone();

        let events = app.dispatch(Command::OpenSearchHit {
            book_id: hit.book_id,
            cfi: hit.cfi.clone(),
            block_index: hit.block_index,
            char_offset: hit.char_offset,
            term_len: hit.term_len,
        });
        assert!(
            events
                .iter()
                .any(|e| matches!(e, crate::Event::SearchResultOpened { .. })),
            "expected SearchResultOpened, got {events:?}"
        );

        let snap = app.snapshot();
        assert_eq!(snap.current_book.as_ref().unwrap().id, hit.book_id);
        assert!(
            snap.transient_highlight.is_some(),
            "transient highlight set"
        );
        assert!(
            snap.page_lines.iter().flatten().any(|run| run.highlighted),
            "page lines contain the transient highlight"
        );

        // Page turn clears the transient highlight.
        app.dispatch(Command::TurnPage { forward: true });
        assert!(app.snapshot().transient_highlight.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_search_hit_unknown_book_errors() {
        use crate::{App, Command};
        let dir = temp_dir();
        let mut app = App::new();
        app.set_search(SearchService::open(&dir).unwrap());

        let events = app.dispatch(Command::OpenSearchHit {
            book_id: BookId::new(),
            cfi: "epubcfi(/6/4!/4/2/2/1:0)".into(),
            block_index: 0,
            char_offset: 0,
            term_len: 1,
        });
        assert!(events
            .iter()
            .any(|e| matches!(e, crate::Event::Error { .. })));

        std::fs::remove_dir_all(&dir).ok();
    }
}

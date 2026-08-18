use crate::models::{AnnotationId, BookId};

/// An event emitted from the application core to the UI after processing
/// a command. The UI renders from a `StateSnapshot` (see `app.rs`), but
/// events carry transient signals for animations, navigation, errors, etc.
#[derive(Debug, Clone)]
pub enum Event {
    // ── Library ──────────────────────────────────────────────────────
    /// The library list has changed (import, delete, metadata update).
    LibraryChanged,

    // ── Reader ───────────────────────────────────────────────────────
    /// The current page has changed.
    PageChanged {
        /// 0-based page index.
        page_index: u32,
        /// Total pages in the current pagination.
        total_pages: u32,
    },

    /// Reading progress was saved to storage.
    ProgressSaved {
        /// The CFI of the saved position.
        cfi: String,
    },

    // ── Import ───────────────────────────────────────────────────────
    /// Import completed successfully.
    ImportFinished {
        /// The ID of the imported book.
        book_id: BookId,
    },

    /// Import failed.
    ImportFailed {
        /// Human-readable error description.
        error: String,
    },

    // ── Annotations ──────────────────────────────────────────────────
    /// An annotation was created or modified.
    AnnotationChanged { annotation_id: AnnotationId },

    /// An annotation was deleted.
    AnnotationDeleted { annotation_id: AnnotationId },

    // ── Search ───────────────────────────────────────────────────────
    /// Search results are ready.
    SearchResults {
        /// Book IDs with matching results.
        results: Vec<BookId>,
    },

    /// Search had no results.
    SearchNoResults,

    /// A search result was opened in the reader (open-at-match, SEA-03).
    SearchResultOpened {
        /// The book that was opened.
        book_id: BookId,
    },

    /// In-reader search state changed (index/total of current match).
    ReaderSearchState {
        /// 0-based index of the currently shown match.
        index: u32,
        /// Total matches in the current book.
        total: u32,
    },

    // ── TTS ──────────────────────────────────────────────────────────
    /// Narration state changed.
    NarrationStateChanged {
        /// Current narration state.
        state: NarrationState,
    },

    /// Word-level highlight during narration.
    WordHighlight {
        /// Global block index of the word being read.
        block_index: u32,
        /// Character offset of the word being read.
        char_offset: u32,
        /// Character length of the word.
        char_len: u32,
    },

    /// The last narratable chapter finished (end of book).
    NarrationFinished,

    // ── Errors ───────────────────────────────────────────────────────
    /// A non-fatal error occurred (shown as a toast/snackbar).
    Error {
        /// Human-readable error message.
        message: String,
    },
}

/// Narration state reported to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrationState {
    /// Not narrating.
    Idle,
    /// Loading / preparing to narrate.
    Loading,
    /// Currently speaking.
    Speaking,
    /// Paused mid-narration.
    Paused,
    /// Stopping narration.
    Stopping,
    /// An error occurred.
    Error,
}

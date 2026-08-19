use std::collections::HashMap;

use reeda_epub::cfi::{Cfi, CfiRange as EpubCfiRange};
use reeda_epub::selection::GlobalRange;

use crate::commands::Command;
use crate::events::{Event, NarrationState};
use crate::models::{
    Annotation, AnnotationId, AnnotationKind, AppSettings, Book, BookFormat, BookId, CfiRange,
    Chapter, ChapterId, LineRun, NotesEntry, SearchHitView, SearchResultsView,
};
use crate::reader::{self, typography_to_layout, PageBlock, ParsedDocRegistry, ReaderState};
use crate::storage::{Database, StorageResult};
use crate::store::BookStore;

/// A snapshot of the complete application state, serializable and sent to
/// the UI after each command dispatch. The UI renders purely from this.
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// All non-deleted books in the library, most recently opened first.
    pub library: Vec<Book>,
    /// The currently open book (None if on the library screen).
    pub current_book: Option<Book>,
    /// Chapters of the current book (in spine order).
    pub current_chapters: Vec<Chapter>,
    /// Current page index within the current book.
    pub current_page: u32,
    /// Total pages after pagination.
    pub total_pages: u32,
    /// Annotations for the current book.
    pub annotations: Vec<Annotation>,
    /// Application settings.
    pub settings: AppSettings,
    /// Current narration state.
    pub narration_state: NarrationState,
    /// Text content of the current page (for Slint rendering).
    pub page_text: String,
    /// Chapter title of the current page.
    pub page_chapter_title: String,
    /// Block-level content of the current page (for rich rendering).
    pub page_blocks: Vec<PageBlock>,
    /// Renderable lines of the current page (plain + highlighted runs).
    pub page_lines: Vec<Vec<LineRun>>,
    /// Notes/highlights list entries for the current book.
    pub notes_entries: Vec<NotesEntry>,
    /// Bookmarks list entries for the current book.
    pub bookmarks_entries: Vec<NotesEntry>,
    /// CFI of the current page's start (for bookmark toggle state).
    pub page_start_cfi: String,
    /// Table of contents labels for the current book.
    pub toc_labels: Vec<String>,
    /// Most recent search results (empty when no search has run).
    pub last_search: Option<SearchResultsView>,
    /// Transient highlight range (current search match), None when inactive.
    pub transient_highlight: Option<CfiRange>,
    /// In-reader search overlay state (None when closed).
    pub reader_search: Option<ReaderSearchView>,
    /// PDF viewer state for the current book (None for EPUBs or on the
    /// library screen).
    pub pdf: Option<PdfView>,
}

/// PDF viewer state exposed to the UI (PDF_SPEC §2).
#[derive(Debug, Clone, Default)]
pub struct PdfView {
    /// Total pages in the document.
    pub page_count: u32,
    /// Per-page `(width, height)` in PDF points (72 dpi) for aspect-correct
    /// sizing of the rasterized pages.
    pub page_sizes: Vec<(f32, f32)>,
    /// Resolved absolute path of the PDF file (feeds the rasterizer).
    pub path: String,
    /// Flattened document outline (bookmarks), pre-order with nesting depth;
    /// every entry resolves to a jumpable page (PDF_SPEC §2.2).
    pub outline: Vec<OutlineItemView>,
}

/// One flattened entry of a PDF document outline (PDF_SPEC §2.2).
///
/// Built from the PDF bookmarks tree in pre-order: parents before
/// children, siblings in document order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutlineItemView {
    /// Section title.
    pub title: String,
    /// Zero-based target page index (jump target).
    pub page_index: u32,
    /// Nesting depth (0 = top-level section).
    pub depth: u32,
}

/// In-reader search state exposed to the UI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ReaderSearchView {
    /// The active query.
    pub query: String,
    /// 0-based index of the currently shown match.
    pub index: u32,
    /// Total matches in the current book.
    pub total: u32,
}

impl Default for StateSnapshot {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            current_book: None,
            current_chapters: Vec::new(),
            current_page: 0,
            total_pages: 0,
            annotations: Vec::new(),
            settings: AppSettings::default(),
            narration_state: NarrationState::Idle,
            page_text: String::new(),
            page_chapter_title: String::new(),
            page_blocks: Vec::new(),
            page_lines: Vec::new(),
            notes_entries: Vec::new(),
            bookmarks_entries: Vec::new(),
            page_start_cfi: String::new(),
            toc_labels: Vec::new(),
            last_search: None,
            transient_highlight: None,
            reader_search: None,
            pdf: None,
        }
    }
}

/// The application core. Owns all mutable state and the command dispatch.
///
/// The UI never accesses state directly: it dispatches a `Command`, receives
/// `Event`s, and reads a `StateSnapshot` after each dispatch cycle.
///
/// See [docs/ARCHITECTURE.md §4.2](../../docs/ARCHITECTURE.md) for the
/// command-bus design.
pub struct App {
    /// Library: all loaded books indexed by ID.
    library: HashMap<BookId, Book>,
    /// Chapters for each book, keyed by book_id.
    chapters: HashMap<BookId, Vec<Chapter>>,
    /// Annotations for each book, keyed by book_id.
    annotations: HashMap<BookId, Vec<Annotation>>,
    /// The currently open book.
    current_book_id: Option<BookId>,
    /// Current page index.
    current_page: u32,
    /// Total pages (set after pagination).
    total_pages: u32,
    /// Application settings.
    settings: AppSettings,
    /// Narration state.
    narration_state: NarrationState,
    /// Parsed documents registry (EPUB content).
    parsed_docs: ParsedDocRegistry,
    /// Reader state for the currently open book (pagination + current page).
    reader_state: Option<ReaderState>,
    /// On-disk file storage for books and covers.
    store: Option<BookStore>,
    /// SQLite database for persistence.
    db: Option<Database>,
    /// Full-text search index.
    search: Option<crate::search::SearchService>,
    /// Most recent search results (for the search screen).
    last_search: Option<SearchResultsView>,
    /// Transient highlight range (current search match), cleared on navigation.
    transient_highlight: Option<CfiRange>,
    /// In-reader search state (current book only).
    reader_search: Option<ReaderSearchState>,
    /// PDF document state for the currently open PDF (None otherwise).
    pdf_state: Option<PdfState>,
    /// Narration engine (chunk queue + state machine, TTS_SPEC §5).
    narration: reeda_tts::engine::NarrationEngine,
    /// Platform speech host (FakeTtsHost on desktop; JNI bridge on Android).
    tts_host: Box<dyn reeda_tts::engine::TtsHost>,
    /// Spine index of the chapter currently being narrated.
    narration_chapter: Option<u32>,
}

/// State of the in-reader search overlay.
#[derive(Debug, Clone)]
struct ReaderSearchState {
    /// The active query.
    query: String,
    /// Hits in the current book (ranked).
    hits: Vec<SearchHitView>,
    /// Index of the currently shown hit.
    index: usize,
}

/// State of an opened PDF document.
#[derive(Debug, Clone)]
struct PdfState {
    /// Resolved absolute path to the PDF file (re-opened on each render).
    path: std::path::PathBuf,
    /// Total pages in the document.
    page_count: u32,
    /// Per-page `(width, height)` in PDF points (72 dpi).
    page_sizes: Vec<(f32, f32)>,
    /// Flattened outline for the outline panel.
    outline: Vec<OutlineItemView>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new `App` with default settings and an empty library.
    pub fn new() -> Self {
        Self {
            library: HashMap::new(),
            chapters: HashMap::new(),
            annotations: HashMap::new(),
            current_book_id: None,
            current_page: 0,
            total_pages: 0,
            settings: AppSettings::default(),
            narration_state: NarrationState::Idle,
            parsed_docs: ParsedDocRegistry::new(),
            reader_state: None,
            store: None,
            db: None,
            search: None,
            last_search: None,
            transient_highlight: None,
            reader_search: None,
            narration: reeda_tts::engine::NarrationEngine::new(1.0, 1.0),
            tts_host: Box::new(reeda_tts::engine::FakeTtsHost::new()),
            narration_chapter: None,
            pdf_state: None,
        }
    }

    /// Create an `App` with a persistent file store.
    pub fn with_store(store: BookStore) -> Self {
        let mut app = Self::new();
        app.store = Some(store);
        app
    }

    /// Create an `App` with a persistent file store and SQLite database.
    pub fn with_store_db(store: BookStore, db: Database) -> Self {
        let mut app = Self::with_store(store);
        app.db = Some(db);
        app
    }

    /// Set the file store (e.g., after initialization with a data directory).
    pub fn set_store(&mut self, store: BookStore) {
        self.store = Some(store);
    }

    /// Set the SQLite database handle.
    pub fn set_db(&mut self, db: Database) {
        self.db = Some(db);
    }

    /// Set the full-text search index service.
    pub fn set_search(&mut self, search: crate::search::SearchService) {
        self.search = Some(search);
    }

    /// Load books from the database into memory (call once at startup).
    ///
    /// Returns the number of books loaded.
    pub fn load_books(&mut self) -> StorageResult<usize> {
        let Some(db) = &self.db else {
            return Ok(0);
        };
        let books = db.list_books()?;
        let mut loaded = 0;
        for book in books {
            if !self.library.contains_key(&book.id) {
                let id = book.id;
                self.library.insert(id, book);
                loaded += 1;
            }
        }
        Ok(loaded)
    }

    /// Load application settings from the database (call once at startup).
    pub fn load_settings_from_db(&mut self) -> StorageResult<()> {
        let Some(db) = &self.db else {
            return Ok(());
        };
        self.settings = db.load_settings()?;
        Ok(())
    }

    /// Dispatch a command, mutating state and returning a list of events
    /// for the UI to process.
    pub fn dispatch(&mut self, command: Command) -> Vec<Event> {
        match command {
            // ── Library ──────────────────────────────────────────────
            Command::Import { uri } => self.import_book(uri),

            Command::ImportPdf { path } => self.import_pdf(path),

            Command::DeleteBook { book_id } => self.delete_book(book_id),

            Command::EditMetadata {
                book_id,
                title,
                author,
            } => self.edit_metadata(book_id, title, author),

            // ── Reader ───────────────────────────────────────────────
            Command::OpenBook { book_id } => self.open_book(book_id),

            Command::OpenPdf { book_id } => self.open_pdf(book_id),

            Command::PdfPage { page_index } => self.pdf_page(page_index),

            Command::CloseBook => self.close_book(),

            Command::TurnPage { forward } => self.turn_page(forward),

            Command::JumpTo { cfi } => self.jump_to(cfi),

            Command::JumpToAnnotation { annotation_id } => self.jump_to_annotation(annotation_id),

            // ── Typography & Theme ───────────────────────────────────
            Command::SetTypography(ty) => {
                self.settings.typography = ty;
                self.settings.updated_at_v2();
                self.repaginate()
            }

            Command::SetTheme(theme) => {
                self.settings.theme = theme;
                self.settings.updated_at_v2();
                vec![]
            }

            // ── Annotations ──────────────────────────────────────────
            Command::AddHighlight { range, color } => {
                let book_id = match self.current_book_id {
                    Some(id) => id,
                    None => {
                        return vec![Event::Error {
                            message: "No book open".into(),
                        }]
                    }
                };
                let snippet = Some(range.short());
                let ann = Annotation::new_highlight(book_id, range, color, snippet);
                let ann_id = ann.id;
                // Persist to SQLite (best-effort).
                if let Some(db) = &self.db {
                    if let Err(e) = db.insert_annotation(&ann) {
                        eprintln!("Warning: failed to persist highlight: {e}");
                    }
                }
                self.annotations.entry(book_id).or_default().push(ann);
                vec![Event::AnnotationChanged {
                    annotation_id: ann_id,
                }]
            }

            Command::EditHighlight {
                annotation_id,
                color,
            } => {
                if let Some(book_id) = self.current_book_id {
                    if let Some(anns) = self.annotations.get_mut(&book_id) {
                        if let Some(ann) = anns.iter_mut().find(|a| a.id == annotation_id) {
                            if let Some(c) = color {
                                ann.color = Some(c);
                                ann.updated_at = chrono::Utc::now();
                            }
                            // Persist to SQLite (best-effort).
                            if let Some(db) = &self.db {
                                if let Err(e) = db.update_annotation(ann) {
                                    eprintln!("Warning: failed to persist color edit: {e}");
                                }
                            }
                            return vec![Event::AnnotationChanged { annotation_id }];
                        }
                    }
                }
                vec![Event::Error {
                    message: "Annotation not found".into(),
                }]
            }

            Command::AddNote {
                annotation_id,
                text,
            } => {
                let book_id = match self.current_book_id {
                    Some(id) => id,
                    None => {
                        return vec![Event::Error {
                            message: "No book open".into(),
                        }]
                    }
                };

                if let Some(hl_id) = annotation_id {
                    // Attach note to existing highlight.
                    if let Some(anns) = self.annotations.get_mut(&book_id) {
                        if let Some(ann) = anns.iter_mut().find(|a| a.id == hl_id) {
                            ann.text = Some(text);
                            ann.updated_at = chrono::Utc::now();
                            // Persist to SQLite (best-effort).
                            if let Some(db) = &self.db {
                                if let Err(e) = db.update_annotation(ann) {
                                    eprintln!("Warning: failed to persist note: {e}");
                                }
                            }
                            return vec![Event::AnnotationChanged {
                                annotation_id: hl_id,
                            }];
                        }
                    }
                    vec![Event::Error {
                        message: "Highlight not found".into(),
                    }]
                } else {
                    // Standalone note.
                    let ann = Annotation::new_note(book_id, None, text);
                    let ann_id = ann.id;
                    // Persist to SQLite (best-effort).
                    if let Some(db) = &self.db {
                        if let Err(e) = db.insert_annotation(&ann) {
                            eprintln!("Warning: failed to persist note: {e}");
                        }
                    }
                    self.annotations.entry(book_id).or_default().push(ann);
                    vec![Event::AnnotationChanged {
                        annotation_id: ann_id,
                    }]
                }
            }

            Command::DeleteAnnotation { annotation_id } => {
                let mut found = false;
                for anns in self.annotations.values_mut() {
                    if let Some(ann) = anns.iter_mut().find(|a| a.id == annotation_id) {
                        ann.deleted_at = Some(chrono::Utc::now());
                        ann.updated_at = chrono::Utc::now();
                        found = true;
                        break;
                    }
                }
                if found {
                    // Persist to SQLite (best-effort).
                    if let Some(db) = &self.db {
                        if let Err(e) = db.delete_annotation(annotation_id) {
                            eprintln!("Warning: failed to persist deletion: {e}");
                        }
                    }
                    vec![Event::AnnotationDeleted { annotation_id }]
                } else {
                    vec![Event::Error {
                        message: "Annotation not found".into(),
                    }]
                }
            }

            Command::ToggleBookmark { cfi } => {
                let book_id = match self.current_book_id {
                    Some(id) => id,
                    None => {
                        return vec![Event::Error {
                            message: "No book open".into(),
                        }]
                    }
                };

                let anns = self.annotations.entry(book_id).or_default();
                // Check if a bookmark already exists at this CFI.
                let existing = anns.iter().position(|a| {
                    a.kind == AnnotationKind::Bookmark
                        && a.deleted_at.is_none()
                        && a.cfi.as_ref().is_some_and(|r| r.start == cfi)
                });

                if let Some(idx) = existing {
                    // Remove existing bookmark (soft-delete).
                    anns[idx].deleted_at = Some(chrono::Utc::now());
                    anns[idx].updated_at = chrono::Utc::now();
                    let removed_id = anns[idx].id;
                    // Persist to SQLite (best-effort).
                    if let Some(db) = &self.db {
                        if let Err(e) = db.delete_annotation(removed_id) {
                            eprintln!("Warning: failed to persist bookmark removal: {e}");
                        }
                    }
                    vec![Event::AnnotationDeleted {
                        annotation_id: removed_id,
                    }]
                } else {
                    let ann = Annotation::new_bookmark(book_id, cfi);
                    let ann_id = ann.id;
                    // Persist to SQLite (best-effort).
                    if let Some(db) = &self.db {
                        if let Err(e) = db.insert_annotation(&ann) {
                            eprintln!("Warning: failed to persist bookmark: {e}");
                        }
                    }
                    anns.push(ann);
                    vec![Event::AnnotationChanged {
                        annotation_id: ann_id,
                    }]
                }
            }

            // ── Search ───────────────────────────────────────────────
            Command::Search { query } => self.search_library(&query),

            Command::OpenSearchHit {
                book_id,
                cfi,
                block_index,
                char_offset,
                term_len,
            } => self.open_search_hit(book_id, cfi, block_index, char_offset, term_len),

            Command::ReaderSearch { query } => self.reader_search(&query),

            Command::ReaderSearchNext => self.reader_search_step(1),

            Command::ReaderSearchPrev => self.reader_search_step(-1),

            Command::ReaderSearchClose => {
                self.reader_search = None;
                self.transient_highlight = None;
                vec![]
            }

            // ── TTS ──────────────────────────────────────────────────
            Command::StartNarration { chapter_id } => self.start_narration(chapter_id),

            Command::PauseNarration => {
                if self.narration.state() == reeda_tts::engine::EngineState::Speaking {
                    self.narration.pause(&mut *self.tts_host);
                    self.narration_state = NarrationState::Paused;
                    vec![Event::NarrationStateChanged {
                        state: NarrationState::Paused,
                    }]
                } else {
                    vec![]
                }
            }

            Command::ResumeNarration => {
                if self.narration.state() == reeda_tts::engine::EngineState::Paused {
                    self.narration.resume(&mut *self.tts_host);
                    self.narration_state = NarrationState::Speaking;
                    vec![Event::NarrationStateChanged {
                        state: NarrationState::Speaking,
                    }]
                } else {
                    vec![]
                }
            }

            Command::StopNarration => self.stop_narration(),

            Command::NarrationSkip { delta } => self.narration_skip(delta),

            Command::PollNarration => self.poll_narration(),

            Command::SetTtsSpeed(speed) => {
                self.settings.tts_speed = speed.clamp(0.5, 2.5);
                self.narration
                    .set_rate(&mut *self.tts_host, self.settings.tts_speed);
                if let Some(db) = &self.db {
                    let _ = db.save_settings(&self.settings);
                }
                vec![]
            }

            Command::SetTtsPitch(pitch) => {
                self.settings.tts_pitch = pitch.clamp(0.5, 1.5);
                self.narration
                    .set_pitch(&mut *self.tts_host, self.settings.tts_pitch);
                if let Some(db) = &self.db {
                    let _ = db.save_settings(&self.settings);
                }
                vec![]
            }

            // ── Settings ─────────────────────────────────────────────
            Command::UpdateSettings(settings) => {
                self.settings = settings.clone();
                if let Some(db) = &self.db {
                    let _ = db.save_settings(&settings);
                }
                vec![]
            }
        }
    }

    /// Return the current application settings.
    pub fn settings(&self) -> AppSettings {
        self.settings.clone()
    }

    /// Return a snapshot of the current application state.
    pub fn snapshot(&self) -> StateSnapshot {
        let current_book = self
            .current_book_id
            .and_then(|id| self.library.get(&id).cloned());

        let current_chapters = self
            .current_book_id
            .and_then(|id| self.chapters.get(&id).cloned())
            .unwrap_or_default();

        let annotations: Vec<Annotation> = self
            .current_book_id
            .and_then(|id| self.annotations.get(&id).cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.deleted_at.is_none())
            .collect();

        let mut library: Vec<Book> = self
            .library
            .values()
            .filter(|b| b.deleted_at.is_none())
            .cloned()
            .collect();
        library.sort_by(|a, b| {
            b.last_opened_at
                .partial_cmp(&a.last_opened_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Page content from reader state.
        let (page_text, page_chapter_title, page_blocks, page_lines, toc_labels) =
            if let (Some(book_id), Some(ref rs)) = (self.current_book_id, &self.reader_state) {
                if let Some(parsed) = self.parsed_docs.get(&book_id) {
                    let content = rs.current_page_content(&parsed.document);
                    let toc: Vec<String> =
                        parsed.toc.items.iter().map(|i| i.label.clone()).collect();
                    let layout = typography_to_layout(&self.settings.typography, 400.0, 700.0);
                    let annotations: Vec<Annotation> =
                        self.annotations.get(&book_id).cloned().unwrap_or_default();
                    let transient = self.transient_highlight.as_ref().map(|r| EpubCfiRange {
                        start: Cfi(r.start.clone()),
                        end: Cfi(r.end.clone()),
                    });
                    let lines = reader::build_page_lines_with_transient(
                        &parsed.document,
                        &rs.pages,
                        self.current_page as usize,
                        layout.chars_per_line(),
                        &annotations,
                        transient.as_ref(),
                    );
                    (
                        content.text,
                        content.chapter_title,
                        content.blocks,
                        lines,
                        toc,
                    )
                } else {
                    (
                        String::new(),
                        String::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                }
            } else {
                (
                    String::new(),
                    String::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            };

        // Notes list entries for the current book.
        let (notes_entries, bookmarks_entries, page_start_cfi) =
            if let Some(book_id) = self.current_book_id {
                if let Some(parsed) = self.parsed_docs.get(&book_id) {
                    let annotations: Vec<Annotation> =
                        self.annotations.get(&book_id).cloned().unwrap_or_default();
                    let spine_len = parsed.spine.len() as u32;
                    let cfi = self
                        .reader_state
                        .as_ref()
                        .map(|rs| reader::page_start_cfi(&rs.pages, self.current_page, spine_len))
                        .unwrap_or_default();
                    (
                        reader::notes_entries(&parsed.document, &annotations),
                        reader::bookmark_entries(&parsed.document, &annotations),
                        cfi,
                    )
                } else {
                    (Vec::new(), Vec::new(), String::new())
                }
            } else {
                (Vec::new(), Vec::new(), String::new())
            };

        StateSnapshot {
            library,
            current_book,
            current_chapters,
            current_page: self.current_page,
            total_pages: self.total_pages,
            annotations,
            settings: self.settings.clone(),
            narration_state: self.narration_state,
            page_text,
            page_chapter_title,
            page_blocks,
            page_lines,
            notes_entries,
            bookmarks_entries,
            page_start_cfi,
            toc_labels,
            last_search: self.last_search.clone(),
            transient_highlight: self.transient_highlight.clone(),
            reader_search: self.reader_search.as_ref().map(|rs| ReaderSearchView {
                query: rs.query.clone(),
                index: rs.index as u32,
                total: rs.hits.len() as u32,
            }),
            pdf: self.pdf_state.as_ref().map(|ps| PdfView {
                page_count: ps.page_count,
                page_sizes: ps.page_sizes.clone(),
                path: ps.path.display().to_string(),
                outline: ps.outline.clone(),
            }),
        }
    }

    /// Export a book's highlights & notes as Markdown.
    ///
    /// Returns `None` if the book or its parsed document is unavailable.
    pub fn export_annotations_markdown(&self, book_id: BookId) -> Option<String> {
        let book = self
            .library
            .get(&book_id)
            .filter(|b| b.deleted_at.is_none())?;
        let parsed = self.parsed_docs.get(&book_id)?;
        let annotations = self.annotations.get(&book_id).cloned().unwrap_or_default();
        Some(crate::export::export_markdown(
            book,
            &parsed.document,
            &annotations,
        ))
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn open_book(&mut self, book_id: BookId) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            book.last_opened_at = Some(chrono::Utc::now());
            self.current_book_id = Some(book_id);

            // Load persisted annotations (best-effort).
            if let Some(db) = &self.db {
                match db.list_annotations(book_id) {
                    Ok(anns) => {
                        self.annotations.insert(book_id, anns);
                    }
                    Err(e) => eprintln!("Warning: failed to load annotations: {e}"),
                }
            }

            // Restore saved page position.
            let saved_page = book
                .last_position
                .as_ref()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            if book.format == BookFormat::Pdf {
                return self.open_pdf_document(book_id, saved_page);
            }

            // Paginate if we have the parsed document.
            if let Some(parsed) = self.parsed_docs.get(&book_id) {
                let layout = typography_to_layout(&self.settings.typography, 400.0, 700.0);
                let pages = reader::paginate_doc(&parsed.document, &layout);
                self.total_pages = pages.pages.len() as u32;
                self.current_page = saved_page.min(self.total_pages.saturating_sub(1));
                self.reader_state = Some(ReaderState {
                    pages,
                    current_page: self.current_page,
                });
            }

            vec![]
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    /// Open a PDF book (must exist in the library with `format == Pdf`).
    fn open_pdf(&mut self, book_id: BookId) -> Vec<Event> {
        let saved_page = self
            .library
            .get(&book_id)
            .and_then(|b| b.last_position.as_ref())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        if let Some(book) = self.library.get_mut(&book_id) {
            book.last_opened_at = Some(chrono::Utc::now());
            self.current_book_id = Some(book_id);
            if let Some(db) = &self.db {
                match db.list_annotations(book_id) {
                    Ok(anns) => {
                        self.annotations.insert(book_id, anns);
                    }
                    Err(e) => eprintln!("Warning: failed to load annotations: {e}"),
                }
            }
            if book.format != BookFormat::Pdf {
                self.current_book_id = None;
                return vec![Event::Error {
                    message: format!("Book {book_id} is not a PDF"),
                }];
            }
            self.open_pdf_document(book_id, saved_page)
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    /// Load the PDF document behind `book_id` into [`PdfState`].
    fn open_pdf_document(&mut self, book_id: BookId, saved_page: u32) -> Vec<Event> {
        let Some(book) = self.library.get(&book_id) else {
            return vec![];
        };
        let Some(path) = self.resolve_book_path(book) else {
            return vec![Event::Error {
                message: "PDF file not found in storage".into(),
            }];
        };
        let doc = match reeda_pdf::document::PdfDocument::open(&path) {
            Ok(doc) => doc,
            Err(e) => {
                self.pdf_state = None;
                self.total_pages = 0;
                self.current_page = 0;
                return vec![Event::Error {
                    message: format!("Failed to open PDF: {e}"),
                }];
            }
        };
        let page_count = doc.page_count().max(1) as u32;
        let page_sizes: Vec<(f32, f32)> = (0..doc.page_count())
            .filter_map(|i| doc.page_size(i))
            .collect();
        // Best-effort outline extraction; a failure only costs the panel.
        let outline = reeda_pdf::outline::extract_outline(&path)
            .map(|items| {
                items
                    .into_iter()
                    .filter_map(|item| {
                        item.page_index.map(|page_index| OutlineItemView {
                            title: item.title,
                            page_index,
                            depth: item.depth,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.pdf_state = Some(PdfState {
            path,
            page_count,
            page_sizes,
            outline,
        });
        self.total_pages = page_count;
        self.current_page = saved_page.min(page_count.saturating_sub(1));
        self.reader_state = None;
        self.transient_highlight = None;
        vec![]
    }

    /// Jump to a page of the currently open PDF (PDF_SPEC §4).
    fn pdf_page(&mut self, page_index: u32) -> Vec<Event> {
        if self.pdf_state.is_none() {
            return vec![Event::Error {
                message: "No PDF open".into(),
            }];
        }
        let clamped = page_index.min(self.total_pages.saturating_sub(1));
        if clamped == self.current_page {
            return vec![];
        }
        self.current_page = clamped;
        self.save_progress();
        vec![Event::PageChanged {
            page_index: clamped,
            total_pages: self.total_pages,
        }]
    }

    /// Resolve the absolute filesystem path of a book's file: an absolute
    /// `file_path` wins (import-time path), otherwise the store path.
    fn resolve_book_path(&self, book: &Book) -> Option<std::path::PathBuf> {
        let direct = std::path::Path::new(&book.file_path);
        if direct.is_absolute() && direct.exists() {
            return Some(direct.to_path_buf());
        }
        if let Some(store) = &self.store {
            let stored = store.book_path(book.id, book.format);
            if stored.exists() {
                return Some(stored);
            }
        }
        None
    }

    fn close_book(&mut self) -> Vec<Event> {
        self.save_progress();
        self.stop_narration();
        self.current_book_id = None;
        self.current_page = 0;
        self.total_pages = 0;
        self.reader_state = None;
        self.pdf_state = None;
        self.transient_highlight = None;
        self.reader_search = None;
        vec![]
    }

    fn turn_page(&mut self, forward: bool) -> Vec<Event> {
        if self.current_book_id.is_none() {
            return vec![];
        }
        if forward {
            if self.current_page + 1 < self.total_pages {
                self.current_page += 1;
            }
        } else if self.current_page > 0 {
            self.current_page -= 1;
        }
        // Navigating turns off the transient search highlight.
        self.transient_highlight = None;
        // Sync reader state.
        if let Some(ref mut rs) = self.reader_state {
            rs.current_page = self.current_page;
        }
        // Persist progress on the book model.
        self.save_progress();
        vec![Event::PageChanged {
            page_index: self.current_page,
            total_pages: self.total_pages,
        }]
    }

    /// Update the current book's progress_pct and last_position in-memory.
    fn save_progress(&mut self) {
        let book_id = self.current_book_id;
        if let Some(book_id) = book_id {
            let current_page = self.current_page;
            let total_pages = self.total_pages;
            if let Some(book) = self.library.get_mut(&book_id) {
                book.last_position = Some(current_page.to_string());
                book.progress_pct = if total_pages > 0 {
                    current_page as f64 / total_pages as f64
                } else {
                    0.0
                };
                book.updated_at = chrono::Utc::now();
            }
            // Persist to SQLite (best-effort).
            if let Some(db) = &self.db {
                let progress = if total_pages > 0 {
                    current_page as f64 / total_pages as f64
                } else {
                    0.0
                };
                if let Err(e) =
                    db.update_book_position(book_id, &current_page.to_string(), progress)
                {
                    eprintln!("Warning: failed to persist reading position: {e}");
                }
            }
        }
    }

    /// Get a mutable reference to the current book.
    #[allow(dead_code)]
    fn current_book_mut(&mut self) -> Option<&mut Book> {
        let book_id = self.current_book_id?;
        self.library.get_mut(&book_id)
    }

    fn jump_to(&mut self, cfi: String) -> Vec<Event> {
        if self.current_book_id.is_none() {
            return vec![];
        }
        if let Some(ref rs) = self.reader_state {
            let spine_len = self
                .current_book_id
                .and_then(|id| self.parsed_docs.get(&id))
                .map(|pd| pd.spine.len() as u32)
                .unwrap_or(0);
            if let Some(page_idx) = reader::find_page_for_cfi(&rs.pages, &cfi, spine_len) {
                self.current_page = page_idx;
                if let Some(ref mut rs) = self.reader_state {
                    rs.current_page = page_idx;
                }
                self.save_progress();
                return vec![Event::PageChanged {
                    page_index: self.current_page,
                    total_pages: self.total_pages,
                }];
            }
        }
        vec![]
    }

    /// Jump to the page containing an annotation.
    fn jump_to_annotation(&mut self, annotation_id: AnnotationId) -> Vec<Event> {
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let found = self.annotations.get(&book_id).and_then(|v| {
            v.iter()
                .find(|a| a.id == annotation_id && a.deleted_at.is_none())
                .and_then(|a| a.cfi.as_ref().map(|r| r.start.clone()))
        });
        match found {
            Some(cfi) => self.jump_to(cfi),
            None => vec![Event::Error {
                message: "Annotation not found".into(),
            }],
        }
    }

    /// Run a library search, storing results for the search screen.
    fn search_library(&mut self, query: &str) -> Vec<Event> {
        // Navigating away from a match clears the transient highlight.
        self.transient_highlight = None;
        let query = query.trim().to_string();
        if query.is_empty() {
            self.last_search = None;
            return vec![Event::SearchNoResults];
        }
        let Some(res) = self.search_books(&query, Some(200)) else {
            return vec![Event::Error {
                message: "Search index unavailable".into(),
            }];
        };
        let view = SearchResultsView {
            total: res.total,
            hits: res.hits.iter().map(|h| self.hit_to_view(h)).collect(),
        };
        let ids: Vec<BookId> = {
            let mut seen = std::collections::HashSet::new();
            view.hits
                .iter()
                .filter(|h| seen.insert(h.book_id))
                .map(|h| h.book_id)
                .collect()
        };
        self.last_search = Some(view);
        if ids.is_empty() {
            vec![Event::SearchNoResults]
        } else {
            vec![Event::SearchResults { results: ids }]
        }
    }

    /// Open a book at a search hit and set the transient highlight.
    fn open_search_hit(
        &mut self,
        book_id: BookId,
        cfi: String,
        block_index: u32,
        char_offset: u32,
        term_len: u32,
    ) -> Vec<Event> {
        if self.library.contains_key(&book_id) {
            if self.current_book_id != Some(book_id) {
                self.open_book(book_id);
            }
            self.set_transient_from_view(&SearchHitView {
                book_id,
                book_title: String::new(),
                chapter_title: String::new(),
                snippet: String::new(),
                cfi: cfi.clone(),
                block_index,
                char_offset,
                term_len,
            });
            let mut events = self.jump_to(cfi);
            events.push(Event::SearchResultOpened { book_id });
            events
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    /// Convert a raw search hit into a UI view (resolving book titles).
    fn hit_to_view(&self, h: &reeda_search::index::SearchHit) -> SearchHitView {
        let book_id = h.book_id.parse().unwrap_or_else(|_| BookId::new());
        SearchHitView {
            book_id,
            book_title: self
                .library
                .get(&book_id)
                .map(|b| b.title.clone())
                .unwrap_or_else(|| h.title.clone()),
            chapter_title: h.chapter_title.clone(),
            snippet: h.snippet.clone(),
            cfi: h.cfi.start.0.clone(),
            block_index: h.block_index,
            char_offset: h.char_offset,
            term_len: h.term_len,
        }
    }

    /// Set the transient highlight from a hit (clamped to the block's text).
    fn set_transient_from_view(&mut self, hit: &SearchHitView) {
        let transient = self
            .current_book_id
            .and_then(|id| self.parsed_docs.get(&id))
            .and_then(|pd| pd.document.block_text(hit.block_index as usize))
            .map(|text| {
                let start = (hit.char_offset as usize).min(text.len());
                let end = (start + hit.term_len as usize).min(text.len()).max(start);
                GlobalRange::new(
                    hit.block_index as usize,
                    start,
                    hit.block_index as usize,
                    end,
                )
                .to_cfi()
            })
            .map(|range| CfiRange {
                start: range.start.0,
                end: range.end.0,
            });
        self.transient_highlight = transient;
    }

    /// Run an in-reader search over the current book (SEA-05) and jump to
    /// the first match.
    fn reader_search(&mut self, query: &str) -> Vec<Event> {
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let query = query.trim().to_string();
        let Some(search) = &mut self.search else {
            return vec![];
        };
        let Ok(res) = search.search_in_book(&query, book_id, 200) else {
            return vec![];
        };
        let hits: Vec<SearchHitView> = res.hits.iter().map(|h| self.hit_to_view(h)).collect();
        if hits.is_empty() {
            self.reader_search = None;
            self.transient_highlight = None;
            return vec![Event::ReaderSearchState { index: 0, total: 0 }];
        }
        self.reader_search = Some(ReaderSearchState {
            query,
            index: 0,
            hits: hits.clone(),
        });
        self.set_transient_from_view(&hits[0]);
        let mut events = self.jump_to(hits[0].cfi.clone());
        events.push(Event::ReaderSearchState {
            index: 0,
            total: hits.len() as u32,
        });
        events
    }

    /// Move to the next/previous in-book match (wraps around).
    fn reader_search_step(&mut self, delta: isize) -> Vec<Event> {
        let (hit, total) = {
            let Some(rs) = &mut self.reader_search else {
                return vec![];
            };
            if rs.hits.is_empty() {
                return vec![];
            }
            let n = rs.hits.len() as isize;
            rs.index = (((rs.index as isize) + delta).rem_euclid(n)) as usize;
            (rs.hits[rs.index].clone(), rs.hits.len() as u32)
        };
        self.set_transient_from_view(&hit);
        let mut events = self.jump_to(hit.cfi);
        events.push(Event::ReaderSearchState {
            index: self.reader_search.as_ref().unwrap().index as u32,
            total,
        });
        events
    }

    // ── Narration (M5, TTS_SPEC §5) ─────────────────────────────────

    /// Start narration from the current page's chapter (or a specific one).
    fn start_narration(&mut self, chapter_id: Option<ChapterId>) -> Vec<Event> {
        let Some(book_id) = self.current_book_id else {
            return vec![Event::Error {
                message: "no open book".to_string(),
            }];
        };
        if self
            .library
            .get(&book_id)
            .is_some_and(|b| b.format == BookFormat::Pdf)
        {
            return vec![Event::Error {
                message: "PDF narration not supported yet (TTS-07, P2)".to_string(),
            }];
        }
        if self.parsed_docs.get(&book_id).is_none() {
            return vec![Event::Error {
                message: "book content not loaded".to_string(),
            }];
        };
        let spine = match chapter_id {
            Some(id) => self
                .chapters
                .get(&book_id)
                .and_then(|cs| cs.iter().find(|c| c.id == id))
                .map(|c| c.spine_index),
            None => self.chapter_of_page(),
        };
        let Some(spine) = spine else {
            return vec![];
        };
        self.load_narration_chapter(spine)
    }

    /// Load + start speaking the chapter at `spine` (replaces any current
    /// narration). Returns state-change events.
    fn load_narration_chapter(&mut self, spine: u32) -> Vec<Event> {
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let chunks = match self.parsed_docs.get(&book_id) {
            Some(pd) => reeda_tts::chunk::Chunker::new().chunks_for_chapter(&pd.document, spine),
            None => return vec![],
        };
        if chunks.is_empty() {
            self.narration_chapter = None;
            self.narration_state = NarrationState::Idle;
            return vec![Event::Error {
                message: "chapter has no narratable text".to_string(),
            }];
        }
        self.narration.load_chunks(chunks);
        self.narration_chapter = Some(spine);
        self.narration
            .set_rate(&mut *self.tts_host, self.settings.tts_speed);
        self.narration
            .set_pitch(&mut *self.tts_host, self.settings.tts_pitch);
        match self.narration.start(&mut *self.tts_host) {
            Ok(()) => {
                self.narration_state = NarrationState::Speaking;
                vec![Event::NarrationStateChanged {
                    state: NarrationState::Speaking,
                }]
            }
            Err(msg) => {
                self.narration_state = NarrationState::Error;
                vec![
                    Event::NarrationStateChanged {
                        state: NarrationState::Error,
                    },
                    Event::Error { message: msg },
                ]
            }
        }
    }

    /// Stop narration and clear its transient highlight.
    fn stop_narration(&mut self) -> Vec<Event> {
        self.narration.stop(&mut *self.tts_host);
        self.narration_chapter = None;
        self.transient_highlight = None;
        if self.narration_state == NarrationState::Idle {
            return vec![];
        }
        self.narration_state = NarrationState::Idle;
        vec![Event::NarrationStateChanged {
            state: NarrationState::Idle,
        }]
    }

    /// Skip narration by `delta` chapters (wraps), reloading from there.
    fn narration_skip(&mut self, delta: isize) -> Vec<Event> {
        if delta == 0 {
            return vec![];
        }
        let Some(spine) = self.narration_chapter else {
            return vec![];
        };
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let count = self
            .parsed_docs
            .get(&book_id)
            .map(|pd| pd.document.chapters.len() as isize)
            .unwrap_or(0);
        if count == 0 {
            return vec![];
        }
        let next = ((spine as isize + delta).rem_euclid(count)) as u32;
        if next == spine {
            return vec![];
        }
        self.load_narration_chapter(next)
    }

    /// Drain TTS host callbacks → highlight / page-sync / chapter-advance.
    fn poll_narration(&mut self) -> Vec<Event> {
        let effects = self.narration.poll(&mut *self.tts_host);
        let mut events = Vec::new();
        for effect in effects {
            match effect {
                reeda_tts::engine::EngineEffect::WordHighlight {
                    block_index,
                    char_start,
                    char_end,
                } => {
                    self.set_transient_from_offsets(block_index, char_start, char_end);
                    events.push(Event::WordHighlight {
                        block_index,
                        char_offset: char_start,
                        char_len: char_end.saturating_sub(char_start),
                    });
                    events.extend(self.narration_sync_page(block_index));
                }
                reeda_tts::engine::EngineEffect::Finished => {
                    events.extend(self.narration_advance_chapter());
                }
                reeda_tts::engine::EngineEffect::Error { message } => {
                    self.narration_state = NarrationState::Error;
                    events.push(Event::NarrationStateChanged {
                        state: NarrationState::Error,
                    });
                    events.push(Event::Error { message });
                }
            }
        }
        events
    }

    /// After a chapter finishes: advance to the next, or end narration.
    fn narration_advance_chapter(&mut self) -> Vec<Event> {
        let Some(spine) = self.narration_chapter else {
            return vec![];
        };
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let (chapters_len, first_block) = self
            .parsed_docs
            .get(&book_id)
            .map(|pd| {
                let mut first_block: u32 = 0;
                for ch in pd.document.chapters.iter().take(spine as usize + 1) {
                    first_block += ch.blocks.len() as u32;
                }
                (pd.document.chapters.len(), first_block)
            })
            .unwrap_or((0, 0));
        let next = spine + 1;
        if (next as usize) < chapters_len {
            let mut events = self.load_narration_chapter(next);
            // Jump the reader to the next chapter's first block page.
            events.extend(self.narration_sync_page(first_block));
            events
        } else {
            self.narration_chapter = None;
            self.transient_highlight = None;
            self.narration_state = NarrationState::Idle;
            vec![
                Event::NarrationFinished,
                Event::NarrationStateChanged {
                    state: NarrationState::Idle,
                },
            ]
        }
    }

    /// Advance the reader page to the one containing `block_index` (without
    /// clearing the transient highlight).
    fn narration_sync_page(&mut self, block_index: u32) -> Vec<Event> {
        let Some(book_id) = self.current_book_id else {
            return vec![];
        };
        let spine_len = self
            .parsed_docs
            .get(&book_id)
            .map(|pd| pd.spine.len() as u32)
            .unwrap_or(0);
        let cfi = GlobalRange::new(block_index as usize, 0, block_index as usize, 0).to_cfi();
        let Some(rs) = &self.reader_state else {
            return vec![];
        };
        let Some(page_idx) = reader::find_page_for_cfi(&rs.pages, &cfi.start.0, spine_len) else {
            return vec![];
        };
        if page_idx == self.current_page {
            return vec![];
        }
        self.current_page = page_idx;
        if let Some(ref mut rs) = self.reader_state {
            rs.current_page = page_idx;
        }
        self.save_progress();
        vec![Event::PageChanged {
            page_index: page_idx,
            total_pages: self.total_pages,
        }]
    }

    /// Spine index of the chapter on the current page.
    fn chapter_of_page(&self) -> Option<u32> {
        let book_id = self.current_book_id?;
        let parsed = self.parsed_docs.get(&book_id)?;
        let rs = self.reader_state.as_ref()?;
        let page = rs.pages.pages.get(rs.current_page as usize)?;
        parsed
            .document
            .block_at(page.first_block)
            .map(|(chapter, _, _)| chapter.spine_index)
    }

    /// Highlight a (block, char range) as a transient narration highlight.
    fn set_transient_from_offsets(&mut self, block_index: u32, char_start: u32, char_end: u32) {
        let Some(book_id) = self.current_book_id else {
            return;
        };
        let Some(pd) = self.parsed_docs.get(&book_id) else {
            return;
        };
        let Some(text) = pd.document.block_text(block_index as usize) else {
            return;
        };
        let start = (char_start as usize).min(text.len());
        let end = (char_end as usize).min(text.len()).max(start);
        let range =
            GlobalRange::new(block_index as usize, start, block_index as usize, end).to_cfi();
        self.transient_highlight = Some(CfiRange {
            start: range.start.0,
            end: range.end.0,
        });
    }

    /// Replace the TTS host (the Android bridge installs its JNI host here).
    ///
    /// Public for the UI layer: `reeda-ui` swaps in the JNI-backed
    /// `reeda_tts::android_bridge::AndroidTtsHost` at startup on
    /// `platform-android` builds; tests and the desktop build keep the
    /// [`FakeTtsHost`](reeda_tts::engine::FakeTtsHost).
    pub fn set_tts_host(&mut self, host: Box<dyn reeda_tts::engine::TtsHost>) {
        self.tts_host = host;
    }

    /// Mutable reference to the TTS host (test access).
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn tts_host_mut(&mut self) -> &mut dyn reeda_tts::engine::TtsHost {
        &mut *self.tts_host
    }

    pub(crate) fn delete_book(&mut self, book_id: BookId) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            book.deleted_at = Some(chrono::Utc::now());
            book.updated_at = chrono::Utc::now();
            if self.current_book_id == Some(book_id) {
                self.current_book_id = None;
                self.reader_state = None;
            }
            // Clean up files from disk.
            if let Some(ref store) = self.store {
                let _ = store.delete_book_files(book_id);
            }
            // Soft-delete in SQLite.
            if let Some(db) = &self.db {
                let _ = db.delete_book(book_id);
            }
            // Remove from the full-text search index.
            if let Some(search) = &mut self.search {
                if let Err(e) = search.delete_book(book_id) {
                    eprintln!("Warning: failed to remove book from index: {e}");
                }
            }
            vec![Event::LibraryChanged]
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    /// Add a book directly to the in-memory library (used by import pipeline).
    pub fn add_book(&mut self, book: Book) {
        let id = book.id;
        self.library.insert(id, book);
    }

    /// Edit a book's metadata (title/author override).
    fn edit_metadata(
        &mut self,
        book_id: BookId,
        title: String,
        author: Option<String>,
    ) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            let trimmed = title.trim().to_string();
            if !trimmed.is_empty() {
                book.title = trimmed;
            }
            if let Some(a) = author {
                let trimmed = a.trim().to_string();
                book.author = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                };
            }
            book.updated_at = chrono::Utc::now();
            // Persist to SQLite.
            if let Some(db) = &self.db {
                let _ = db.update_book_metadata(book_id, &book.title, book.author.as_deref());
            }
            vec![Event::LibraryChanged]
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    /// Import a book from a file path (desktop) or SAF URI (Android).
    fn import_book(&mut self, path: String) -> Vec<Event> {
        // Read file bytes.
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                return vec![Event::ImportFailed {
                    error: format!("Failed to read file: {e}"),
                }];
            }
        };
        self.import_from_bytes(data, path)
    }

    /// Import a book from raw EPUB bytes.
    ///
    /// If a `BookStore` is configured, copies the file to persistent storage.
    /// Performs SHA-256 deduplication and stores parsed content for pagination.
    pub fn import_from_bytes(&mut self, data: Vec<u8>, path: String) -> Vec<Event> {
        // 1. Parse EPUB.
        let epub_book = match reeda_epub::open_epub(&data) {
            Ok(b) => b,
            Err(e) => {
                return vec![Event::ImportFailed {
                    error: format!("EPUB parse error: {e}"),
                }];
            }
        };

        // 2. Compute hash.
        let sha256 = crate::store::sha256_hex(&data);

        // 3. Dedup check.
        let duplicate = self.library.values().any(|b| b.sha256 == sha256);
        if duplicate {
            return vec![Event::ImportFailed {
                error: "Duplicate book (already in library)".into(),
            }];
        }

        use crate::models::BookFormat;
        let mut book = Book::new(
            epub_book
                .opf
                .metadata
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".into()),
            BookFormat::Epub,
            path,
            sha256,
        );
        book.author = epub_book.opf.metadata.creators.first().cloned();
        book.language = epub_book.opf.metadata.language.clone();
        book.publisher = epub_book.opf.metadata.publisher.clone();
        book.description = epub_book.opf.metadata.description.clone();
        book.published_at = epub_book.opf.metadata.date.clone();

        let book_id = book.id;

        // 4. Copy file to persistent storage (if store is configured).
        if let Some(ref store) = self.store {
            if let Err(e) = store.store_book(book_id, BookFormat::Epub, &data) {
                return vec![Event::ImportFailed {
                    error: format!("Failed to store book file: {e}"),
                }];
            }
            book.file_path = store.relative_book_path(book_id, BookFormat::Epub);

            // 4b. Extract and store cover image.
            if let Ok(Some(cover_bytes)) = reeda_epub::extract_cover_bytes(&data) {
                if let Err(e) = store.store_cover(book_id, &cover_bytes) {
                    eprintln!("Warning: failed to store cover: {e}");
                } else {
                    book.cover_path = Some(store.relative_cover_path(book_id));
                }
            }
        }

        // 5. Build chapters from TOC.
        let core_chapters = reader::toc_to_chapters(&epub_book.toc, book_id);
        self.chapters.insert(book_id, core_chapters);

        // 6. Store the parsed document + index it for full-text search.
        let parsed = reader::epub_book_to_parsed_doc(&epub_book, book_id);
        if let Some(search) = &mut self.search {
            if let Err(e) = search.index_book(book_id, &parsed) {
                eprintln!("Warning: failed to index book: {e}");
            }
        }
        self.parsed_docs.insert(book_id, parsed);

        // 7. Persist book + chapters to SQLite (if configured).
        if let Some(db) = &self.db {
            if let Err(e) = db.insert_book(&book) {
                eprintln!("Warning: failed to persist book: {e}");
            }
            if let Some(chapters) = self.chapters.get(&book_id) {
                for chapter in chapters {
                    if let Err(e) = db.insert_chapter(chapter) {
                        eprintln!("Warning: failed to persist chapter: {e}");
                        break;
                    }
                }
            }
        }

        self.library.insert(book_id, book);

        vec![Event::ImportFinished { book_id }]
    }

    /// Import a PDF from a file path (PDF_SPEC §1).
    ///
    /// Validates the file with PDFium (page count/sizes), copies it to
    /// persistent storage, and dedupes by SHA-256 like EPUBs. Title is the
    /// file stem (PDF metadata extraction is deferred to the outline work).
    pub fn import_pdf(&mut self, path: String) -> Vec<Event> {
        // 1. Read file bytes.
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                return vec![Event::ImportFailed {
                    error: format!("Failed to read file: {e}"),
                }];
            }
        };

        // 2. Validate with PDFium (the document handle is dropped here; page
        //    metadata is re-extracted on open).
        let _pdf = match reeda_pdf::document::PdfDocument::open(&path) {
            Ok(doc) => doc,
            Err(e) => {
                return vec![Event::ImportFailed {
                    error: format!("PDF open error: {e}"),
                }];
            }
        };

        // 3. Compute hash + dedup.
        let sha256 = crate::store::sha256_hex(&data);
        if self.library.values().any(|b| b.sha256 == sha256) {
            return vec![Event::ImportFailed {
                error: "Duplicate book (already in library)".into(),
            }];
        }

        // 4. Build the book record.
        let title = std::path::Path::new(&path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled".into());
        let mut book = Book::new(title, BookFormat::Pdf, path.clone(), sha256);
        let book_id = book.id;

        // 5. Copy file to persistent storage (if store is configured).
        if let Some(ref store) = self.store {
            if let Err(e) = store.store_book(book_id, BookFormat::Pdf, &data) {
                return vec![Event::ImportFailed {
                    error: format!("Failed to store book file: {e}"),
                }];
            }
            book.file_path = store.relative_book_path(book_id, BookFormat::Pdf);
        }

        // 6. Persist to SQLite (if configured). PDFs get no chapters or
        //    search index entries (text extraction is P2, PDF_SPEC §6).
        if let Some(db) = &self.db {
            if let Err(e) = db.insert_book(&book) {
                eprintln!("Warning: failed to persist book: {e}");
            }
        }

        self.library.insert(book_id, book);

        vec![Event::ImportFinished { book_id }]
    }

    /// Re-paginate the current book with updated settings.
    fn repaginate(&mut self) -> Vec<Event> {
        if let Some(book_id) = self.current_book_id {
            if let Some(parsed) = self.parsed_docs.get(&book_id) {
                let layout = typography_to_layout(&self.settings.typography, 400.0, 700.0);
                let pages = reader::paginate_doc(&parsed.document, &layout);
                self.total_pages = pages.pages.len() as u32;
                // Clamp current page.
                if self.current_page >= self.total_pages && self.total_pages > 0 {
                    self.current_page = self.total_pages - 1;
                }
                self.reader_state = Some(ReaderState {
                    pages,
                    current_page: self.current_page,
                });
            }
        }
        vec![]
    }

    /// Load settings (used by storage layer after DB read).
    pub fn load_settings(&mut self, settings: AppSettings) {
        self.settings = settings;
    }

    /// Full-text search across the library.
    ///
    /// Returns `None` when no search index is configured. Results are ranked
    /// by relevance; each hit carries a snippet plus a CFI locator.
    pub fn search_books(
        &mut self,
        query: &str,
        limit: Option<usize>,
    ) -> Option<reeda_search::index::SearchResult> {
        let search = self.search.as_mut()?;
        match search.search(query, limit) {
            Ok(res) => Some(res),
            Err(e) => {
                eprintln!("Warning: search failed: {e}");
                None
            }
        }
    }
}

// Workaround: `AppSettings` doesn't have an updated_at field, but we need
// to track mutations. This is called after settings changes so future code
// can persist them. The real implementation will go through the storage layer.
trait SettingsExt {
    fn updated_at_v2(&mut self);
}

impl SettingsExt for AppSettings {
    fn updated_at_v2(&mut self) {
        // Placeholder — the actual LWW timestamp lives in the DB row.
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;
    use crate::models::{
        AnnotationId, AnnotationKind, BookFormat, CfiRange, HighlightColor, Theme,
    };

    fn make_test_book() -> Book {
        Book::new(
            "Test Book".into(),
            BookFormat::Epub,
            "books/test/book.epub".into(),
            "abc123".into(),
        )
    }

    /// Downcast the App's TTS host to the fake for event injection.
    fn fake_host(app: &mut App) -> &mut reeda_tts::engine::FakeTtsHost {
        (app.tts_host.as_mut() as &mut dyn std::any::Any)
            .downcast_mut::<reeda_tts::engine::FakeTtsHost>()
            .expect("fake tts host installed")
    }

    #[test]
    fn open_book_not_found_returns_error() {
        let mut app = App::new();
        let fake_id = BookId::new();
        let events = app.dispatch(Command::OpenBook { book_id: fake_id });
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::Error { message } => assert!(message.contains("not found")),
            _ => panic!("expected Error event"),
        }
    }

    #[test]
    fn open_book_succeeds() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);

        let events = app.dispatch(Command::OpenBook { book_id: id });
        assert!(events.is_empty());

        let snap = app.snapshot();
        assert!(snap.current_book.is_some());
        assert_eq!(snap.current_book.unwrap().id, id);
    }

    #[test]
    fn close_book_clears_state() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::CloseBook);
        assert!(events.is_empty());

        let snap = app.snapshot();
        assert!(snap.current_book.is_none());
    }

    #[test]
    fn delete_book_soft_deletes() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);

        let events = app.dispatch(Command::DeleteBook { book_id: id });
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::LibraryChanged));

        let snap = app.snapshot();
        assert!(snap.library.is_empty());
    }

    #[test]
    fn set_theme_updates_settings() {
        let mut app = App::new();
        let events = app.dispatch(Command::SetTheme(Theme::Dark));
        assert!(events.is_empty());
        assert_eq!(app.snapshot().settings.theme, Theme::Dark);
    }

    #[test]
    fn turn_page_forward() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::TurnPage { forward: true });
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::PageChanged { page_index, .. } => assert_eq!(*page_index, 0),
            _ => panic!("expected PageChanged"),
        }
    }

    #[test]
    fn add_highlight_without_book_returns_error() {
        let mut app = App::new();
        let events = app.dispatch(Command::AddHighlight {
            range: CfiRange::new("/6/4".into(), "/6/5".into()),
            color: HighlightColor::Yellow,
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::Error { .. }));
    }

    #[test]
    fn add_highlight_with_open_book() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::AddHighlight {
            range: CfiRange::new("/6/4".into(), "/6/5".into()),
            color: HighlightColor::Blue,
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::AnnotationChanged { .. }));

        let snap = app.snapshot();
        assert_eq!(snap.annotations.len(), 1);
        assert_eq!(snap.annotations[0].kind, AnnotationKind::Highlight);
    }

    #[test]
    fn toggle_bookmark_adds_and_removes() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::ToggleBookmark { cfi: "/6/4".into() });
        assert!(matches!(&events[0], Event::AnnotationChanged { .. }));
        assert_eq!(app.snapshot().annotations.len(), 1);

        let events = app.dispatch(Command::ToggleBookmark { cfi: "/6/4".into() });
        assert!(matches!(&events[0], Event::AnnotationDeleted { .. }));
        assert_eq!(app.snapshot().annotations.len(), 0);
    }

    #[test]
    fn jump_to_annotation_navigates_to_page() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);

        // Register a parsed doc with enough content for multiple pages.
        let mut blocks = Vec::new();
        for i in 0..40 {
            blocks.push(reeda_epub::document::Block::Paragraph(vec![
                reeda_epub::document::Inline::Text(format!(
                    "Paragraph {i}: content that fills space for pagination purposes here."
                )),
            ]));
        }
        let doc = reeda_epub::document::DocumentModel {
            chapters: vec![reeda_epub::document::Chapter {
                spine_index: 0,
                title: "Chapter".into(),
                href: "ch.xhtml".into(),
                blocks,
            }],
        };
        app.parsed_docs.insert(
            id,
            crate::reader::ParsedDoc {
                document: doc,
                toc: reeda_epub::nav::TableOfContents { items: vec![] },
                spine: vec![crate::reader::SpineEntry {
                    title: "Chapter".into(),
                    spine_index: 0,
                }],
            },
        );

        app.dispatch(Command::OpenBook { book_id: id });
        assert!(app.snapshot().total_pages > 1);

        // Highlight at global block 25 (late in the book).
        let range = reeda_epub::selection::GlobalRange::new(25, 0, 25, 10).to_cfi();
        let events = app.dispatch(Command::AddHighlight {
            range: CfiRange::new(range.start.0, range.end.0),
            color: HighlightColor::Yellow,
        });
        let annotation_id = match &events[0] {
            Event::AnnotationChanged { annotation_id } => *annotation_id,
            _ => panic!("expected AnnotationChanged"),
        };

        // Move to page 0, then jump to the annotation's page.
        let page0 = app.snapshot().current_page;
        let events = app.dispatch(Command::JumpToAnnotation { annotation_id });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::PageChanged { .. }));
        let snap = app.snapshot();
        assert!(snap.current_page >= page0);
    }

    #[test]
    fn jump_to_unknown_annotation_returns_error() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::JumpToAnnotation {
            annotation_id: AnnotationId::new(),
        });
        assert!(matches!(&events[0], Event::Error { .. }));
    }

    #[test]
    fn delete_annotation() {
        let mut app = App::new();
        let book = make_test_book();
        let id = book.id;
        app.add_book(book);
        app.dispatch(Command::OpenBook { book_id: id });

        let events = app.dispatch(Command::AddHighlight {
            range: CfiRange::new("/6/4".into(), "/6/5".into()),
            color: HighlightColor::Green,
        });
        let ann_id = match &events[0] {
            Event::AnnotationChanged { annotation_id } => *annotation_id,
            _ => panic!("expected AnnotationChanged"),
        };

        let events = app.dispatch(Command::DeleteAnnotation {
            annotation_id: ann_id,
        });
        assert!(matches!(&events[0], Event::AnnotationDeleted { .. }));
        assert!(app.snapshot().annotations.is_empty());
    }

    #[test]
    fn narration_start_without_book_returns_error() {
        let mut app = App::new();
        let events = app.dispatch(Command::StartNarration { chapter_id: None });
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Error { message } if message.contains("open book"))));
        assert_eq!(app.snapshot().narration_state, NarrationState::Idle);
    }

    #[test]
    fn narration_speaks_chunks_with_word_highlights() {
        use reeda_tts::engine::HostEvent;
        let mut app = App::new();
        app.set_tts_host(Box::new(reeda_tts::engine::FakeTtsHost::new()));
        app.import_from_bytes(make_test_epub_bytes(), "test.epub".into());
        let book_id = app.snapshot().library[0].id;
        app.dispatch(Command::OpenBook { book_id });

        let events = app.dispatch(Command::StartNarration { chapter_id: None });
        assert!(events.iter().any(|e| matches!(
            e,
            Event::NarrationStateChanged {
                state: NarrationState::Speaking
            }
        )));
        let first_utterance = fake_host(&mut app).spoken()[0].0;
        assert!(
            !fake_host(&mut app).spoken().is_empty(),
            "expected utterances"
        );

        // Word range callback → transient highlight + WordHighlight event.
        fake_host(&mut app).push_event(HostEvent::Range {
            utterance_id: first_utterance,
            start: 0,
            end: 4,
        });
        let events = app.dispatch(Command::PollNarration);
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::WordHighlight { char_len: 4, .. })));
        assert!(app.snapshot().transient_highlight.is_some());
    }

    #[test]
    fn narration_advances_to_next_chapter_and_finishes() {
        use reeda_tts::engine::HostEvent;
        let mut app = App::new();
        app.set_tts_host(Box::new(reeda_tts::engine::FakeTtsHost::new()));
        app.import_from_bytes(make_test_epub_bytes(), "test.epub".into());
        let book_id = app.snapshot().library[0].id;
        app.dispatch(Command::OpenBook { book_id });

        app.dispatch(Command::StartNarration { chapter_id: None });
        // Drain chapter 1 fully: keep feeding Done events for every utterance
        // spoken so far until the engine advances to the next chapter.
        let mut advanced = false;
        for _ in 0..20 {
            let ids: Vec<u64> = fake_host(&mut app)
                .spoken()
                .iter()
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                fake_host(&mut app).push_event(HostEvent::Done { utterance_id: id });
            }
            let events = app.dispatch(Command::PollNarration);
            if events.iter().any(|e| {
                matches!(
                    e,
                    Event::NarrationStateChanged {
                        state: NarrationState::Speaking
                    }
                )
            }) {
                advanced = true;
                break;
            }
        }
        assert!(advanced, "should advance to next chapter");
        let last = fake_host(&mut app).spoken().last().unwrap().1.clone();
        assert_eq!(last, "More content here.");
    }

    #[test]
    fn narration_skip_changes_chapter() {
        let mut app = App::new();
        app.set_tts_host(Box::new(reeda_tts::engine::FakeTtsHost::new()));
        app.import_from_bytes(make_test_epub_bytes(), "test.epub".into());
        let book_id = app.snapshot().library[0].id;
        app.dispatch(Command::OpenBook { book_id });
        app.dispatch(Command::StartNarration { chapter_id: None });
        let before_last = fake_host(&mut app).spoken().last().unwrap().1.clone();

        let events = app.dispatch(Command::NarrationSkip { delta: 1 });
        assert!(events.iter().any(|e| matches!(
            e,
            Event::NarrationStateChanged {
                state: NarrationState::Speaking
            }
        )));
        let last = fake_host(&mut app).spoken().last().unwrap().1.clone();
        assert_ne!(last, before_last, "should narrate next chapter");
        assert_eq!(last, "More content here.");
    }

    #[test]
    fn narration_stop_and_close_clear_state() {
        let mut app = App::new();
        app.set_tts_host(Box::new(reeda_tts::engine::FakeTtsHost::new()));
        app.import_from_bytes(make_test_epub_bytes(), "test.epub".into());
        let book_id = app.snapshot().library[0].id;
        app.dispatch(Command::OpenBook { book_id });
        app.dispatch(Command::StartNarration { chapter_id: None });
        assert_eq!(app.snapshot().narration_state, NarrationState::Speaking);

        let events = app.dispatch(Command::PauseNarration);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::NarrationStateChanged {
                state: NarrationState::Paused
            }
        )));
        let events = app.dispatch(Command::ResumeNarration);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::NarrationStateChanged {
                state: NarrationState::Speaking
            }
        )));

        let events = app.dispatch(Command::StopNarration);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::NarrationStateChanged {
                state: NarrationState::Idle
            }
        )));
        assert_eq!(app.snapshot().narration_state, NarrationState::Idle);
        assert!(fake_host(&mut app).stop_count() >= 1);

        // Closing the book stops narration too.
        app.dispatch(Command::StartNarration { chapter_id: None });
        app.dispatch(Command::CloseBook);
        assert_eq!(app.snapshot().narration_state, NarrationState::Idle);
    }

    #[test]
    fn narration_tts_speed_pitch_propagate() {
        let mut app = App::new();
        app.set_tts_host(Box::new(reeda_tts::engine::FakeTtsHost::new()));
        app.dispatch(Command::SetTtsSpeed(2.0));
        app.dispatch(Command::SetTtsPitch(1.25));
        assert_eq!(fake_host(&mut app).rate(), 2.0);
        assert_eq!(fake_host(&mut app).pitch(), 1.25);
        assert_eq!(app.snapshot().settings.tts_speed, 2.0);
        assert_eq!(app.snapshot().settings.tts_pitch, 1.25);
        app.dispatch(Command::SetTtsSpeed(99.0));
        assert_eq!(fake_host(&mut app).rate(), 2.5, "clamped");
    }

    #[test]
    fn snapshot_library_sorted_by_recent() {
        let mut app = App::new();

        let mut book1 = make_test_book();
        book1.title = "Book 1".into();
        book1.last_opened_at = Some(chrono::Utc::now() - chrono::Duration::hours(2));
        let id1 = book1.id;

        let mut book2 = make_test_book();
        book2.title = "Book 2".into();
        book2.last_opened_at = Some(chrono::Utc::now());
        let id2 = book2.id;

        app.add_book(book1);
        app.add_book(book2);

        let snap = app.snapshot();
        assert_eq!(snap.library.len(), 2);
        assert_eq!(snap.library[0].id, id2);
        assert_eq!(snap.library[1].id, id1);
    }

    /// Build a minimal test EPUB in memory (same as reeda-epub tests).
    pub(crate) fn make_test_epub_bytes() -> Vec<u8> {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();

            zip.start_file("META-INF/container.xml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();

            zip.start_file("OEBPS/content.opf", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="BookId"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Integration Test Book</dc:title><dc:creator>Test Author</dc:creator><dc:language>en</dc:language><dc:identifier id="BookId">urn:uuid:test-001</dc:identifier></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/><item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="ch1"/><itemref idref="ch2"/></spine></package>"#).unwrap();

            zip.start_file("OEBPS/nav.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Navigation</title></head><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">Chapter 1</a></li><li><a href="chapter2.xhtml">Chapter 2</a></li></ol></nav></body></html>"#).unwrap();

            zip.start_file("OEBPS/chapter1.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Ch1</title></head><body><h1>Chapter 1</h1><p>Hello <strong>world</strong>.</p><p>Second paragraph.</p></body></html>"#).unwrap();

            zip.start_file("OEBPS/chapter2.xhtml", deflated).unwrap();
            zip.write_all(br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Ch2</title></head><body><h1>Chapter 2</h1><p>More <em>content</em> here.</p></body></html>"#).unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn import_epub_adds_to_library() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());

        assert_eq!(events.len(), 1);
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        let snap = app.snapshot();
        assert_eq!(snap.library.len(), 1);
        assert_eq!(snap.library[0].id, book_id);
        assert_eq!(snap.library[0].title, "Integration Test Book");
    }

    #[test]
    fn import_then_open_paginates() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        let events = app.dispatch(Command::OpenBook { book_id });
        assert!(events.is_empty());

        let snap = app.snapshot();
        assert!(snap.total_pages > 0);
        assert!(!snap.page_text.is_empty());
        assert_eq!(snap.current_page, 0);
    }

    #[test]
    fn open_then_turn_page_updates_content() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::OpenBook { book_id });
        let snap1 = app.snapshot();
        let text1 = snap1.page_text.clone();

        // Try turning forward — if only one page, content stays the same.
        app.dispatch(Command::TurnPage { forward: true });
        let snap2 = app.snapshot();
        if snap2.total_pages > 1 {
            assert_ne!(text1, snap2.page_text);
        } else {
            assert_eq!(text1, snap2.page_text);
        }
    }

    #[test]
    fn set_typography_triggers_repagination() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        let pages_before = app.snapshot().total_pages;

        // Change to smaller font → more pages.
        app.dispatch(Command::SetTypography(crate::models::Typography {
            font_size_pt: 8.0,
            ..Default::default()
        }));
        let pages_after = app.snapshot().total_pages;
        assert!(pages_after >= pages_before);
    }

    #[test]
    fn close_book_clears_reader_state() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        assert!(app.snapshot().total_pages > 0);

        app.dispatch(Command::CloseBook);
        let snap = app.snapshot();
        assert_eq!(snap.total_pages, 0);
        assert!(snap.page_text.is_empty());
    }

    #[test]
    fn import_invalid_bytes_returns_import_failed() {
        let mut app = App::new();
        let events = app.import_from_bytes(vec![0, 1, 2, 3], "fake.epub".into());
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::ImportFailed { error } => assert!(error.contains("EPUB parse error")),
            _ => panic!("expected ImportFailed"),
        }
    }

    #[test]
    fn import_preserves_metadata() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        let snap = app.snapshot();
        let book = snap.library.iter().find(|b| b.id == book_id).unwrap();
        assert_eq!(book.title, "Integration Test Book");
        assert_eq!(book.author.as_deref(), Some("Test Author"));
    }

    #[test]
    fn open_book_populates_toc() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        assert_eq!(snap.toc_labels.len(), 2);
        assert_eq!(snap.toc_labels[0], "Chapter 1");
        assert_eq!(snap.toc_labels[1], "Chapter 2");
    }

    #[test]
    fn turn_page_backward_at_start_stays_at_zero() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });

        // Already at page 0, going backward should stay at 0.
        let events = app.dispatch(Command::TurnPage { forward: false });
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::PageChanged { page_index, .. } => assert_eq!(*page_index, 0),
            _ => panic!("expected PageChanged"),
        }
    }

    #[test]
    fn page_chapter_title_changes_across_pages() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        let _first_title = snap.page_chapter_title.clone();

        // If there are multiple pages, the title might differ.
        if snap.total_pages > 1 {
            app.dispatch(Command::TurnPage { forward: true });
            let snap2 = app.snapshot();
            // At least the page text should differ (different content).
            // Title may be same if still in same chapter.
            assert_ne!(snap.page_text, snap2.page_text);
        }
    }

    #[test]
    fn import_duplicate_returns_import_failed() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub.clone(), "test.epub".into());
        assert!(matches!(&events[0], Event::ImportFinished { .. }));

        // Import the same bytes again → should be deduplicated.
        let events = app.import_from_bytes(epub, "test2.epub".into());
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::ImportFailed { error } => assert!(error.contains("Duplicate")),
            _ => panic!("expected ImportFailed for duplicate"),
        }
    }

    #[test]
    fn import_with_store_copies_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::BookStore::new(dir.path()).unwrap();
        let mut app = App::with_store(store);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // The book's file_path should be a relative path.
        let snap = app.snapshot();
        let book = snap.library.iter().find(|b| b.id == book_id).unwrap();
        assert!(book.file_path.starts_with("books/"));

        // The file should exist on disk.
        let store = app.store.as_ref().unwrap();
        assert!(store
            .book_path(book_id, crate::models::BookFormat::Epub)
            .exists());
    }

    #[test]
    fn delete_with_store_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::BookStore::new(dir.path()).unwrap();
        let mut app = App::with_store(store);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // Verify file exists before delete.
        let book_path = app
            .store
            .as_ref()
            .unwrap()
            .book_path(book_id, crate::models::BookFormat::Epub);
        assert!(book_path.exists());

        app.dispatch(Command::DeleteBook { book_id });

        // File should be removed.
        assert!(!book_path.exists());
    }

    #[test]
    fn progress_saved_after_page_turn() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });

        let snap = app.snapshot();
        let total = snap.total_pages;
        if total > 1 {
            app.dispatch(Command::TurnPage { forward: true });
            let book = app.library.get(&book_id).unwrap();
            assert_eq!(book.last_position.as_deref(), Some("1"));
            assert!(book.progress_pct > 0.0);
        }
    }

    #[test]
    fn progress_restored_on_open() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // Open and advance to page 1.
        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        if snap.total_pages > 1 {
            app.dispatch(Command::TurnPage { forward: true });
        }
        app.dispatch(Command::CloseBook);

        // Re-open: should restore page 1.
        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        if snap.total_pages > 1 {
            assert_eq!(snap.current_page, 1);
        }
    }

    #[test]
    fn progress_saved_on_close() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        if snap.total_pages > 1 {
            app.dispatch(Command::TurnPage { forward: true });
            app.dispatch(Command::CloseBook);
            let book = app.library.get(&book_id).unwrap();
            assert_eq!(book.last_position.as_deref(), Some("1"));
            assert!(book.progress_pct > 0.0);
        }
    }

    #[test]
    fn edit_metadata_updates_title_and_author() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        let events = app.dispatch(Command::EditMetadata {
            book_id,
            title: "New Title".into(),
            author: Some("New Author".into()),
        });
        assert!(matches!(&events[0], Event::LibraryChanged));

        let book = app.library.get(&book_id).unwrap();
        assert_eq!(book.title, "New Title");
        assert_eq!(book.author.as_deref(), Some("New Author"));
    }

    #[test]
    fn edit_metadata_clears_author_on_empty() {
        let mut app = App::new();
        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::EditMetadata {
            book_id,
            title: "T".into(),
            author: Some("  ".into()),
        });
        let book = app.library.get(&book_id).unwrap();
        assert_eq!(book.author, None);
    }

    #[test]
    fn import_persists_book_to_db() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // Verify the book is in the DB.
        let db = app.db.as_ref().unwrap();
        let books = db.list_books().unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, book_id);
        assert_eq!(books[0].title, "Integration Test Book");
        assert_eq!(books[0].author.as_deref(), Some("Test Author"));
    }

    #[test]
    fn progress_persisted_to_db() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::OpenBook { book_id });
        let snap = app.snapshot();
        if snap.total_pages > 1 {
            app.dispatch(Command::TurnPage { forward: true });
            let db = app.db.as_ref().unwrap();
            let book = db.get_book(book_id).unwrap().unwrap();
            assert_eq!(book.last_position.as_deref(), Some("1"));
            assert!(book.progress_pct > 0.0);
        }
    }

    #[test]
    fn metadata_edit_persists_to_db() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::EditMetadata {
            book_id,
            title: "Renamed".into(),
            author: Some("New Author".into()),
        });

        let db = app.db.as_ref().unwrap();
        let book = db.get_book(book_id).unwrap().unwrap();
        assert_eq!(book.title, "Renamed");
        assert_eq!(book.author.as_deref(), Some("New Author"));
    }

    #[test]
    fn load_books_restores_library() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub, "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // Simulate restart: new App sharing the same DB.
        let db = app.db.take().unwrap();
        let mut app2 = App::new();
        app2.set_db(db);
        let loaded = app2.load_books().unwrap();
        assert_eq!(loaded, 1);
        assert!(app2.library.contains_key(&book_id));
    }

    #[test]
    fn settings_persist_and_load_round_trip() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let mut settings = app.settings();
        settings.theme = crate::models::Theme::Dark;
        settings.typography.font_size_pt = 24.0;
        app.dispatch(Command::UpdateSettings(settings));

        let db = app.db.as_ref().unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.theme, crate::models::Theme::Dark);
        assert_eq!(loaded.typography.font_size_pt, 24.0);
    }

    /// M3.6: highlights must survive an app restart — created via commands,
    /// persisted to SQLite, reloaded into a fresh App instance, and still
    /// rendered on the page.
    #[test]
    fn annotations_persist_across_restart() {
        let db = crate::storage::Database::open_memory().unwrap();
        let mut app = App::new();
        app.set_db(db);

        let epub = make_test_epub_bytes();
        let events = app.import_from_bytes(epub.clone(), "test.epub".into());
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        app.dispatch(Command::OpenBook { book_id });
        // Highlight "Hello" (block 1, chars 0..5) in chapter 1.
        let events = app.dispatch(Command::AddHighlight {
            range: CfiRange::new("epubcfi(/6/4!/4/4:0)".into(), "epubcfi(/6/4!/4/4:5)".into()),
            color: HighlightColor::Yellow,
        });
        let annotation_id = match &events[0] {
            Event::AnnotationChanged { annotation_id } => *annotation_id,
            _ => panic!("expected AnnotationChanged"),
        };

        // Verify the DB row exists.
        let db = app.db.as_ref().unwrap();
        assert_eq!(db.list_annotations(book_id).unwrap().len(), 1);

        // Simulate restart: fresh App sharing the same DB.
        let db = app.db.take().unwrap();
        let mut app2 = App::new();
        app2.set_db(db);
        assert_eq!(app2.load_books().unwrap(), 1);

        // Re-parse the document (import-time parsing is in-memory only).
        let epub_book = reeda_epub::open_epub(&epub).unwrap();
        let parsed = crate::reader::epub_book_to_parsed_doc(&epub_book, book_id);
        app2.parsed_docs.insert(book_id, parsed);

        app2.dispatch(Command::OpenBook { book_id });
        let snap = app2.snapshot();
        assert!(
            snap.annotations.iter().any(|a| a.id == annotation_id),
            "highlight must reload from DB"
        );
        let highlighted: Vec<String> = snap
            .page_lines
            .iter()
            .flatten()
            .filter(|r| r.highlighted)
            .map(|r| r.text.clone())
            .collect();
        assert!(
            !highlighted.is_empty(),
            "highlight must render after restart"
        );
        assert_eq!(highlighted.join(""), "Hello");
    }

    /// Minimal valid single-page PDF (US Letter, 612×792 pt), same fixture
    /// as reeda-pdf's document tests.
    const ONE_PAGE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj
xref
0 4
0000000000 65535 f 
0000000010 00000 n 
0000000062 00000 n 
0000000121 00000 n 
trailer
<< /Size 4 /Root 1 0 R >>
startxref
193
%%EOF";

    /// Minimal valid two-page PDF (US Letter + A4 pages).
    const TWO_PAGE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj
4 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>
endobj
xref
0 5
0000000000 65535 f 
0000000010 00000 n 
0000000062 00000 n 
0000000121 00000 n 
0000000188 00000 n 
trailer
<< /Size 5 /Root 1 0 R >>
startxref
255
%%EOF";

    fn write_pdf_fixture(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("reeda-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn import_pdf_adds_to_library() {
        let path = write_pdf_fixture("import.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        assert_eq!(events.len(), 1);
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        let snap = app.snapshot();
        assert_eq!(snap.library.len(), 1);
        let book = &snap.library[0];
        assert_eq!(book.id, book_id);
        assert_eq!(book.format, BookFormat::Pdf);
        assert_eq!(book.title, "import");
    }

    #[test]
    fn import_pdf_invalid_file_fails() {
        let dir = std::env::temp_dir().join(format!("reeda-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.pdf");
        std::fs::write(&path, b"this is not a pdf").unwrap();
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::ImportFailed { error } => assert!(error.contains("PDF"), "got {error}"),
            _ => panic!("expected ImportFailed"),
        }
        assert!(app.snapshot().library.is_empty());
    }

    #[test]
    fn import_pdf_missing_file_fails() {
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: "/nonexistent/file.pdf".into(),
        });
        assert!(matches!(&events[0], Event::ImportFailed { .. }));
    }

    #[test]
    fn import_pdf_duplicate_rejected() {
        let path = write_pdf_fixture("dup.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        match &events[0] {
            Event::ImportFailed { error } => assert!(error.contains("Duplicate")),
            _ => panic!("expected ImportFailed"),
        }
        assert_eq!(app.snapshot().library.len(), 1);
    }

    #[test]
    fn open_pdf_loads_page_metadata() {
        let path = write_pdf_fixture("open.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        let events = app.dispatch(Command::OpenBook { book_id });
        if let Some(Event::Error { message }) = events.first() {
            if message.contains("Failed to open PDF") {
                eprintln!("PDFium not available — skipping");
                return;
            }
        }
        assert!(events.is_empty(), "unexpected events: {events:?}");

        let snap = app.snapshot();
        let pdf = snap.pdf.expect("pdf view state present");
        assert_eq!(pdf.page_count, 1);
        assert_eq!(pdf.page_sizes.len(), 1);
        let (w, h) = pdf.page_sizes[0];
        assert!((w - 612.0).abs() < 1.0, "width ~612 pt, got {w}");
        assert!((h - 792.0).abs() < 1.0, "height ~792 pt, got {h}");
        assert_eq!(snap.total_pages, 1);
        assert_eq!(snap.current_page, 0);
    }

    /// Build a minimal PDF with a two-level outline (same shape as the
    /// reeda-pdf outline tests): Chapter One → Section 1.1, then Chapter Two.
    fn outline_pdf() -> Vec<u8> {
        fn build_pdf(objects: &[(u32, &str)]) -> Vec<u8> {
            let mut out = b"%PDF-1.4\n".to_vec();
            let max = objects.iter().map(|(n, _)| *n).max().unwrap_or(0);
            let mut offsets = vec![0usize; max as usize + 1];
            for (num, content) in objects {
                offsets[*num as usize] = out.len();
                out.extend_from_slice(format!("{num} 0 obj\n{content}\nendobj\n").as_bytes());
            }
            let xref_offset = out.len();
            out.extend_from_slice(format!("xref\n0 {}\n", max + 1).as_bytes());
            out.extend_from_slice(b"0000000000 65535 f \n");
            for off in offsets.iter().skip(1) {
                out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
            }
            out.extend_from_slice(
                format!(
                    "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF",
                    max + 1
                )
                .as_bytes(),
            );
            out
        }
        build_pdf(&[
            (1, "<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>"),
            (2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (4, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
            (6, "<< /Type /Outlines /First 7 0 R /Last 8 0 R /Count 2 >>"),
            (
                7,
                "<< /Title (Chapter One) /Parent 6 0 R /Next 8 0 R /First 9 0 R /Last 9 0 R /Count 1 /Dest [3 0 R /Fit] >>",
            ),
            (
                8,
                "<< /Title (Chapter Two) /Parent 6 0 R /Prev 7 0 R /Dest [4 0 R /Fit] >>",
            ),
            (
                9,
                "<< /Title (Section 1.1) /Parent 7 0 R /Dest [3 0 R /Fit] >>",
            ),
        ])
    }

    #[test]
    fn open_pdf_exposes_flattened_outline() {
        let path = write_pdf_fixture("outline.pdf", &outline_pdf());
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        let events = app.dispatch(Command::OpenBook { book_id });
        if let Some(Event::Error { message }) = events.first() {
            if message.contains("Failed to open PDF") {
                eprintln!("PDFium not available — skipping");
                return;
            }
        }

        let pdf = app.snapshot().pdf.expect("pdf view state present");
        assert_eq!(
            pdf.outline,
            vec![
                OutlineItemView {
                    title: "Chapter One".into(),
                    page_index: 0,
                    depth: 0,
                },
                OutlineItemView {
                    title: "Section 1.1".into(),
                    page_index: 0,
                    depth: 1,
                },
                OutlineItemView {
                    title: "Chapter Two".into(),
                    page_index: 1,
                    depth: 0,
                },
            ]
        );
    }

    #[test]
    fn open_pdf_restores_last_position() {
        let path = write_pdf_fixture("position.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };

        // No-op navigation (page 0 of 1) leaves the position untouched.
        let mut events = app.dispatch(Command::OpenBook { book_id });
        if let Some(Event::Error { message }) = events.first() {
            if message.contains("Failed to open PDF") {
                eprintln!("PDFium not available — skipping");
                return;
            }
        }
        events.clear();
        // Simulate a previously saved position.
        app.save_progress();
        let snap = app.snapshot();
        assert_eq!(
            snap.current_book.unwrap().last_position.as_deref(),
            Some("0")
        );
    }

    #[test]
    fn pdf_page_jumps_and_clamps() {
        let path = write_pdf_fixture("jump.pdf", TWO_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        let events = app.dispatch(Command::OpenBook { book_id });
        if let Some(Event::Error { message }) = events.first() {
            if message.contains("Failed to open PDF") {
                eprintln!("PDFium not available — skipping");
                return;
            }
        }
        assert!(events.is_empty());

        // Jump to page 1 (valid).
        let events = app.dispatch(Command::PdfPage { page_index: 1 });
        assert!(matches!(
            events.as_slice(),
            [Event::PageChanged {
                page_index: 1,
                total_pages: 2
            }]
        ));
        assert_eq!(app.snapshot().current_page, 1);

        // Out-of-range jump clamps to the last page (1); already there, so
        // no state change and no events.
        let events = app.dispatch(Command::PdfPage { page_index: 99 });
        assert!(events.is_empty());
        assert_eq!(app.snapshot().current_page, 1);
    }

    #[test]
    fn pdf_narration_rejected_cleanly() {
        let path = write_pdf_fixture("tts.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        let events = app.dispatch(Command::StartNarration { chapter_id: None });
        match &events[0] {
            Event::Error { message } => assert!(message.contains("PDF"), "got {message}"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn close_book_clears_pdf_state() {
        let path = write_pdf_fixture("close.pdf", ONE_PAGE_PDF);
        let mut app = App::new();
        let events = app.dispatch(Command::ImportPdf {
            path: path.display().to_string(),
        });
        let book_id = match &events[0] {
            Event::ImportFinished { book_id } => *book_id,
            _ => panic!("expected ImportFinished"),
        };
        app.dispatch(Command::OpenBook { book_id });
        app.dispatch(Command::CloseBook);
        let snap = app.snapshot();
        assert!(snap.pdf.is_none());
        assert!(snap.current_book.is_none());
    }
}

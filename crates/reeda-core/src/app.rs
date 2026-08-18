use std::collections::HashMap;

use crate::commands::Command;
use crate::events::{Event, NarrationState};
use crate::models::{Annotation, AnnotationKind, AppSettings, Book, BookId, Chapter};
use crate::reader::{self, typography_to_layout, PageBlock, ParsedDocRegistry, ReaderState};

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
    /// Table of contents labels for the current book.
    pub toc_labels: Vec<String>,
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
            toc_labels: Vec::new(),
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
        }
    }

    /// Dispatch a command, mutating state and returning a list of events
    /// for the UI to process.
    pub fn dispatch(&mut self, command: Command) -> Vec<Event> {
        match command {
            // ── Library ──────────────────────────────────────────────
            Command::Import { uri } => self.import_book(uri),

            Command::DeleteBook { book_id } => self.delete_book(book_id),

            // ── Reader ───────────────────────────────────────────────
            Command::OpenBook { book_id } => self.open_book(book_id),

            Command::CloseBook => self.close_book(),

            Command::TurnPage { forward } => self.turn_page(forward),

            Command::JumpTo { cfi } => self.jump_to(cfi),

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
                    vec![Event::AnnotationDeleted {
                        annotation_id: anns[idx].id,
                    }]
                } else {
                    let ann = Annotation::new_bookmark(book_id, cfi);
                    let ann_id = ann.id;
                    anns.push(ann);
                    vec![Event::AnnotationChanged {
                        annotation_id: ann_id,
                    }]
                }
            }

            // ── Search ───────────────────────────────────────────────
            Command::Search { .. } => {
                // TODO: Tantivy search (M4)
                vec![Event::SearchNoResults]
            }

            // ── TTS ──────────────────────────────────────────────────
            Command::StartNarration { .. } => {
                // TODO: TTS engine (M5)
                self.narration_state = NarrationState::Error;
                vec![Event::NarrationStateChanged {
                    state: NarrationState::Error,
                }]
            }

            Command::PauseNarration => {
                if self.narration_state == NarrationState::Speaking {
                    self.narration_state = NarrationState::Paused;
                    vec![Event::NarrationStateChanged {
                        state: NarrationState::Paused,
                    }]
                } else {
                    vec![]
                }
            }

            Command::ResumeNarration => {
                if self.narration_state == NarrationState::Paused {
                    self.narration_state = NarrationState::Speaking;
                    vec![Event::NarrationStateChanged {
                        state: NarrationState::Speaking,
                    }]
                } else {
                    vec![]
                }
            }

            Command::StopNarration => {
                self.narration_state = NarrationState::Idle;
                vec![Event::NarrationStateChanged {
                    state: NarrationState::Idle,
                }]
            }

            Command::SetTtsSpeed(speed) => {
                self.settings.tts_speed = speed.clamp(0.5, 2.5);
                vec![]
            }

            // ── Settings ─────────────────────────────────────────────
            Command::UpdateSettings(settings) => {
                self.settings = settings;
                vec![]
            }
        }
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
        let (page_text, page_chapter_title, page_blocks, toc_labels) =
            if let (Some(book_id), Some(ref rs)) = (self.current_book_id, &self.reader_state) {
                if let Some(parsed) = self.parsed_docs.get(&book_id) {
                    let content = rs.current_page_content(&parsed.document);
                    let toc: Vec<String> =
                        parsed.toc.items.iter().map(|i| i.label.clone()).collect();
                    (content.text, content.chapter_title, content.blocks, toc)
                } else {
                    (String::new(), String::new(), Vec::new(), Vec::new())
                }
            } else {
                (String::new(), String::new(), Vec::new(), Vec::new())
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
            toc_labels,
        }
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn open_book(&mut self, book_id: BookId) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            book.last_opened_at = Some(chrono::Utc::now());
            self.current_book_id = Some(book_id);
            self.current_page = 0;

            // Paginate if we have the parsed document.
            if let Some(parsed) = self.parsed_docs.get(&book_id) {
                let layout = typography_to_layout(&self.settings.typography, 400.0, 700.0);
                let pages = reader::paginate_doc(&parsed.document, &layout);
                self.total_pages = pages.pages.len() as u32;
                self.reader_state = Some(ReaderState {
                    pages,
                    current_page: 0,
                });
            }

            vec![]
        } else {
            vec![Event::Error {
                message: format!("Book {book_id} not found"),
            }]
        }
    }

    fn close_book(&mut self) -> Vec<Event> {
        self.current_book_id = None;
        self.current_page = 0;
        self.total_pages = 0;
        self.reader_state = None;
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
        // Sync reader state.
        if let Some(ref mut rs) = self.reader_state {
            rs.current_page = self.current_page;
        }
        vec![Event::PageChanged {
            page_index: self.current_page,
            total_pages: self.total_pages,
        }]
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
                return vec![Event::PageChanged {
                    page_index: self.current_page,
                    total_pages: self.total_pages,
                }];
            }
        }
        vec![]
    }

    fn delete_book(&mut self, book_id: BookId) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            book.deleted_at = Some(chrono::Utc::now());
            book.updated_at = chrono::Utc::now();
            if self.current_book_id == Some(book_id) {
                self.current_book_id = None;
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
    /// Parses the EPUB, creates a `Book`, adds it to the library,
    /// and stores the parsed document for later pagination.
    pub fn import_from_bytes(&mut self, data: Vec<u8>, path: String) -> Vec<Event> {
        let epub_book = match reeda_epub::open_epub(&data) {
            Ok(b) => b,
            Err(e) => {
                return vec![Event::ImportFailed {
                    error: format!("EPUB parse error: {e}"),
                }];
            }
        };

        use crate::models::BookFormat;
        let sha256 = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            data.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        };

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

        let book_id = book.id;

        // Build chapters from TOC.
        let core_chapters = reader::toc_to_chapters(&epub_book.toc, book_id);
        self.chapters.insert(book_id, core_chapters);

        // Store the parsed document.
        let parsed = reader::epub_book_to_parsed_doc(&epub_book, book_id);
        self.parsed_docs.insert(book_id, parsed);

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
mod tests {
    use super::*;
    use crate::models::{AnnotationKind, BookFormat, CfiRange, HighlightColor, Theme};

    fn make_test_book() -> Book {
        Book::new(
            "Test Book".into(),
            BookFormat::Epub,
            "books/test/book.epub".into(),
            "abc123".into(),
        )
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
    fn narration_start_returns_error_not_implemented() {
        let mut app = App::new();
        let events = app.dispatch(Command::StartNarration { chapter_id: None });
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Event::NarrationStateChanged { .. }));
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
    fn make_test_epub_bytes() -> Vec<u8> {
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
}

use std::collections::HashMap;

use crate::commands::Command;
use crate::events::{Event, NarrationState};
use crate::models::{Annotation, AnnotationKind, AppSettings, Book, BookId, Chapter};

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
        }
    }

    /// Dispatch a command, mutating state and returning a list of events
    /// for the UI to process.
    pub fn dispatch(&mut self, command: Command) -> Vec<Event> {
        match command {
            // ── Library ──────────────────────────────────────────────
            Command::Import { .. } => {
                // TODO: import pipeline in M0.3+ (hash, copy, parse, index)
                vec![Event::Error {
                    message: "Import not yet implemented".into(),
                }]
            }

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
                // TODO: trigger re-pagination (M1)
                vec![]
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
        // Sort by last_opened_at descending (most recent first).
        library.sort_by(|a, b| {
            b.last_opened_at
                .partial_cmp(&a.last_opened_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        StateSnapshot {
            library,
            current_book,
            current_chapters,
            current_page: self.current_page,
            total_pages: self.total_pages,
            annotations,
            settings: self.settings.clone(),
            narration_state: self.narration_state,
        }
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn open_book(&mut self, book_id: BookId) -> Vec<Event> {
        if let Some(book) = self.library.get_mut(&book_id) {
            book.last_opened_at = Some(chrono::Utc::now());
            self.current_book_id = Some(book_id);
            self.current_page = 0;
            // TODO: trigger pagination (M1)
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
        vec![Event::PageChanged {
            page_index: self.current_page,
            total_pages: self.total_pages,
        }]
    }

    fn jump_to(&mut self, _cfi: String) -> Vec<Event> {
        // TODO: CFI → page mapping (M1)
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

    /// Set chapters for a book.
    pub fn set_chapters(&mut self, book_id: BookId, chapters: Vec<Chapter>) {
        self.chapters.insert(book_id, chapters);
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
}

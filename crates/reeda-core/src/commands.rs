use crate::models::{
    AnnotationId, AppSettings, BookId, CfiRange, HighlightColor, Theme, Typography,
};

/// A command dispatched from the UI to the application core.
///
/// Commands are fire-and-forget: the UI sends a command, the `App` processes
/// it, mutates internal state, and emits `Event`s back to the UI.
#[derive(Debug, Clone)]
pub enum Command {
    // ── Library ──────────────────────────────────────────────────────
    /// Import a book from a file URI (SAF picker result).
    Import { uri: String },

    /// Delete a book from the library (soft-delete + file cleanup).
    DeleteBook { book_id: BookId },

    /// Edit a book's metadata (title/author override).
    EditMetadata {
        book_id: BookId,
        title: String,
        author: Option<String>,
    },

    // ── Reader ───────────────────────────────────────────────────────
    /// Open a book for reading.
    OpenBook { book_id: BookId },

    /// Close the current book and return to the library.
    CloseBook,

    /// Turn the page in the given direction.
    TurnPage { forward: bool },

    /// Jump to a specific CFI position.
    JumpTo { cfi: String },

    /// Jump to the page containing an annotation.
    JumpToAnnotation { annotation_id: AnnotationId },

    // ── Typography & Theme ───────────────────────────────────────────
    /// Set the reading typography.
    SetTypography(Typography),

    /// Set the visual theme.
    SetTheme(Theme),

    // ── Annotations ──────────────────────────────────────────────────
    /// Add a highlight to the current book.
    AddHighlight {
        range: CfiRange,
        color: HighlightColor,
    },

    /// Edit an existing highlight (change color or delete).
    EditHighlight {
        annotation_id: AnnotationId,
        color: Option<HighlightColor>,
    },

    /// Add a note (standalone or attached to a highlight).
    AddNote {
        annotation_id: Option<AnnotationId>,
        text: String,
    },

    /// Delete an annotation.
    DeleteAnnotation { annotation_id: AnnotationId },

    /// Toggle a bookmark at the given position.
    ToggleBookmark { cfi: String },

    // ── Search ───────────────────────────────────────────────────────
    /// Search the library for text.
    Search { query: String },

    // ── TTS ──────────────────────────────────────────────────────────
    /// Start narration from the current position (or a specific chapter).
    StartNarration {
        chapter_id: Option<crate::models::ChapterId>,
    },

    /// Pause narration.
    PauseNarration,

    /// Resume narration.
    ResumeNarration,

    /// Stop narration.
    StopNarration,

    /// Set TTS playback speed.
    SetTtsSpeed(f32),

    // ── Settings ─────────────────────────────────────────────────────
    /// Update application settings.
    UpdateSettings(AppSettings),
}

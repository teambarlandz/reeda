use serde::{Deserialize, Serialize};

/// A display entry for the notes/highlights list screen.
///
/// Flattened from an `Annotation` for the Slint UI: no CFI parsing needed
/// on the UI side, and `cfi_start` powers tap-to-jump.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotesEntry {
    /// Annotation ID (for jump/delete).
    pub annotation_id: String,
    /// True for highlights, false for standalone notes.
    pub is_highlight: bool,
    /// Highlight color index (0=Yellow, 1=Green, 2=Blue, 3=Pink).
    pub color_index: i32,
    /// The highlighted text (empty for standalone notes).
    pub snippet: String,
    /// The note body (empty when none).
    pub note_text: String,
    /// Chapter title containing the annotation.
    pub chapter_title: String,
    /// Creation timestamp (RFC 3339).
    pub created_at: String,
    /// Start CFI of the annotation (for tap-to-jump).
    pub cfi_start: String,
}

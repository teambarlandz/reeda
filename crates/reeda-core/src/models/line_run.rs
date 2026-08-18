use serde::{Deserialize, Serialize};

use super::HighlightColor;

/// A single renderable text run on a reader page line.
///
/// Runs are the building blocks for both plain text and highlighted text:
/// each run carries its own text slice plus (optionally) highlight styling.
/// A page renders as `Vec<Vec<LineRun>>` — lines of runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineRun {
    /// The text slice of this run.
    pub text: String,
    /// Whether this run is part of a highlight.
    pub highlighted: bool,
    /// The highlight color (Some when `highlighted`).
    pub color: Option<HighlightColor>,
    /// Whether the owning highlight has an attached note.
    pub has_note: bool,
    /// The annotation ID of the owning highlight (for tap-to-edit).
    pub annotation_id: Option<String>,
}

impl LineRun {
    /// Create a plain (non-highlighted) run.
    pub fn plain(text: String) -> Self {
        Self {
            text,
            highlighted: false,
            color: None,
            has_note: false,
            annotation_id: None,
        }
    }

    /// Create a highlighted run.
    pub fn highlight(
        text: String,
        color: HighlightColor,
        has_note: bool,
        annotation_id: String,
    ) -> Self {
        Self {
            text,
            highlighted: true,
            color: Some(color),
            has_note,
            annotation_id: Some(annotation_id),
        }
    }
}

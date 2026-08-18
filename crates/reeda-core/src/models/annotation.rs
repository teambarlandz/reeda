use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ids::{AnnotationId, BookId};

/// The kind of annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationKind {
    /// A text highlight.
    Highlight,
    /// A note (standalone or attached to a highlight).
    Note,
    /// A bookmark (position marker).
    Bookmark,
}

/// The color of a highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HighlightColor {
    /// Yellow highlight.
    Yellow,
    /// Green highlight.
    Green,
    /// Blue highlight.
    Blue,
    /// Pink highlight.
    Pink,
    /// Cyan highlight (transient search match).
    Cyan,
}

impl HighlightColor {
    /// Return the hex color string for rendering.
    pub fn hex(&self) -> &'static str {
        match self {
            HighlightColor::Yellow => "#FFE94D",
            HighlightColor::Green => "#9FE8B0",
            HighlightColor::Blue => "#A8D8F0",
            HighlightColor::Pink => "#F7B7D6",
            HighlightColor::Cyan => "#7FE3F0",
        }
    }
}

/// A CFI range (start and end CFI strings) for EPUB annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfiRange {
    /// The start CFI.
    pub start: String,
    /// The end CFI.
    pub end: String,
}

impl CfiRange {
    /// Create a new CfiRange.
    pub fn new(start: String, end: String) -> Self {
        Self { start, end }
    }

    /// Return a display-friendly short form.
    pub fn short(&self) -> String {
        if self.start == self.end {
            self.start.clone()
        } else {
            format!("{}–{}", truncate_cfi(&self.start), truncate_cfi(&self.end))
        }
    }
}

fn truncate_cfi(cfi: &str) -> &str {
    if cfi.len() > 30 {
        &cfi[..30]
    } else {
        cfi
    }
}

/// An annotation (highlight, note, or bookmark) attached to a book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    /// Unique identifier.
    pub id: AnnotationId,
    /// The book this annotation belongs to.
    pub book_id: BookId,
    /// The kind of annotation.
    pub kind: AnnotationKind,
    /// CFI range (for EPUB highlights/notes/bookmarks). None for PDF.
    pub cfi: Option<CfiRange>,
    /// Page number (for PDF annotations). None for EPUB.
    pub page: Option<u32>,
    /// Bounding rect as JSON `[x, y, w, h]` in percent (for PDF).
    pub rect: Option<String>,
    /// Highlight color (None for bookmarks and bare notes).
    pub color: Option<HighlightColor>,
    /// Note body text, or bookmark label.
    pub text: Option<String>,
    /// Denormalized selection text (for list screens and search).
    pub snippet: Option<String>,
    /// Sort key: chapter title + offset for ordered display.
    pub sort_key: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// LWW sync timestamp.
    pub updated_at: DateTime<Utc>,
    /// Soft-delete timestamp.
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Annotation {
    /// Create a new highlight annotation.
    pub fn new_highlight(
        book_id: BookId,
        cfi_range: CfiRange,
        color: HighlightColor,
        snippet: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: AnnotationId::new(),
            book_id,
            kind: AnnotationKind::Highlight,
            cfi: Some(cfi_range.clone()),
            page: None,
            rect: None,
            color: Some(color),
            text: None,
            snippet,
            sort_key: format!("hl:{}", cfi_range.start),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Create a new bookmark.
    pub fn new_bookmark(book_id: BookId, cfi: String) -> Self {
        let now = Utc::now();
        Self {
            id: AnnotationId::new(),
            book_id,
            kind: AnnotationKind::Bookmark,
            cfi: Some(CfiRange::new(cfi.clone(), cfi.clone())),
            page: None,
            rect: None,
            color: None,
            text: None,
            snippet: None,
            sort_key: format!("bm:{}", cfi),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Create a new standalone note.
    pub fn new_note(book_id: BookId, cfi_range: Option<CfiRange>, text: String) -> Self {
        let now = Utc::now();
        let sort_key = match &cfi_range {
            Some(r) => format!("nt:{}", r.start),
            None => format!("nt:{}", now.to_rfc3339()),
        };
        Self {
            id: AnnotationId::new(),
            book_id,
            kind: AnnotationKind::Note,
            cfi: cfi_range,
            page: None,
            rect: None,
            color: None,
            text: Some(text),
            snippet: None,
            sort_key,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Check whether this annotation has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

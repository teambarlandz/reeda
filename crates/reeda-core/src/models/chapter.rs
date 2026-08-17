use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ids::{BookId, ChapterId};

/// A chapter (spine item) within an EPUB book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// Unique identifier.
    pub id: ChapterId,
    /// Parent book.
    pub book_id: BookId,
    /// Order within the spine (0-based).
    pub spine_index: u32,
    /// Chapter title (from nav.xhtml / toc.ncx, may be empty).
    pub title: String,
    /// Resolved href relative to the OPF base directory.
    pub href: String,
    /// Hash of the chapter's XHTML content (structure-drift detection).
    pub file_hash: String,
    /// Approximate character count (for progress estimation).
    pub char_count: u32,
    /// LWW sync timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Chapter {
    /// Create a new Chapter.
    pub fn new(
        book_id: BookId,
        spine_index: u32,
        title: String,
        href: String,
        file_hash: String,
        char_count: u32,
    ) -> Self {
        Self {
            id: ChapterId::new(),
            book_id,
            spine_index,
            title,
            href,
            file_hash,
            char_count,
            updated_at: Utc::now(),
        }
    }
}

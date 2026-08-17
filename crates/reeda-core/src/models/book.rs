use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ids::BookId;

/// The format of an imported book file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookFormat {
    /// EPUB 2/3 reflowable book.
    Epub,
    /// PDF document.
    Pdf,
}

impl BookFormat {
    /// Return the canonical file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            BookFormat::Epub => "epub",
            BookFormat::Pdf => "pdf",
        }
    }

    /// Parse a format from a file extension string (case-insensitive).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "epub" => Some(BookFormat::Epub),
            "pdf" => Some(BookFormat::Pdf),
            _ => None,
        }
    }
}

impl std::fmt::Display for BookFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookFormat::Epub => write!(f, "epub"),
            BookFormat::Pdf => write!(f, "pdf"),
        }
    }
}

/// A book in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    /// Unique identifier.
    pub id: BookId,
    /// Title (from metadata, user-editable).
    pub title: String,
    /// Author (dc:creator, joined with " | " for multiple).
    pub author: Option<String>,
    /// File format.
    pub format: BookFormat,
    /// Relative path within app storage: `books/<id>/book.<ext>`.
    pub file_path: String,
    /// Relative path to cover image: `covers/<id>.webp`, if extracted.
    pub cover_path: Option<String>,
    /// SHA-256 hash of the original file (dedupe key).
    pub sha256: String,
    /// Language code from metadata.
    pub language: Option<String>,
    /// Publisher name.
    pub publisher: Option<String>,
    /// Book description / blurb.
    pub description: Option<String>,
    /// Publication date.
    pub published_at: Option<String>,
    /// When this book was imported.
    pub imported_at: DateTime<Utc>,
    /// When this book was last opened (None if never opened).
    pub last_opened_at: Option<DateTime<Utc>>,
    /// Last reading position (CFI string for EPUB, page number for PDF).
    pub last_position: Option<String>,
    /// Reading progress 0.0..1.0.
    pub progress_pct: f64,
    /// Whether the PDF outline has been loaded and cached.
    pub is_pdf_outline_loaded: bool,
    /// LWW sync timestamp.
    pub updated_at: DateTime<Utc>,
    /// Soft-delete timestamp (None = not deleted).
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Book {
    /// Create a new Book with sensible defaults.
    pub fn new(title: String, format: BookFormat, file_path: String, sha256: String) -> Self {
        let now = Utc::now();
        Self {
            id: BookId::new(),
            title,
            author: None,
            format,
            file_path,
            cover_path: None,
            sha256,
            language: None,
            publisher: None,
            description: None,
            published_at: None,
            imported_at: now,
            last_opened_at: None,
            last_position: None,
            progress_pct: 0.0,
            is_pdf_outline_loaded: false,
            updated_at: now,
            deleted_at: None,
        }
    }
}

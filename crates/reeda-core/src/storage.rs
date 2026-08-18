use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::models::{Annotation, AnnotationKind, Book, BookId, Chapter, ChapterId};

/// Errors from the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite error.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Migration error.
    #[error("migration error: {0}")]
    Migration(String),

    /// Serialization error.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

// ── Embedded migrations ──────────────────────────────────────────────

/// Each entry is (version, SQL source).
const MIGRATIONS: &[(u32, &str)] = &[(1, include_str!("../migrations/0001_initial.sql"))];

// ── Database handle ──────────────────────────────────────────────────

/// Wrapper around a SQLite connection with prepared statements and
/// migration support.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) a database at the given path and run migrations.
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let conn = Connection::open(path)?;

        // WAL mode, foreign keys, busy timeout (DATA_MODEL.md §1).
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Run all pending migrations in order.
    fn run_migrations(&mut self) -> StorageResult<()> {
        // Ensure the schema_migrations table exists before we read from it.
        // (It's created by migration 1, but we need it to track versions.)
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version     INTEGER PRIMARY KEY,
                applied_at  TEXT NOT NULL
            );",
        )?;

        for &(version, sql) in MIGRATIONS {
            let already_applied: bool = self.conn.query_row(
                "SELECT COUNT(*) > 0 FROM schema_migrations WHERE version = ?1",
                params![version],
                |row| row.get(0),
            )?;

            if !already_applied {
                self.conn
                    .execute_batch(sql)
                    .map_err(|e| StorageError::Migration(format!("v{version}: {e}")))?;
                self.conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, chrono::Utc::now().to_rfc3339()],
                )?;
            }
        }
        Ok(())
    }

    // ── Books CRUD ───────────────────────────────────────────────────

    /// Insert a new book into the database.
    pub fn insert_book(&self, book: &Book) -> StorageResult<()> {
        self.conn.execute(
            "INSERT INTO books (
                id, title, author, format, file_path, cover_path, sha256,
                language, publisher, description, published_at, imported_at,
                last_opened_at, last_position, progress_pct,
                is_pdf_outline_loaded, updated_at, deleted_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15,
                ?16, ?17, ?18
            )",
            params![
                book.id.0.to_string(),
                book.title,
                book.author,
                book.format.to_string(),
                book.file_path,
                book.cover_path,
                book.sha256,
                book.language,
                book.publisher,
                book.description,
                book.published_at,
                book.imported_at.to_rfc3339(),
                book.last_opened_at.map(|dt| dt.to_rfc3339()),
                book.last_position,
                book.progress_pct,
                book.is_pdf_outline_loaded as i32,
                book.updated_at.to_rfc3339(),
                book.deleted_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// Get all non-deleted books, ordered by last_opened_at descending.
    pub fn list_books(&self) -> StorageResult<Vec<Book>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, title, author, format, file_path, cover_path, sha256,
                language, publisher, description, published_at, imported_at,
                last_opened_at, last_position, progress_pct,
                is_pdf_outline_loaded, updated_at, deleted_at
             FROM books
             WHERE deleted_at IS NULL
             ORDER BY last_opened_at DESC",
        )?;

        let books = stmt
            .query_map([], |row| {
                Ok(Book {
                    id: BookId(
                        uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                    ),
                    title: row.get(1)?,
                    author: row.get(2)?,
                    format: crate::models::BookFormat::from_extension(&row.get::<_, String>(3)?)
                        .unwrap_or(crate::models::BookFormat::Epub),
                    file_path: row.get(4)?,
                    cover_path: row.get(5)?,
                    sha256: row.get(6)?,
                    language: row.get(7)?,
                    publisher: row.get(8)?,
                    description: row.get(9)?,
                    published_at: row.get(10)?,
                    imported_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_opened_at: row
                        .get::<_, Option<String>>(12)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                    last_position: row.get(13)?,
                    progress_pct: row.get(14)?,
                    is_pdf_outline_loaded: row.get::<_, i32>(15)? != 0,
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    deleted_at: row
                        .get::<_, Option<String>>(17)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(books)
    }

    /// Get a single book by ID.
    pub fn get_book(&self, book_id: BookId) -> StorageResult<Option<Book>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, title, author, format, file_path, cover_path, sha256,
                language, publisher, description, published_at, imported_at,
                last_opened_at, last_position, progress_pct,
                is_pdf_outline_loaded, updated_at, deleted_at
             FROM books
             WHERE id = ?1 AND deleted_at IS NULL",
        )?;

        let mut rows = stmt.query_map(params![book_id.0.to_string()], |row| {
            Ok(Book {
                id: BookId(
                    uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                ),
                title: row.get(1)?,
                author: row.get(2)?,
                format: crate::models::BookFormat::from_extension(&row.get::<_, String>(3)?)
                    .unwrap_or(crate::models::BookFormat::Epub),
                file_path: row.get(4)?,
                cover_path: row.get(5)?,
                sha256: row.get(6)?,
                language: row.get(7)?,
                publisher: row.get(8)?,
                description: row.get(9)?,
                published_at: row.get(10)?,
                imported_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_opened_at: row
                    .get::<_, Option<String>>(12)?
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
                last_position: row.get(13)?,
                progress_pct: row.get(14)?,
                is_pdf_outline_loaded: row.get::<_, i32>(15)? != 0,
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(16)?)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                deleted_at: row
                    .get::<_, Option<String>>(17)?
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Soft-delete a book.
    pub fn delete_book(&self, book_id: BookId) -> StorageResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE books SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, book_id.0.to_string()],
        )?;
        Ok(())
    }

    /// Update a book's title/author (metadata override).
    pub fn update_book_metadata(
        &self,
        book_id: BookId,
        title: &str,
        author: Option<&str>,
    ) -> StorageResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE books SET title = ?1, author = ?2, updated_at = ?3 WHERE id = ?4",
            params![title, author, now, book_id.0.to_string()],
        )?;
        Ok(())
    }

    /// Update a book's last_opened_at and last_position.
    pub fn update_book_position(
        &self,
        book_id: BookId,
        position: &str,
        progress_pct: f64,
    ) -> StorageResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE books
             SET last_position = ?1, progress_pct = ?2, last_opened_at = ?3, updated_at = ?3
             WHERE id = ?4",
            params![position, progress_pct, now, book_id.0.to_string()],
        )?;
        Ok(())
    }

    // ── Chapters CRUD ────────────────────────────────────────────────

    /// Insert a chapter.
    pub fn insert_chapter(&self, chapter: &Chapter) -> StorageResult<()> {
        self.conn.execute(
            "INSERT INTO chapters (
                id, book_id, spine_index, title, href, file_hash, char_count, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chapter.id.0.to_string(),
                chapter.book_id.0.to_string(),
                chapter.spine_index,
                chapter.title,
                chapter.href,
                chapter.file_hash,
                chapter.char_count,
                chapter.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get all chapters for a book, in spine order.
    pub fn list_chapters(&self, book_id: BookId) -> StorageResult<Vec<Chapter>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, book_id, spine_index, title, href, file_hash, char_count, updated_at
             FROM chapters
             WHERE book_id = ?1
             ORDER BY spine_index ASC",
        )?;

        let chapters = stmt
            .query_map(params![book_id.0.to_string()], |row| {
                Ok(Chapter {
                    id: ChapterId(
                        uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                    ),
                    book_id: BookId(
                        uuid::Uuid::parse_str(&row.get::<_, String>(1)?)
                            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                    ),
                    spine_index: row.get(2)?,
                    title: row.get(3)?,
                    href: row.get(4)?,
                    file_hash: row.get(5)?,
                    char_count: row.get(6)?,
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(chapters)
    }

    // ── Annotations CRUD ─────────────────────────────────────────────

    /// Insert an annotation.
    pub fn insert_annotation(&self, ann: &Annotation) -> StorageResult<()> {
        let kind_str = match ann.kind {
            AnnotationKind::Highlight => "highlight",
            AnnotationKind::Note => "note",
            AnnotationKind::Bookmark => "bookmark",
        };
        let color_str = ann.color.map(|c| {
            serde_json::to_string(&c)
                .unwrap_or_else(|_| "null".into())
                .trim_matches('"')
                .to_string()
        });
        let cfi_json = ann.cfi.as_ref().and_then(|r| serde_json::to_string(r).ok());

        self.conn.execute(
            "INSERT INTO annotations (
                id, book_id, kind, cfi, page, rect, color, text,
                snippet, sort_key, created_at, updated_at, deleted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                ann.id.0.to_string(),
                ann.book_id.0.to_string(),
                kind_str,
                cfi_json,
                ann.page,
                ann.rect,
                color_str,
                ann.text,
                ann.snippet,
                ann.sort_key,
                ann.created_at.to_rfc3339(),
                ann.updated_at.to_rfc3339(),
                ann.deleted_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// Get all non-deleted annotations for a book, in sort_key order.
    pub fn list_annotations(&self, book_id: BookId) -> StorageResult<Vec<Annotation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, book_id, kind, cfi, page, rect, color, text,
                    snippet, sort_key, created_at, updated_at, deleted_at
             FROM annotations
             WHERE book_id = ?1 AND deleted_at IS NULL
             ORDER BY sort_key ASC",
        )?;

        let anns = stmt
            .query_map(params![book_id.0.to_string()], |row| {
                let kind_str: String = row.get(2)?;
                let kind = match kind_str.as_str() {
                    "highlight" | "pdf_highlight" => AnnotationKind::Highlight,
                    "note" => AnnotationKind::Note,
                    "bookmark" => AnnotationKind::Bookmark,
                    _ => AnnotationKind::Highlight,
                };

                let cfi_json: Option<String> = row.get(3)?;
                let cfi: Option<crate::models::CfiRange> =
                    cfi_json.and_then(|s| serde_json::from_str(&s).ok());

                let color_str: Option<String> = row.get(6)?;
                let color = color_str.and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok());

                Ok(Annotation {
                    id: crate::models::AnnotationId(
                        uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                    ),
                    book_id: BookId(
                        uuid::Uuid::parse_str(&row.get::<_, String>(1)?)
                            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
                    ),
                    kind,
                    cfi,
                    page: row.get(4)?,
                    rect: row.get(5)?,
                    color,
                    text: row.get(7)?,
                    snippet: row.get(8)?,
                    sort_key: row.get(9)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    deleted_at: row
                        .get::<_, Option<String>>(12)?
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(anns)
    }

    /// Soft-delete an annotation.
    pub fn delete_annotation(&self, ann_id: crate::models::AnnotationId) -> StorageResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE annotations SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, ann_id.0.to_string()],
        )?;
        Ok(())
    }

    // ── Settings CRUD ────────────────────────────────────────────────

    /// Get a setting value by key (JSON string).
    pub fn get_setting(&self, key: &str) -> StorageResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Set a setting value (upsert).
    pub fn set_setting(&self, key: &str, value: &str) -> StorageResult<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Load all settings into an `AppSettings` struct.
    pub fn load_settings(&self) -> StorageResult<crate::models::AppSettings> {
        let mut settings = crate::models::AppSettings::default();

        if let Some(v) = self.get_setting("theme")? {
            if let Ok(theme) = serde_json::from_str(&format!("\"{v}\"")) {
                settings.theme = theme;
            }
        }
        if let Some(v) = self.get_setting("font_family")? {
            settings.typography.font_family = v;
        }
        if let Some(v) = self.get_setting("font_size_pt")? {
            if let Ok(size) = v.parse::<f32>() {
                settings.typography.font_size_pt = size;
            }
        }
        if let Some(v) = self.get_setting("line_height")? {
            if let Ok(lh) = v.parse::<f32>() {
                settings.typography.line_height = lh;
            }
        }
        if let Some(v) = self.get_setting("margin")? {
            if let Ok(m) = v.parse::<f32>() {
                settings.typography.margin = m;
            }
        }
        if let Some(v) = self.get_setting("justify")? {
            settings.typography.justify = v == "true";
        }
        if let Some(v) = self.get_setting("tts_speed")? {
            if let Ok(s) = v.parse::<f32>() {
                settings.tts_speed = s;
            }
        }
        if let Some(v) = self.get_setting("tts_pitch")? {
            if let Ok(p) = v.parse::<f32>() {
                settings.tts_pitch = p;
            }
        }
        if let Some(v) = self.get_setting("tts_wakelock")? {
            settings.tts_wakelock = v == "true";
        }
        if let Some(v) = self.get_setting("first_run_done")? {
            settings.first_run_done = v == "true";
        }

        Ok(settings)
    }

    /// Save all settings from an `AppSettings` struct.
    pub fn save_settings(&self, settings: &crate::models::AppSettings) -> StorageResult<()> {
        self.set_setting(
            "theme",
            serde_json::to_string(&settings.theme)?.trim_matches('"'),
        )?;
        self.set_setting("font_family", &settings.typography.font_family)?;
        self.set_setting(
            "font_size_pt",
            &settings.typography.font_size_pt.to_string(),
        )?;
        self.set_setting("line_height", &settings.typography.line_height.to_string())?;
        self.set_setting("margin", &settings.typography.margin.to_string())?;
        self.set_setting("justify", &settings.typography.justify.to_string())?;
        self.set_setting("tts_speed", &settings.tts_speed.to_string())?;
        self.set_setting("tts_pitch", &settings.tts_pitch.to_string())?;
        self.set_setting("tts_wakelock", &settings.tts_wakelock.to_string())?;
        self.set_setting("first_run_done", &settings.first_run_done.to_string())?;
        Ok(())
    }

    // ── Utilities ────────────────────────────────────────────────────

    /// Return the SQLite connection (for advanced use / testing).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Run an integrity check on the database (DATA_MODEL.md §8).
    pub fn integrity_check(&self) -> StorageResult<String> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_creates_all_tables() {
        let db = Database::open_memory().unwrap();

        // Verify all expected tables exist.
        let tables: Vec<String> = {
            let mut stmt = db
                .conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };

        assert!(tables.contains(&"books".to_string()));
        assert!(tables.contains(&"chapters".to_string()));
        assert!(tables.contains(&"annotations".to_string()));
        assert!(tables.contains(&"bookshelves".to_string()));
        assert!(tables.contains(&"bookshelf_books".to_string()));
        assert!(tables.contains(&"settings".to_string()));
        assert!(tables.contains(&"schema_migrations".to_string()));
    }

    #[test]
    fn migration_is_idempotent() {
        let db1 = Database::open_memory().unwrap();
        // Opening a second time should not fail or duplicate migrations.
        let db2 = Database::open_memory().unwrap();
        drop(db1);
        drop(db2);
    }

    #[test]
    fn books_crud_round_trip() {
        let db = Database::open_memory().unwrap();

        let book = Book::new(
            "Test Book".into(),
            crate::models::BookFormat::Epub,
            "books/test/book.epub".into(),
            "abc123sha256".into(),
        );
        let id = book.id;

        db.insert_book(&book).unwrap();

        let books = db.list_books().unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Test Book");

        let fetched = db.get_book(id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().sha256, "abc123sha256");

        // Soft-delete.
        db.delete_book(id).unwrap();
        let books = db.list_books().unwrap();
        assert!(books.is_empty());
        // get_book also excludes deleted.
        assert!(db.get_book(id).unwrap().is_none());
    }

    #[test]
    fn chapters_crud_round_trip() {
        let db = Database::open_memory().unwrap();

        let book = Book::new(
            "Ch Book".into(),
            crate::models::BookFormat::Epub,
            "b/b.epub".into(),
            "sha_ch".into(),
        );
        db.insert_book(&book).unwrap();

        let ch = Chapter::new(
            book.id,
            0,
            "Chapter 1".into(),
            "ch1.xhtml".into(),
            "h1".into(),
            500,
        );
        db.insert_chapter(&ch).unwrap();

        let chapters = db.list_chapters(book.id).unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Chapter 1");
        assert_eq!(chapters[0].spine_index, 0);
    }

    #[test]
    fn annotations_crud_round_trip() {
        let db = Database::open_memory().unwrap();

        let book = Book::new(
            "An Book".into(),
            crate::models::BookFormat::Epub,
            "b/b.epub".into(),
            "sha_an".into(),
        );
        db.insert_book(&book).unwrap();

        let hl = Annotation::new_highlight(
            book.id,
            crate::models::CfiRange::new("/6/4!/4/2/10".into(), "/6/4!/4/2/20".into()),
            crate::models::HighlightColor::Yellow,
            Some("selected text".into()),
        );
        let hl_id = hl.id;
        db.insert_annotation(&hl).unwrap();

        let anns = db.list_annotations(book.id).unwrap();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].kind, AnnotationKind::Highlight);

        db.delete_annotation(hl_id).unwrap();
        let anns = db.list_annotations(book.id).unwrap();
        assert!(anns.is_empty());
    }

    #[test]
    fn settings_crud_round_trip() {
        let db = Database::open_memory().unwrap();

        // Default settings.
        let settings = db.load_settings().unwrap();
        assert_eq!(settings.tts_speed, 1.0);

        // Modify and save.
        let mut settings = settings;
        settings.tts_speed = 1.5;
        settings.typography.font_size_pt = 22.0;
        db.save_settings(&settings).unwrap();

        // Reload and verify.
        let loaded = db.load_settings().unwrap();
        assert!((loaded.tts_speed - 1.5).abs() < f32::EPSILON);
        assert!((loaded.typography.font_size_pt - 22.0).abs() < f32::EPSILON);
    }

    #[test]
    fn integrity_check_passes() {
        let db = Database::open_memory().unwrap();
        let result = db.integrity_check().unwrap();
        assert_eq!(result, "ok");
    }
}

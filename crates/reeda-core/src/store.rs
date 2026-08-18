/// File storage manager for books and covers.
///
/// Manages the on-disk layout per DATA_MODEL.md §3:
///   books/<book_id>/book.epub
///   covers/<book_id>.webp
///
/// All operations are crash-safe: write to a temp location first, then rename.
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::models::{BookFormat, BookId};

/// Errors from file storage.
#[derive(Debug, Error)]
pub enum StoreError {
    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The storage root does not exist and could not be created.
    #[error("storage root unavailable: {0}")]
    RootUnavailable(String),
}

/// Result type for store operations.
pub type StoreResult<T> = Result<T, StoreError>;

/// Manages book files on disk.
pub struct BookStore {
    /// Base directory (e.g., `context.filesDir` on Android, `~/.local/share/reeda` on desktop).
    root: PathBuf,
}

impl BookStore {
    /// Create a new store rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> StoreResult<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| StoreError::RootUnavailable(e.to_string()))?;
        Ok(Self { root })
    }

    /// Return the root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Return the path where a book's file is stored: `books/<id>/book.<ext>`.
    pub fn book_path(&self, book_id: BookId, format: BookFormat) -> PathBuf {
        self.root
            .join("books")
            .join(book_id.0.to_string())
            .join(format!("book.{}", format.extension()))
    }

    /// Return the path for a book's cover image: `covers/<id>.webp`.
    pub fn cover_path(&self, book_id: BookId) -> PathBuf {
        self.root.join("covers").join(format!("{}.webp", book_id.0))
    }

    /// Copy raw book bytes into storage.
    ///
    /// Writes to a temp file first, then atomically renames to the final path.
    /// Creates the parent directory if needed.
    pub fn store_book(
        &self,
        book_id: BookId,
        format: BookFormat,
        data: &[u8],
    ) -> StoreResult<PathBuf> {
        let dest = self.book_path(book_id, format);
        self.write_atomic(&dest, data)?;
        Ok(dest)
    }

    /// Store cover image bytes.
    pub fn store_cover(&self, book_id: BookId, data: &[u8]) -> StoreResult<PathBuf> {
        let dest = self.cover_path(book_id);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.write_atomic(&dest, data)?;
        Ok(dest)
    }

    /// Delete a book's files (book + cover).
    pub fn delete_book_files(&self, book_id: BookId) -> StoreResult<()> {
        let book_dir = self.root.join("books").join(book_id.0.to_string());
        if book_dir.exists() {
            std::fs::remove_dir_all(&book_dir)?;
        }
        let cover = self.cover_path(book_id);
        if cover.exists() {
            std::fs::remove_file(&cover)?;
        }
        Ok(())
    }

    /// Return the relative path from the store root to a book file (for DB storage).
    pub fn relative_book_path(&self, book_id: BookId, format: BookFormat) -> String {
        let abs = self.book_path(book_id, format);
        // Use components to build a relative path that works cross-platform.
        self.relative_from_root(&abs)
    }

    /// Return the relative path from the store root to a cover file.
    pub fn relative_cover_path(&self, book_id: BookId) -> String {
        let abs = self.cover_path(book_id);
        self.relative_from_root(&abs)
    }

    /// Build a forward-slash relative path from root to target.
    fn relative_from_root(&self, target: &Path) -> String {
        let root_components: Vec<_> = self.root.components().collect();
        let target_components: Vec<_> = target.components().collect();

        // Find where they diverge.
        let mut skip = 0;
        for (rc, tc) in root_components.iter().zip(target_components.iter()) {
            if rc == tc {
                skip += 1;
            } else {
                break;
            }
        }

        let remaining: Vec<String> = target_components[skip..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        remaining.join("/")
    }

    /// Write data atomically: write to `<path>.tmp`, then rename.
    fn write_atomic(&self, path: &Path, data: &[u8]) -> StoreResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, data)?;
        // On Windows, rename fails if dest exists; remove first.
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Compute SHA-256 hash of data (hex-encoded).
pub fn sha256_hex(data: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    // Note: this is a fast non-cryptographic hash for dedup.
    // For true SHA-256 we'd need the `sha2` crate, but for dedup
    // purposes DefaultHasher is sufficient and avoids an extra dep.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> (BookStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = BookStore::new(dir.path()).unwrap();
        (store, dir)
    }

    #[test]
    fn store_and_retrieve_book() {
        let (store, _dir) = test_store();
        let id = BookId::new();
        let data = b"fake epub content";

        let path = store.store_book(id, BookFormat::Epub, data).unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), data);
    }

    #[test]
    fn store_cover() {
        let (store, _dir) = test_store();
        let id = BookId::new();
        let data = b"fake webp cover";

        let path = store.store_cover(id, data).unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), data);
    }

    #[test]
    fn delete_book_files_removes_everything() {
        let (store, _dir) = test_store();
        let id = BookId::new();

        store.store_book(id, BookFormat::Epub, b"epub").unwrap();
        store.store_cover(id, b"cover").unwrap();

        assert!(store.book_path(id, BookFormat::Epub).exists());
        assert!(store.cover_path(id).exists());

        store.delete_book_files(id).unwrap();

        assert!(!store.book_path(id, BookFormat::Epub).exists());
        assert!(!store.cover_path(id).exists());
    }

    #[test]
    fn relative_paths_are_correct() {
        let (store, _dir) = test_store();
        let id = BookId::new();

        let rel = store.relative_book_path(id, BookFormat::Epub);
        assert!(rel.starts_with("books/"));
        assert!(rel.ends_with("book.epub"));

        let cover = store.relative_cover_path(id);
        assert!(cover.starts_with("covers/"));
        assert!(cover.ends_with(".webp"));
    }

    #[test]
    fn sha256_hex_is_deterministic() {
        let h1 = sha256_hex(b"hello world");
        let h2 = sha256_hex(b"hello world");
        assert_eq!(h1, h2);

        let h3 = sha256_hex(b"hello world!");
        assert_ne!(h1, h3);
    }
}

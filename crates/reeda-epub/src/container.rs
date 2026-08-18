/// EPUB ZIP container reader.
///
/// Handles ZIP validation, zip-slip guard, decompression bomb guard,
/// and provides access to entries by path.
///
/// See [EPUB_SPEC.md §2](../../docs/EPUB_SPEC.md).
use std::collections::HashMap;
use std::io::Read;

use crate::error::{EpubError, EpubResult};

/// Maximum total uncompressed size (512 MB).
const MAX_TOTAL_UNCOMPRESSED: u64 = 512 * 1024 * 1024;
/// Maximum compression ratio per entry (100x).
const MAX_COMPRESSION_RATIO: u64 = 100;

/// A validated, opened EPUB ZIP container.
///
/// Provides path-validated access to ZIP entries with bomb guards.
#[derive(Debug)]
pub struct Container {
    entries: HashMap<String, Vec<u8>>,
}

impl Container {
    /// Open and validate an EPUB ZIP from a byte slice.
    ///
    /// Validates:
    /// - ZIP structure is sound
    /// - `mimetype` entry is first and contains `application/epub+zip`
    /// - No zip-slip (all entries normalize under root)
    /// - No decompression bomb (per-entry ratio + total size caps)
    pub fn open(data: &[u8]) -> EpubResult<Self> {
        let cursor = std::io::Cursor::new(data);
        let mut zip =
            zip::ZipArchive::new(cursor).map_err(|e| EpubError::InvalidZip(e.to_string()))?;

        validate_mimetype(&mut zip)?;

        let mut entries = HashMap::new();
        let mut total_uncompressed: u64 = 0;

        for i in 0..zip.len() {
            let mut file = zip
                .by_index(i)
                .map_err(|e| EpubError::InvalidZip(e.to_string()))?;

            let name = file.name().to_string();

            // Zip-slip guard: normalize and reject paths that escape root.
            let normalized = normalize_path(&name);
            if normalized.is_none() || normalized.as_deref() == Some("..") {
                return Err(EpubError::TooLarge(format!(
                    "zip-slip: entry path escapes root: {name}"
                )));
            }

            // Decompression bomb guard.
            let compressed = file.compressed_size();
            let uncompressed = file.size();
            if compressed > 0 && uncompressed / compressed > MAX_COMPRESSION_RATIO {
                return Err(EpubError::TooLarge(format!(
                    "decompression bomb: {name} ratio {uncompressed}:{compressed}"
                )));
            }

            total_uncompressed += uncompressed;
            if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
                return Err(EpubError::TooLarge(format!(
                    "total uncompressed size exceeds {MAX_TOTAL_UNCOMPRESSED} bytes"
                )));
            }

            let mut buf = Vec::with_capacity(uncompressed as usize);
            file.read_to_end(&mut buf)
                .map_err(|e| EpubError::InvalidZip(e.to_string()))?;

            entries.insert(name, buf);
        }

        Ok(Self { entries })
    }

    /// Get the raw bytes of an entry by path.
    pub fn read(&self, path: &str) -> Option<&[u8]> {
        self.entries.get(path).map(|v| v.as_slice())
    }

    /// Get the content of an entry as a UTF-8 string.
    pub fn read_str(&self, path: &str) -> EpubResult<&str> {
        let bytes = self
            .read(path)
            .ok_or_else(|| EpubError::InvalidZip(format!("entry not found: {path}")))?;
        std::str::from_utf8(bytes)
            .map_err(|e| EpubError::InvalidZip(format!("non-UTF-8 entry {path}: {e}")))
    }

    /// List all entry paths.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// Check if an entry exists.
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }
}

/// Validate the mimetype entry is first and correct per EPUB spec.
fn validate_mimetype(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> EpubResult<()> {
    let mut file = zip
        .by_index(0)
        .map_err(|e| EpubError::InvalidZip(e.to_string()))?;

    let name = file.name().to_string();
    if name != "mimetype" {
        return Err(EpubError::InvalidZip(format!(
            "first entry must be 'mimetype', found '{name}'"
        )));
    }

    // Per EPUB spec, mimetype must be stored (not deflated).
    if file.compression() != zip::CompressionMethod::Stored {
        return Err(EpubError::InvalidZip(
            "mimetype entry must use Stored compression".into(),
        ));
    }

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| EpubError::InvalidZip(e.to_string()))?;

    if content.trim() != "application/epub+zip" {
        return Err(EpubError::InvalidZip(format!(
            "mimetype content must be 'application/epub+zip', found '{content}'"
        )));
    }

    Ok(())
}

/// Normalize a ZIP entry path, rejecting anything that escapes the root.
///
/// Returns `None` if the path is invalid (absolute, backslash-escaped, etc.).
fn normalize_path(path: &str) -> Option<String> {
    // Reject backslashes (Windows-style zip-slip).
    if path.contains('\\') {
        return None;
    }
    // Reject absolute paths.
    if path.starts_with('/') {
        return None;
    }

    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => continue,
            ".." => {
                components.pop()?;
            }
            c => components.push(c),
        }
    }

    Some(components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Create a minimal valid EPUB zip in memory.
    fn make_test_epub() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", options).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();

            let options_deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("META-INF/container.xml", options_deflated)
                .unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><container xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\" version=\"1.0\"><rootfiles><rootfile full-path=\"content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>").unwrap();

            zip.start_file("content.opf", options_deflated).unwrap();
            zip.write_all(b"<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\"><metadata><dc:title xmlns:dc=\"http://purl.org/dc/elements/1.1/\">Test</dc:title></metadata><manifest/><spine/></package>").unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn open_valid_epub() {
        let data = make_test_epub();
        let container = Container::open(&data).unwrap();
        assert!(container.contains("META-INF/container.xml"));
        assert!(container.contains("content.opf"));
    }

    #[test]
    fn read_str_works() {
        let data = make_test_epub();
        let container = Container::open(&data).unwrap();
        let mimetype = container.read_str("mimetype").unwrap();
        assert_eq!(mimetype, "application/epub+zip");
    }

    #[test]
    fn missing_entry_returns_none() {
        let data = make_test_epub();
        let container = Container::open(&data).unwrap();
        assert!(container.read("nonexistent").is_none());
    }

    #[test]
    fn normalize_path_basic() {
        assert_eq!(normalize_path("foo/bar"), Some("foo/bar".into()));
        assert_eq!(normalize_path("foo/../bar"), Some("bar".into()));
        assert_eq!(normalize_path("./foo"), Some("foo".into()));
        assert_eq!(normalize_path("foo/../../bar"), None);
        assert_eq!(normalize_path("/absolute"), None);
        assert_eq!(normalize_path("foo\\bar"), None);
    }

    #[test]
    fn reject_bad_mimetype() {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", options).unwrap();
            zip.write_all(b"wrong/content-type").unwrap();
            zip.finish().unwrap();
        }
        let err = Container::open(&buf).unwrap_err();
        assert!(err.to_string().contains("mimetype"));
    }
}

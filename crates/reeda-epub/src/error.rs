/// EPUB engine errors.
#[derive(Debug, thiserror::Error)]
pub enum EpubError {
    /// The file is not a valid ZIP archive.
    #[error("invalid ZIP archive: {0}")]
    InvalidZip(String),

    /// container.xml is missing or malformed.
    #[error("missing or invalid container.xml")]
    MissingContainer,

    /// No rootfile found in container.xml.
    #[error("no rootfile in container.xml")]
    MissingRootfile,

    /// The OPF package document is missing or malformed.
    #[error("invalid OPF package document: {0}")]
    InvalidOpf(String),

    /// An unsupported EPUB version.
    #[error("unsupported EPUB version: {0}")]
    UnsupportedVersion(String),

    /// A manifest item referenced by the spine is missing.
    #[error("spine references missing manifest item: {0}")]
    MissingManifestItem(String),

    /// The EPUB exceeds size limits (zip-slip or decompression bomb).
    #[error("EPUB too large: {0}")]
    TooLarge(String),

    /// A nav/TOC file is missing or malformed.
    #[error("invalid navigation document: {0}")]
    InvalidNav(String),

    /// An XHTML content file is malformed.
    #[error("invalid XHTML content: {0}")]
    InvalidContent(String),

    /// A general I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for EPUB operations.
pub type EpubResult<T> = Result<T, EpubError>;

/// Strip `<!DOCTYPE ...>` declarations from XML.
///
/// `roxmltree` does not support DTDs, but EPUB XHTML content often includes
/// `<!DOCTYPE html>` or longer doctype declarations.
pub fn strip_doctype(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len());
    let mut remaining = xml;
    while let Some(doctype_pos) = remaining.find("<!") {
        if let Some(end) = remaining[doctype_pos..].find('>') {
            result.push_str(&remaining[..doctype_pos]);
            remaining = &remaining[doctype_pos + end + 1..];
        } else {
            result.push_str(remaining);
            break;
        }
    }
    result.push_str(remaining);
    result
}

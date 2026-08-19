pub mod app;
pub mod commands;
pub mod events;
pub mod export;
pub mod models;
pub mod platform;
pub mod reader;
pub mod search;
pub mod storage;
pub mod store;

pub use app::{App, OutlineItemView, PdfView, StateSnapshot};
pub use commands::Command;
pub use events::{Event, NarrationState};
pub use export::export_markdown;
pub use models::{
    Annotation, AnnotationId, AnnotationKind, AppSettings, Book, BookFormat, BookId, CfiRange,
    Chapter, ChapterId, HighlightColor, ShelfId, TapZonesLayout, Theme, Typography,
};
pub use reader::{PageBlock, PageContent};
pub use storage::{Database, StorageError, StorageResult};
pub use store::{sha256_hex, BookStore};

/// Returns the current reeda-core crate version.
pub fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_version_is_parseable_semver() {
        let v = crate_version();
        assert_eq!(v.split('.').count(), 3, "expected semver, got {v}");
    }
}

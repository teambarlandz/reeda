# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer](https://semver.org/) (see RELEASE.md).

## [Unreleased]

### Added

- Project scaffolding: Cargo workspace with `reeda-core`, `reeda-epub`,
  `reeda-pdf`, `reeda-search`, `reeda-tts`, `reeda-ui` crates.
- Full documentation set (see [TODO.md](../TODO.md)): PRD, architecture,
  roadmap, technical design, ADRs, feature specs (EPUB, PDF, TTS,
  highlights, search), data model, platform/build/CI/testing/performance/
  accessibility/security/localization/release guides.
- Public GitHub repository.
- **M0 domain layer** (`reeda-core`): IDs, Book, Chapter, Annotation, AppSettings models;
  Command/Event enums; Platform trait with Desktop + Android stubs; App dispatch + snapshot;
  SQLite storage with migrations, CRUD queries, WAL mode.
- **M0 CI**: `ci.yml` (fmt, clippy, test, doc-build) and `build-apk.yml` (Android debug APK).
- **M0 Slint UI shell** (`reeda-ui`): Theme.slint (Light/Sepia/Night palettes), AppRoot.slint
  (Library↔Reader navigation), LibraryScreen.slint (empty-state onboarding + import button),
  ReaderScreen.slint (page canvas + chrome overlay), Dialogs.slint (error dialog).
- **M0 Android stubs**: UI-layer SAF picker, intent reader, permission request stubs gated
  behind `platform-android` feature.
- **M1 EPUB reader core**:
  - EPUB ZIP container reader with zip-slip guard and decompression bomb guard.
  - OPF metadata/manifest/spine parser supporting EPUB2 and EPUB3.
  - Navigation parser: EPUB3 `nav.xhtml` + EPUB2 `toc.ncx` → unified `TableOfContents`.
  - XHTML → `DocumentModel` parser (headings, paragraphs, lists, code blocks, images, links,
    bold/italic/strikethrough, subscript/superscript).
  - CFI (Canonical Fragment Identifier) position model with parse/serialize and range support.
  - Deterministic paginator: block-aware character-count pagination with layout hash for cache
    keying, page_containing/cfi_of_page_start lookup, and monotonic progress tracking.
  - `ParsedDocRegistry` in reeda-core: bridges reeda-epub parsing into App state.
  - `Import` command: reads EPUB file, parses, extracts metadata, adds to library.
  - `OpenBook` triggers pagination, exposes page_text/page_blocks in `StateSnapshot`.
  - `SetTypography` triggers re-pagination; `JumpTo` uses CFI→page lookup.
  - Reader screen UI: AppRoot.slint forwards page-text, book-title, progress to ReaderScreen.
  - `main.rs` wires Slint callbacks (next/prev/back) to `App::dispatch` via `Rc<RefCell<App>>`.
  - `update_ui()` pushes `StateSnapshot` into Slint properties after each dispatch.
  - 84 tests across workspace (40 reeda-core, 40 reeda-epub, 4 others).
- **M2 Library & metadata**:
  - `BookStore` file storage: SHA-256 dedup, atomic writes, book/cover file layout
    under `books/<id>/` and `covers/<id>`, file cleanup on delete.
  - Import pipeline: copies EPUB to storage, extracts metadata (language, publisher,
    description, published date), extracts and stores cover images.
  - Library grid UI: `BookCard` component (cover placeholder, title, author, progress
    bar), scrollable list, book count, edit button per card.
  - Reading progress: saved on page turn/close, restored on open; persisted to SQLite.
  - Metadata editing: `EditMetadata` command + `MetadataDialog` (title/author override),
    persisted to SQLite.
  - Settings screen v1: `SettingsScreen.slint` with theme picker (Light/Sepia/Dark),
    font size and line height controls; theme applied live via Slint global.
  - SQLite persistence wiring: books, chapters, reading position, metadata, settings
    saved on mutation; `load_books()`/`load_settings_from_db()` restore at startup.
  - 103 tests across workspace (58 reeda-core, 42 reeda-epub, 3 others).
- **M3 Highlighting & notes**:
  - Selection engine (`reeda-epub/selection.rs`): `GlobalRange` over the global
    block sequence, CFI anchoring with round-trip + orphan detection,
    `intersect_range_with_page()` → clipped segments; global-block fix for
    `find_page_for_cfi`.
  - Highlight rendering: `build_page_lines` produces plain/highlight `LineRun`s
    per visual line; ReaderScreen shows translucent color backgrounds, underline,
    note dot, and a tap popover (4 color swatches / delete).
  - Notes: `AddNote` (attach to highlight or standalone), NotesScreen grouped by
    chapter with jump-to-annotation (HIL-06).
  - Bookmarks: ribbon toggle keyed off page-start CFI, BookmarksScreen with jump
    and delete.
  - Markdown export (`export_markdown`): per-chapter grouping in spine order,
    snippets + inline notes, `annotations.md` written next to the book file from
    the NotesScreen Export button.
  - Annotation persistence: full CRUD wired into App commands (insert/update/
    soft-delete, reload on open) with restart persistence + HIL-08 font-size
    invariance tests.
  - 131 tests across workspace (74 reeda-core, 54 reeda-epub, 3 others).

### Changed

- (none yet)

### Fixed

- (none yet)

# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer](https://semver.org/) (see RELEASE.md).

## [Unreleased]

### Added

- **M7f Play Store assets:** adaptive launcher icon (brand green + open
  book glyph; vector drawable + monochrome themed-icon variant + PNG
  mipmap fallbacks), 512 px store icon + 1024×500 feature graphic
  (regenerable via `scripts/make_icons.ps1`); store listing draft
  (`docs/store/listing.md`); privacy policy draft
  (`docs/store/privacy_policy.md`, zero-collection v1, hosted page before
  release). Device screenshots deferred to M7g.
- **M7e security review:** crash-reporting decision made (ADR OQ-2: none
  in v1, anonymous opt-in evaluated for v1.1); `cargo audit` CI job added
  (every push/PR + weekly schedule, fails on `high`); clippy
  `undocumented_unsafe_blocks = deny` enabled workspace-wide with SAFETY
  comments added to both Android JNI blocks in `reeda-tts`; Android backup
  rules implemented (`android:fullBackupContent` +
  `android:dataExtractionRules`: `reeda.sqlite` + `books/` + `covers/`
  backed up, `index/` excluded). 214 tests green, clippy clean.
- **M7d performance pass:** the PDF raster path now uses the 128 MB LRU
  `RasterCache` (PDF_SPEC §5) — visible-window pages are served from the
  cache and pages outside the window are dropped from the image model, so
  scrolling re-blits instead of re-rasterizing and memory stays within the
  byte budget; fit-to-width rasters invalidate on a material viewport
  resize. New release-gated benchmark tests (`scripts/bench_desktop.ps1`):
  PDF first-raster p95 (measured 19.6 ms vs < 250 ms budget) and cached
  blit p95 (0.1 µs vs < 8 ms) in `reeda-pdf/tests/perf_bench.rs`; EPUB
  pagination p95 for avg/long chapters (52.8 µs / 130.7 µs) in
  `reeda-epub/tests/perf_bench.rs`; search index/query gate (3.4 s) in
  `reeda-search/tests/perf_fixture.rs`. 214 tests green, clippy clean.
- **M7c localization framework:** all 122 user-facing strings across the 9
  Slint screens are wrapped in Slint's native `@tr("…")` (msgid = English
  text) and bundled via gettext catalogs
  (`crates/reeda-ui/translations/<lang>/LC_MESSAGES/reeda-ui.po`) using
  `slint_build::compile_with_config().with_bundled_translations(...)`.
  Ships `en` (identity source of truth) and `en-GB` ("colour" spelling
  variant). Runtime auto-selects from the system locale (exact > base
  language > default), with `slint::select_bundled_translation` as a manual
  override; plural rules and RTL mirroring come from Slint 1.17. Replaces
  the draft custom-JSON-catalog design (ADR-011, LOCALIZATION.md).
- **M7b accessibility pass:** `accessible-role: button` + meaningful
  `accessible-label` on all 54 interactive elements across the 9 Slint
  screens (back, theme, search, narration, highlight, bookmark, note,
  jump, zoom controls, dialogs); backdrop-dismiss and page-turn hit
  zones intentionally left unexposed.
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
- **M4 Full-text search**:
  - `reeda-search` (Tantivy, ADR-009): schema (book_id, spine/block/char
    locators, boosted title + body, stored chapter_title/language), version-stamped
    index with auto-rebuild, `index_book`/`index_many` (replace-then-add),
    `delete_book`, BM25 ranking with title boost 2.0, AND-by-default queries,
    `<mark>` snippets, CFI locator on hits, per-book filter for in-book search.
  - English analyzer: simple segmentation + lowercase + stopword filter
    (EN_STOPWORDS); registered at index and query time (INDEX_VERSION 2).
  - `SearchService` in reeda-core: index on import, delete on delete, query
    across library or within the open book; wired into desktop startup
    (`reeda_data/` next to the app).
  - Search commands/events/snapshot (`Search`, `OpenSearchHit`,
    `ReaderSearch*`, `ReaderSearchState`) with debounced query dispatch,
    open-at-match via CFI jump, and a cyan transient highlight synced with the
    reader (cleared on page turn / close / new search).
  - UI: full-screen Library search (results grouped by book, chapter headings,
    no-results state) and an in-reader overlay (prev/next arrows with wrap,
    match counter "x / y", close button).
  - M4.7 perf fixture (`reeda-search/tests/perf_fixture.rs`): deterministic
    50-book multi-language corpus (diacritics, long book, empty book) asserting
    the M4 exit criterion — index build < 10 s/100 books and query p95 < 1 s in
    release (debug runs a scaled-down corpus with generous smoke bounds).
  - 155 tests across workspace (83 reeda-core, 54 reeda-epub, 16 reeda-search,
    2 integration/perf).
- **M5 Read aloud (TTS)**:
  - `reeda-tts` chunker (`chunk.rs`): sentence-boundary detection with
    abbreviation guard list, 4000-char chunk limit, text cleaning (soft
    hyphens, nbsp, control chars), skip of images/rules/repeated chapter
    titles, chunk→CFI mapping via `GlobalRange`.
  - `reeda-tts` engine (`engine.rs`): `TtsHost` trait (Any-supertrait, e.g.
    `FakeTtsHost` for desktop/tests) + `NarrationEngine` state machine
    (Idle/Loading/Speaking/Paused/Error), queue depth 2 with prefetch,
    monotonic utterance ids, 3-error retry policy, rate 0.5–2.5 / pitch
    0.5–1.5 clamping.
  - Core narration wiring: `StartNarration`/`PauseNarration`/`ResumeNarration`/
    `StopNarration`/`NarrationSkip { delta }`/`SetTtsSpeed`/`SetTtsPitch`/
    `PollNarration` commands; word highlight events + transient highlight,
    auto page turn on chunk CFI crossing the page end (TTS-05), chapter
    auto-advance, `NarrationFinished` on last chapter; narration cleared on
    stop/close-book; `TtsHost` injectable via `App::set_tts_host`.
  - Reader TTS bar (ReaderScreen.slint): play/pause, stop, chapter skip
    fwd/back, speed chip cycling 0.5–2.5; 300 ms narration poll timer drives
    word highlights and bar state from the snapshot.
  - Android JNI bridge (feature `platform-android`): `AndroidTtsHost`
    (jni + ndk-context) over `android/src/io/reeda/app/TtsShim.java`
    (TextToSpeech init/speak/stop/rate/pitch + `UtteranceProgressListener`
    onStart/onRangeStart/onDone/onError marshalled via the exported
    `Java_io_reeda_app_TtsShim_onEvent` symbol); wired in at startup on
    Android; manifest already declares foreground-service/wake-lock
    permissions. CI: `build-apk.yml` builds the APK with
    `--no-default-features --features platform-android` and adds a
    aarch64-linux-android compile check for the bridge.
  - Device-dependent items (media notification with lock-screen controls,
    audio focus, wake-lock, ±15 s within chunk) deferred to emulator/device
    verification in M7 (see docs/TTS_SPEC.md §2).
  - 159 tests across workspace (88 reeda-core, 54 reeda-epub, 16
    reeda-search, 18 reeda-tts, 1 perf fixture).
- **M6 PDF reader**:
  - `reeda-pdf` renderer (M6.2): `render.rs` rasterizes pages via PDFium
    (`pdfium-render` `PdfBitmap`, RGBA at 96 dpi × scale, 4096 px/axis
    cap), `cache.rs` 128 MB LRU keyed `(page, scale_bucket, theme)`,
    `theme.rs` render-time night (luminance-preserving invert-ish) and
    sepia filters, multi-page document + page-size APIs, error mapping to
    `PdfError` (open/render/missing-library).
  - Core PDF support (M6.3): `ImportPdf`/`OpenPdf`/`PdfPage` commands,
    `PdfState` + `PdfView { page_count, page_sizes, path, outline }` in
    the snapshot, import validation via PDFium, dedup (keyed by path),
    store + SQLite persistence, narration guard (no TTS for PDFs yet,
    TTS-07 P2), path resolution against the import directory.
  - Reader PDF mode (M6.4): continuous vertical page canvas in
    ReaderScreen (image per page, fit-to-width default, zoom 0.25×–5×
    bar, double-tap fit-width ↔ 100 % toggle), auto-hiding page
    indicator + jump-to-page dialog, viewport raster loop in main.rs
    (`PdfUiState`: LRU cache, scroll-driven render of visible pages,
    scale buckets), theme change re-renders with the night/sepia filter,
    `PdfView` export from reeda-core.
  - Outline support (M6.5): `reeda-pdf::outline::extract_outline`
    flattens the PDFium bookmark tree pre-order (iterative, no stack
    overflow on deep trees) into `{ title, page_index, depth }`;
    `OutlineItemView` in the snapshot; reader chrome "≡" toggle opens an
    outline panel (depth indentation, tap-to-jump via
    `pdf-outline-jumped`, empty-state message).
  - 212 tests across workspace (98 reeda-core, 54 reeda-epub, 25
    reeda-pdf, 16 reeda-search, 18 reeda-tts, 1 perf fixture).

### Changed

- `pdf_jump` in main.rs now takes a 1-based `u32` page (dialog path
  parses the text, outline path passes the page directly).
- **M7 desktop packaging:** PDFium is bundled with the app — no runtime
  download. New `scripts/package.ps1` builds the release binary and copies
  `pdfium.dll` next to `reeda-ui.exe` into `dist/reeda-<version>-win-x64.zip`;
  Windows DLL search order loads it via `bind_to_system_library` with zero
  configuration (verified: reeda-pdf tests green without
  `PDFIUM_LIBRARY_PATH`; packaged exe smoke-tested).

### Fixed

- (none yet)

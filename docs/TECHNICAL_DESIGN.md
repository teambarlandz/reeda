# Technical Design — Reeda

> Status: draft · Version: 0.2 · Owner: @teambarlandz · Last updated: 2026-08-17
> Supplements ARCHITECTURE.md with module-level design, data flow, and code
> conventions. Read ARCHITECTURE.md first.

## 1. Workspace layout

```
Cargo.toml            # [workspace] members = crates/*
rust-toolchain.toml   # pinned stable + Android targets
crates/
  reeda-core/
    src/
      app.rs          # App, command dispatch, state diffing
      commands.rs     # all UI → core commands (enum)
      events.rs       # core → UI events (enum)
      models/         # book, chapter, position(CFI), annotation, settings
      services/       # library, reader_session, annotation, import_pipeline
      storage/        # db (migrations, queries), files, covers, backup
      platform/       # Platform trait: SAF, TTS host, notification, wake-lock
    migrations/       # embedded SQL, one file per version
  reeda-epub/
    src/
      container.rs    # zip open/validate/read (zip-slip guarded)
      opf.rs          # metadata, manifest, spine, cover
      nav.rs          # nav.xhtml + toc.ncx → TableOfContents
      xhtml.rs        # html5ever → ElementTree (subset)
      css.rs          # cssparser subset: rules → typed styles
      style.rs        # cascade → per-node ComputedStyle
      document.rs     # DocumentModel (chapters, blocks, images, links)
      paginator.rs    # measure → pages; CFI ↔ page mapping
      cfi.rs          # CFI parse/serialize (EPUB_SPEC.md §7)
      locate.rs       # locate(chapter, offset) → text position
  reeda-pdf/
    src/
      doc.rs          # PdfDocument wrapper (pdfium-render)
      raster.rs       # page → RGBA, LRU cache, decode-scale
      outline.rs      # PDF TOC → tree
  reeda-search/
    src/
      indexer.rs      # Tantivy writer, queue, rebuild
      query.rs        # tokenize → ranked results
      schema.rs       # fields: title, body, book_id, block_ref
  reeda-tts/
    src/
      engine.rs       # NarrationEngine (state machine)
      android_bridge.rs # JNI calls: init, speak, stop, listeners
      focus.rs        # audio focus + ducking
      notify.rs       # media notification actions
  reeda-ui/
    src/
      main.rs         # android entry (android-activity init)
      app.rs          # SlintApp: binds commands ↔ core::App
      screens/        # library, reader, search, settings, highlights
      widgets/        # custom Slint components (.slint files)
      theme.rs        # Light/Sepia/Dark palettes
      i18n.rs         # catalogs, plural rules
    ui/               # *.slint component files
android/              # AndroidManifest.xml, icons, res, cargo-apk config
.github/workflows/    # ci.yml, build-apk.yml, release.yml
```

## 2. Module contracts

### 2.1 `reeda-core::app`
```rust
pub struct App { /* state, worker handles, platform box */ }

pub enum Command {
    Import { uri: String },
    OpenBook { book_id: BookId },
    CloseBook,
    TurnPage { dir: PageDir },
    JumpTo { locator: Cfi },
    SetTypography(Typography),
    SetTheme(ThemeId),
    AddHighlight { range: CfiRange, color: HighlightColor },
    AddNote { highlight_id: AnnotationId, text: String },
    ToggleBookmark { position: Cfi },
    Search { query: String },
    StartNarration { .. }, PauseNarration, StopNarration,
    // … full list in commands.rs
}

pub enum Event {  // emitted to UI after state change
    LibraryChanged, ReaderPageChanged { page: PageView }, ProgressSaved { cfi: Cfi },
    NarrationState(NarrationState), WordHighlight { cfi: Cfi, range: (u32,u32) },
    ImportFinished { book_id: BookId }, ImportFailed { error: ImportError },
    SearchResults { results: Vec<SearchHit> }, // …
}
```
Rules: commands are fire-and-forget; `App::snapshot()` returns the current
`StateSnapshot` (serializable) that the UI renders. UI never queries engines
directly.

### 2.2 `reeda-epub::paginator`
```rust
pub struct PageLayout { width: f32, height: f32, typography: Typography }

pub fn paginate(doc: &DocumentModel, layout: &PageLayout)
    -> Pages;                       // deterministic given inputs
pub fn page_containing(pages: &Pages, cfi: &Cfi) -> PageIndex;
pub fn cfi_of_page_start(pages: &Pages, idx: PageIndex) -> Cfi;
```
`Pages` is `Send + Sync` and holds: per-page first/last text offsets,
image refs, link targets, footnote refs, and highlight ranges clipped to the
page. It is cheap to rebuild after typography changes (measurement cache).

### 2.3 `reeda-tts::engine`
```rust
pub enum NarrationState { Idle, Loading, Speaking, Paused, Stopping, Error(String) }

pub trait TtsHost {   // implemented by android_bridge
    fn speak(&self, text: &str, utterance_id: u64, config: VoiceConfig);
    fn stop(&self); fn pause(&self) -> bool; fn resume(&self);
    fn set_speed(&self, f32);
}
pub trait TtsListener { // called on host thread
    fn on_start(&self, u: u64);
    fn on_done(&self, u: u64, was_interrupted: bool);
    fn on_range(&self, u: u64, start: usize, end: usize); // word/char range
    fn on_error(&self, u: u64);
}
```
Engine splits chapter text into speakable chunks (sentence-granular, ≤ 4000
chars), maps chunk offsets to CFI via `reeda-epub::locate`, and drives
page/word highlighting from `on_range`.

## 3. Data flow details

### 3.1 Rendering pipeline (EPUB page)
```
[FontConfig + TextBlock slices] → Slint Text element (rich text via
  <a>/<b>/<i> markup generated by reeda-ui from ComputedStyle) → rendered
```
Notes:
- Slint renders text with `cosmic-text`; we never rasterize EPUB ourselves.
- Bold/italic/underline/color from CSS cascade map onto Slint rich-text spans.
- Images inline: `<img>` from extracted resources, sized by CSS, wrapped in a
  Slint `Image` element inside a horizontal box; measured during pagination
  via a fixed default (10 % height) then refined by layout pass.
- The paginator runs in the worker pool; while running, UI shows current page
  and disables rapid re-pagination (debounce 120 ms).

### 3.2 Selection & highlights
- Selection: Slint gesture (long-press + drag) → word-boundary snapped range
  (via `locate` + word metadata in `Pages`) → popover with actions.
- Highlight persistence invariant (HIL-08): store CFI range; on every page
  render, intersection of highlight ranges with the page is recomputed —
  never cached page indices. See HIGHLIGHTS_SPEC.md §6.

### 3.3 PDF rendering
- `pdfium-render` opens file; pages rasterized at `device_pixel_ratio ×
  zoom` size via `Bitmap::render` → `Rgba8` → `Image`. LRU cache holds ~8
  full pages (budget ≤ 128 MB, configurable). Prefetch neighbors on idle.
- Night theme: render-time luminance filter (see PDF_SPEC.md §6).

### 3.4 Import pipeline state machine
```
Picked → Hashing → Copying → Parsing(meta) → Extracting → Indexing → Done
                    └────────── Error(classified) ──────────▶ Retryable?
```
Progress surfaced via `ImportProgress` events (percent + stage). Idempotent:
re-import of same sha256 with different URI updates the existing record.

## 4. Concurrency & locking

- `App` is `!Sync` in spirit; a single `Mutex<AppInner>` guarded; worker
  results are applied via `Event` queue drained by the UI thread
  (`try_recv` each frame, 60 fps safe).
- SQLite: one `Connection` per thread (writer on its own thread), WAL mode.
  All queries are prepared at startup (zero re-prepare in hot paths).
- Pagination jobs are cancellation-aware: a new typography change abandons the
  previous job via generation counter.
- TTS callbacks arrive on a binder thread → marshalled to UI via channel.

## 5. Configuration & feature flags

- Cargo features: `reeda-ui`/`reeda-core` = `default`; platform crates behind
  `platform-android`. Host dev builds use a stub platform (fake picker, no
  TTS) so most UI work is possible on desktop.
- Runtime settings table (DATA_MODEL.md §6): typography defaults, theme,
  TTS voice/speed/pitch, edge-tap layout, locale.

## 6. Testing hooks

- `reeda-epub` ships a `fixtures` test-helper crate generating deterministic
  EPUBs (sizes, charset oddities, malicious zips).
- `reeda-core` runs headless against an in-memory SQLite; a `TestPlatform`
  records TTS/notification calls for assertion.
- Golden pagination tests: fixed input → expected CFI-per-page table
  (TESTING.md §4).

## 7. Performance budgets (summary)

| Metric | Budget |
|--------|--------|
| Cold start → reader ready | < 1.2 s (dev device) |
| Page turn (render) | < 33 ms p95 |
| Typography change re-pagination | < 150 ms for avg chapter |
| Search 50-book library | < 1 s p95 |
| PDF page raster | < 250 ms first, cached after |
| App heap (reading, EPUB) | < 200 MB |
| Index build | < 10 s per 100 books, background |

See [PERFORMANCE.md](PERFORMANCE.md).

## 8. Coding conventions

- `cargo fmt` enforced; clippy with `-D warnings` in CI.
- All public items documented (`#![deny(missing_docs)]` on engine crates).
- Errors: `thiserror` per crate; `core::ImportError` carries
  `kind` + `user_message_key` for i18n.
- No `unwrap`/`expect` outside tests; no unsafe except JNI boundary and
  pdfium FFI (isolated behind unsafe-free wrappers).
- IDs: `uuid::Uuid` v4 strings for books/annotations; CFI strings for
  positions.
- Conventions are enforced in [CONTRIBUTING.md](CONTRIBUTING.md).

# Architecture — Reeda

> Status: draft · Version: 0.2 · Owner: @teambarlandz · Last updated: 2026-08-17

## 1. Principles

1. **Pure Rust everywhere.** Slint for UI, Rust for everything else. The only
   non-Rust artifacts are (a) the Android activity shim required by the
   platform, (b) JNI glue (Rust `jni` crate, no Kotlin logic), and (c) PDFium
   (C++) via a crate, used exclusively for rasterization.
2. **Crate-per-concern.** Each crate has one job and a stable public API; the
   UI depends on core services, never on parsing internals.
3. **Offline-first.** No network in v1. All data lives in app-private storage.
4. **Deterministic rendering.** Given (book content + viewport + font config),
   pagination is a pure function → enables tests, TTS sync, and position
   anchoring.
5. **Everything async.** Book parsing, indexing, PDF rasterization run off the
   UI thread; results arrive via channels/handles that Slint polls.

## 2. System context

```
┌───────────────────────────── Android device ─────────────────────────────┐
│                                                                          │
│  ┌────────────────────────────┐                                          │
│  │  reeda-ui (Slint UI crate) │  UI thread: Slint event loop, rendering  │
│  │  screens · gestures · theming                                        │
│  └────────────┬───────────────┘                                          │
│               │ Rust API calls (in-process)                              │
│  ┌────────────▼───────────────┐                                          │
│  │  reeda-core (app core)     │  services: library, reader, annotations,│
│  │  state machine, commands   │  search, narration coordinator           │
│  └──┬──────┬──────┬──────┬────┴──┐                                       │
│     │      │      │      │       │                                      │
│  ┌──▼──┐┌──▼───┐┌──▼────┐┌──▼───┐┌▼────────┐                             │
│  │epub ││ pdf  ││search ││ tts  ││storage  │   engine crates            │
│  │crate││crate ││crate  ││crate ││(rusqlite│                             │
│  └─────┘└──────┘└───────┘└──┬───┘│ +books) │                             │
│                             │     └────────┘                             │
│                    ┌────────▼─────────┐                                  │
│                    │ Android platform │  JNI bridge (activity, SAF       │
│                    │  (Java/NDK, TTS, │  picker, TextToSpeech,           │
│                    │  PDFium, WakeLock│  notification, a11y, fs)         │
│                    └──────────────────┘                                  │
└──────────────────────────────────────────────────────────────────────────┘
```

## 3. Crate map

| Crate | Responsibility | Key crates/deps | Notes |
|-------|----------------|-----------------|-------|
| `reeda-core` | Domain models, app state, command bus, services (library, reader session, annotations, narration) | serde, thiserror, chrono, uuid, rusqlite | UI talks only to this crate |
| `reeda-epub` | ZIP container, OPF/spine/nav parsing, XHTML→document model, CSS subset engine, CFI, paginator | zip, quick-xml/roxmltree, html5ever, cssparser | Pure computation, no I/O policy |
| `reeda-pdf` | PDFium wrapper: open, page count, rasterize, TOC, (v1.1: text, search) | pdfium-render | Raster cache lives here |
| `reeda-search` | Tantivy index wrapper: build, query, rank, per-position hits | tantivy | Background indexer |
| `reeda-tts` | Narration engine + JNI bridge to Android TextToSpeech; audio focus; word-sync callbacks | jni | Platform shim, see TTS_SPEC.md |
| `reeda-ui` | Slint app: screens, navigation, gestures, theming, i18n catalogs | slint, slint-builtin-widgets | Android entry point |

## 4. Runtime model

### 4.1 Threads
- **UI thread** — Slint event loop, rendering, gestures. Never blocks: no I/O,
  no parsing, no DB.
- **Worker pool** (`std::thread` + mpsc channels; no async runtime in v1) —
  long jobs: import pipeline, pagination, index builds, PDF rasterization.
- **Narration thread** — owned by `reeda-tts`, driven by TTS callbacks;
  coordinates with the UI via the command bus.
- **DB access** — single dedicated writer thread (SQLite serializes anyway);
  reads served from an in-memory cache + connection pool guarded by rwlock.

> Async decision rationale in ADR-003. `tokio` may replace the pool when
> network features arrive (v2).

### 4.2 Command bus
The UI dispatches commands (`ImportBook`, `TurnPage`, `SetFontSize`, …) through
`reeda-core::app::App` (single entry point). App validates, mutates state,
schedules background work, and returns a `StateSnapshot` diff that Slint
applies via bound properties. This keeps the UI declarative and the core
testable headlessly (see TESTING.md).

```
UI ──command──▶ App ──work──▶ worker/engine ──event──▶ App ──diff──▶ UI
```

### 4.3 Position model (EPUB)
A **CFI-compatible** position (`epub:cfi`) is the canonical locator:
`epubcfi(/6/4[chap03]!/4/2/1:42)`-style, extended with a **range** form for
highlights/notes. Pagination maps CFI ⇄ page index deterministically. All
persisted state (progress, highlights, bookmarks) stores CFI — never page
numbers (pages depend on viewport/fonts). Details: EPUB_SPEC.md §7.

## 5. Storage architecture

- **SQLite (rusqlite, bundled)** at `context.filesDir/reeda.db` — metadata,
  library, annotations, settings, progress. Schema & migrations in
  DATA_MODEL.md (schema_migrations table, `migrations/` dir, versioned
  embedded SQL).
- **Book files** at `context.filesDir/books/<book_id>/` — extracted EPUB
  (folder, for cheap random access), PDF copied verbatim.
- **Covers** at `context.filesDir/covers/<book_id>.webp`.
- **Search index** at `context.filesDir/index/` (Tantivy) — rebuildable.
- **Backup** (v1.1): zip of `reeda.db` + `books/` via SAF export.
- No MediaStore writes; books are private. Nothing leaves the device.

## 6. Android integration layer

- Entry: `android-activity` via Slint's Android backend (`cargo-apk`).
- A **minimal** Java `Activity` shim (≈100 lines) forwards:
  `onActivityResult` (SAF), intents, `TextToSpeech` callbacks, notification
  intents, audio focus, wake-lock, and a11y actions — via JNI into
  `reeda-tts`/`reeda-core`. Nothing else lives in Java.
- Permissions (minimal set): `FOREGROUND_SERVICE`,
  `FOREGROUND_SERVICE_MEDIA_PLAYBACK`, `POST_NOTIFICATIONS`,
  `WAKE_LOCK` (declared, used only while narrating). No storage permission
  needed (SAF). See PLATFORM.md.

## 7. Key flows

### 7.1 Import (EPUB)
```
Picker/Intent → core copies bytes → hasher (sha256) → dedupe check →
write books/<id>/ → open container.xml → parse OPF (metadata, spine, manifest)
→ extract nav/toc → parse XHTML → build DocumentModel → generate cover →
insert DB row → spawn index job (search crate) → refresh library UI
```
Failure at any step → DB rollback, file cleanup, user-visible error with
cause classification (corrupt zip, missing OPF, unsupported version).

### 7.2 Page turn
```
TurnPage(next) → App → paginator.advance() (pure) → new CFI anchor →
render visible chapter slice to Slint text (shaped once) → persist progress
(debounced 5 s + on pause) → TTS resync if narrating
```

### 7.3 Narration
```
Narrate(chapter) → tts crate → Android TTS queue (SSML-ish plain text chunks)
→ onRangeStart(word) → highlight word in current page; onDone → advance page
if needed → onError → recovery policy (skip chunk, 3-strike → pause)
```
Full state machine in TTS_SPEC.md §5.

## 8. Error handling & resilience

- Typed errors per crate (`Result<T, EpubError>`, `ImportError` with
  `UserMessage`), mapped to UI strings via i18n.
- Crash-safe progress: `PRAGMA synchronous=NORMAL` + WAL; progress row updated
  on every page turn in memory, flushed on 5 s cadence, on `onPause`, and on
  `onStop` (FR-03).
- Never trust external files: all parsing is fuzz-tested (TESTING.md §6),
  size-capped imports, decompression-bomb guards in EPUB zip reader
  (zip-slip protection: entry paths validated against `..`).
- OOM safety for PDF: LRU raster cache (budget in PERFORMANCE.md), pages
  rasterized at display resolution only.

## 9. Security & privacy

- No network, no analytics, no advertising IDs (v1). Crash reporting opt-in
  (v1.1+).
- TTS content never leaves the device (on-device Android TTS engines; cloud
  engines user-chosen explicitly).
- Encryption at rest: SQLite content is app-private; optional app-level
  passphrase (SQLCipher) tracked as P1 — see DRM_SECURITY.md.

## 10. Evolution paths

- **iOS (v1.1+):** swap the Android integration layer (crate boundary is
  `platform/`-scoped traits); UI and engines unchanged. Slint supports iOS.
- **Sync (v2):** schema is sync-ready (UUID PKs, `updated_at` LWW columns,
  soft deletes); a sync adapter plugs into the command bus.
- **Network (v2):** introduce `tokio`; workers become tasks; import/purchase
  endpoints behind the same command bus.

## 11. References

- [PRD](PRD.md) · [TECHNICAL_DESIGN](TECHNICAL_DESIGN.md) ·
  [DATA_MODEL](DATA_MODEL.md) · [ADR](ADR.md) · [PLATFORM](PLATFORM.md) ·
  [PERFORMANCE](PERFORMANCE.md)

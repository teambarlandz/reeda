# TODO — Reeda

> Master planning index. Every task below is traced to a milestone in
> [ROADMAP.md](docs/ROADMAP.md) and a spec in `docs/`. Statuses:
> `[ ]` = pending, `[~]` = in progress, `[x]` = done, `[-]` = cancelled.

---

## M0 — Foundations (target: 2–3 weeks)

**Exit criterion:** `cargo apk run` shows the shell with an empty library + import button.

### M0.1 GitHub & CI

- [ ] Enable branch protection on `main`: PR-required, 1 review minimum, CI must pass
- [ ] Create `.github/workflows/ci.yml` — host-side CI (fmt, clippy, test, doc-build)
- [ ] Create `.github/workflows/build-apk.yml` — Android debug APK build (arm64 + x86_64)

### M0.2 reeda-core: domain models, commands, events, platform trait

- [ ] `models/mod.rs` — `BookId`, `ChapterId`, `AnnotationId` type aliases (newtypes over `uuid::Uuid`)
- [ ] `models/book.rs` — `Book` struct (id, title, author, format, file_path, cover_path, sha256, language, publisher, description, published_at, imported_at, last_opened_at, last_position, progress_pct, updated_at, deleted_at)
- [ ] `models/chapter.rs` — `Chapter` struct (id, book_id, spine_index, title, href, file_hash, char_count, updated_at)
- [ ] `models/annotation.rs` — `Annotation` struct + `AnnotationKind` enum (`highlight | note | bookmark`), `HighlightColor` enum (yellow/green/blue/pink), `CfiRange` type
- [ ] `models/settings.rs` — `AppSettings` struct (theme, font_family, font_size_pt, line_height, margin, justify, tts_speed, tts_pitch, tts_wakelock, tap_zones_layout, locale, first_run_done) with serde defaults
- [ ] `models/mod.rs` — re-export all models
- [ ] `commands.rs` — `Command` enum (Import, OpenBook, CloseBook, TurnPage, JumpTo, SetTypography, SetTheme, AddHighlight, AddNote, ToggleBookmark, Search, StartNarration, PauseNarration, StopNarration, UpdateSettings, DeleteBook)
- [ ] `events.rs` — `Event` enum (LibraryChanged, ReaderPageChanged, ProgressSaved, NarrationState, ImportFinished, ImportFailed, SearchResults)
- [ ] `platform/mod.rs` — `Platform` trait: `pick_file()`, `get_intent_data()`, `request_permission()`, `start_narration_service()`, `stop_narration_service()`
- [ ] `platform/desktop.rs` — `DesktopPlatform` stub (default feature)
- [ ] `app.rs` — `App` struct (state, platform box), `App::dispatch(Command) -> Vec<Event>`, `App::snapshot() -> StateSnapshot`
- [ ] `app.rs` — `StateSnapshot` struct (library list, current book, current page, settings, narration state) — serializable
- [ ] Unit tests for domain models (construction, defaults, serde round-trip)

### M0.3 reeda-core: storage layer (SQLite)

- [ ] Add `rusqlite` (bundled) + `rusqlite` workspace dependency
- [ ] `storage/mod.rs` — `Database` struct wrapping rusqlite connection (WAL, foreign_keys, busy_timeout)
- [ ] `storage/mod.rs` — migration runner: read `migrations/` dir, apply in order, record in `schema_migrations`
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE books` (matching DATA_MODEL.md §2.1)
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE chapters` (DATA_MODEL.md §2.2)
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE annotations` (DATA_MODEL.md §2.3)
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE bookshelves` + `bookshelf_books` (DATA_MODEL.md §2.4)
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE settings` (DATA_MODEL.md §2.5)
- [ ] `migrations/0001_initial.sql` — `CREATE TABLE schema_migrations` (DATA_MODEL.md §2.6)
- [ ] `storage/queries.rs` — prepared statements: library_grid, continuar, annotations_for_book, upsert_book, delete_book, get/set_setting
- [ ] Unit tests: migration 0→N on empty DB, CRUD round-trip on each table

### M0.4 reeda-ui: Slint app shell

- [ ] Add `slint` dependency to `reeda-ui/Cargo.toml` (with `backend-android` feature for android, no default features on desktop)
- [ ] Create `reeda-ui/ui/Theme.slint` — palette tokens: Light (#F7F4EC), Sepia (#F1E8D8), Night (#101418), accent #2E8B57, highlight colors; typography tokens
- [ ] Create `reeda-ui/ui/AppRoot.slint` — root component with theme provider + stack navigation
- [ ] Create `reeda-ui/ui/LibraryScreen.slint` — grid placeholder, empty-state onboarding ("Import your first book"), import FAB
- [ ] Create `reeda-ui/ui/ReaderScreen.slint` — page canvas stub, chrome overlay stub (top/bottom bars)
- [ ] Create `reeda-ui/ui/Dialogs.slint` — confirm/error/progress dialog components
- [ ] `src/theme.rs` — Rust-side theme enum (Light/Sepia/Dark) + Slint model binding
- [ ] `src/main.rs` — Slint entry point: init app, bind theme, show AppRoot, event loop
  - `platform-desktop` feature: `slint::platform::Backend::new()` desktop mode
  - `platform-android` feature: `android-activity` integration

### M0.5 reeda-ui: navigation + empty library screen

- [ ] Wire navigation: Library → Reader (stub) → back to Library
- [ ] Library screen empty state: illustration placeholder + "Import your first book" text + import button
- [ ] Import button placeholder: prints to log / shows toast on desktop, triggers SAF on android (stub in M0)

### M0.6 Android bridge stubs

- [ ] `reeda-ui/src/android/mod.rs` — SAF file picker stub (returns `Result<Uri>`, real impl later)
- [ ] `reeda-ui/src/android/mod.rs` — intent data reader stub
- [ ] `reeda-ui/src/android/mod.rs` — permission request stub
- [ ] `reeda-tts` — keep `platform-desktop` stub as-is for M0 (real JNI bridge in M5)

### M0.7 reeda-epub / reeda-pdf / reeda-search / reeda-tts (skeleton keep)

- [ ] Verify all four engine crate skeletons compile with `cargo check`
- [ ] Confirm `reeda-tts` feature flags (`platform-desktop` / `platform-android`) gate correctly
- [ ] No real implementation — these land in M1/M4/M5/M6 respectively

### M0.8 Tests

- [ ] `reeda-core`: unit tests for models (serde, defaults, CfiRange construction)
- [ ] `reeda-core`: unit tests for storage (migration, CRUD, settings get/set)
- [ ] `reeda-core`: unit tests for App dispatch (OpenBook → state change → snapshot)
- [ ] `reeda-ui`: verify `cargo check -p reeda-ui` on host
- [ ] `cargo test --workspace` passes on host (CI green)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --check` passes

### M0.9 Documentation updates

- [ ] Update CHANGELOG.md with M0 additions
- [ ] Update README.md status table (M0 → In Progress / Done)
- [ ] Verify all cross-references in docs/ are valid

---

## M1 — EPUB reader core (3–4 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M1.

## M2 — Library & metadata (2 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M2.

## M3 — Highlighting & notes (3 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M3.

## M4 — Full-text search (2 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M4.

## M5 — Read aloud / TTS (3 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M5.

## M6 — PDF support (3 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M6.

## M7 — Hardening & ship (3 weeks) — _not started_

See [ROADMAP.md](docs/ROADMAP.md) §M7.

---

## Doc inventory

| Document | Status | Last updated |
|----------|--------|-------------|
| [PRD](docs/PRD.md) | draft | 2026-08-17 |
| [Architecture](docs/ARCHITECTURE.md) | draft | 2026-08-17 |
| [Technical Design](docs/TECHNICAL_DESIGN.md) | draft | 2026-08-17 |
| [Roadmap](docs/ROADMAP.md) | draft | 2026-08-17 |
| [Data Model](docs/DATA_MODEL.md) | draft | 2026-08-17 |
| [EPUB Spec](docs/EPUB_SPEC.md) | draft | 2026-08-17 |
| [PDF Spec](docs/PDF_SPEC.md) | draft | 2026-08-17 |
| [TTS Spec](docs/TTS_SPEC.md) | draft | 2026-08-17 |
| [Search Spec](docs/SEARCH_SPEC.md) | draft | 2026-08-17 |
| [Highlights Spec](docs/HIGHLIGHTS_SPEC.md) | draft | 2026-08-17 |
| [UI/UX Design](docs/UI_UX.md) | draft | 2026-08-17 |
| [Platform / Build](docs/PLATFORM.md) | draft | 2026-08-17 |
| [CI/CD](docs/BUILD_CI.md) | draft | 2026-08-17 |
| [Testing Strategy](docs/TESTING.md) | draft | 2026-08-17 |
| [Performance](docs/PERFORMANCE.md) | draft | 2026-08-17 |
| [Accessibility](docs/ACCESSIBILITY.md) | draft | 2026-08-17 |
| [Localization](docs/LOCALIZATION.md) | draft | 2026-08-17 |
| [Security / DRM](docs/DRM_SECURITY.md) | draft | 2026-08-17 |
| [Release Guide](docs/RELEASE.md) | draft | 2026-08-17 |
| [Contributing](docs/CONTRIBUTING.md) | draft | 2026-08-17 |
| [Changelog](docs/CHANGELOG.md) | draft | 2026-08-17 |
| [Glossary](docs/GLOSSARY.md) | active | 2026-08-17 |
| [ADRs](docs/ADR.md) | active | 2026-08-17 |

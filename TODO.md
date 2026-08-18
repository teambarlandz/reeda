# TODO — Reeda

> Master planning index. Every task below is traced to a milestone in
> [ROADMAP.md](docs/ROADMAP.md) and a spec in `docs/`. Statuses:
> `[ ]` = pending, `[~]` = in progress, `[x]` = done, `[-]` = cancelled.

---

## M0 — Foundations ✅

Done. See git log for full history.

---

## M1 — EPUB reader core ✅

**Exit criterion:** read a full Gutenberg EPUB, turn pages, change font/size/theme, resume exactly where you left off.

- [x] **M1.1** reeda-epub — Container/ZIP + OPF parsing (`container.rs`, `opf.rs`)
- [x] **M1.2** reeda-epub — Nav/TOC parsing (`nav.rs`)
- [x] **M1.3** reeda-epub — XHTML → document model (`xhtml.rs`, `document.rs`)
- [x] **M1.4** reeda-epub — CSS subset engine — deferred to M2 (basic reading works without styled CSS)
- [x] **M1.5** reeda-epub — CFI position model (`cfi.rs`)
- [x] **M1.6** reeda-epub — Paginator (`paginator.rs`)
- [x] **M1.7** reeda-core — Wire EPUB open into App dispatch (`reader.rs`)
- [x] **M1.8** reeda-ui — Reader screen integration (Slint bindings + main.rs)
- [x] **M1.9** Tests — 84 total (40 reeda-core, 40 reeda-epub, 4 others)
- [x] **M1.10** Update TODO.md + CHANGELOG.md + commit/push

---

## M2 — Library & metadata ✅

**Exit criterion:** import 10 books, covers show, recent/continuar correct, delete works.

- [x] **M2.1** Import pipeline: file storage + SHA-256 dedup (`store.rs`, BookStore)
- [x] **M2.2** Library grid Slint UI (`BookCard.slint`, LibraryScreen model, main.rs)
- [x] **M2.3** Cover extraction from EPUB (`extract_cover_bytes`, cover stored on import)
- [x] **M2.4** Progress save/restore (page index persisted on turn/close, restored on open)
- [x] **M2.5** Metadata editing (`EditMetadata`, `MetadataDialog`, persists to DB)
- [x] **M2.6** Settings screen v1 (`SettingsScreen.slint`, theme picker, font size, line height)
- [x] **M2.7** Tests + integration — SQLite persistence wired into App (books, chapters,
  position, metadata, settings); 103 tests total (58 core, 42 epub, 3 others)
- [x] **M2.8** Update TODO.md + CHANGELOG.md + README.md + commit/push

**Notes / deferred:** cover rendering as image in grid (placeholder initial shown —
cover_path stored, image display pending M3); "Continue reading" section and sort
buttons (M2.2) not yet in UI; CSS subset engine (M1.4) still deferred.

---

## M3 — Highlighting & notes (3 weeks)

**Exit criterion:** highlight a passage, kill the app, relaunch → highlight + note
intact at correct position with different font size.

**Plan (task order):**

### M3.1 — Selection + intersection engine (core)
`selection.rs` in reeda-core: `locator_of_global_block()` (global block → spine/block
Locator), `LocatorRange` from (block, char_start, char_end), CFI anchoring via
`cfi::Cfi::from_locator`, and `intersect_range_with_page()` — clip a CfiRange to a
page's visible segments for rendering. Tests: intersection math, cross-block ranges,
page-clipping, CFI round-trips.

### M3.2 — Highlight rendering in reader (UI)
`PageContent` gains `highlight_segments` (block, char_start, char_end, color, has_note).
ReaderScreen renders per-line segments with translucent colored backgrounds + underline
(HIGHLIGHTS_SPEC §3). Tap highlight → popover: edit color (4 swatches) / delete.
App commands wired: AddHighlight/EditHighlight/DeleteAnnotation already exist — connect
to UI callbacks + snapshot.

### M3.3 — Notes attach + notes list screen
`AddNote` command wired end-to-end (attach to highlight / standalone). New NotesScreen:
per-book list of highlights+notes grouped by chapter, snippet text, color chip, date;
tap → jump to location (HIL-06).

### M3.4 — Bookmarks + bookmarks list
Chrome ribbon toggle button; icon state derived from current page CFI (filled when page
start CFI equals bookmark). Bookmarks list screen: tap → jump, delete. Uses existing
`ToggleBookmark`.

### M3.5 — Export highlights/notes (Markdown)
`export_markdown(book)` in core per HIGHLIGHTS_SPEC §4 format. Android: share sheet
(ACTION_SEND, text/plain); desktop: write `books/<id>/annotations.md` + print path.

### M3.6 — Persistence wiring + position invariance + docs
Wire annotation CRUD to SQLite in App commands (insert/soft-delete/list on open).
Font-size change invariance test (HIL-08): same CFI → same geometry per layout (golden
tests). Restart persistence integration test. Update TODO.md + CHANGELOG.md + README.md
+ commit/push.

---

## M4 — Full-text search (2 weeks) — _not started_
## M5 — Read aloud / TTS (3 weeks) — _not started_
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_

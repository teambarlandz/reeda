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

- [~] **M3.1** reeda-epub — Selection + intersection engine (`selection.rs`):
  - `GlobalRange` (block_start, char_start, block_end, char_end over global block
    sequence) + `is_valid()`
  - `to_cfi()` / `from_cfi()` — CFI anchoring via `cfi::Cfi` (round-trip, orphaned
    CFI → None)
  - `intersect_range_with_page()` → `Vec<ClippedSegment>` (page char/block clipping)
  - Tests: intersection math, cross-block, page-clipping, CFI round-trips
  - Fix `find_page_for_cfi` (reader.rs) to use global `block_index` (not spine_index)
- [ ] **M3.2** reeda-ui — Highlight rendering in reader:
  - `PageContent.highlight_segments` (block, char_start, char_end, color, has_note)
  - ReaderScreen: per-line segments, translucent colored background + underline
    (HIGHLIGHTS_SPEC §3)
  - Tap highlight → popover: edit color (4 swatches) / delete
  - Wire AddHighlight/EditHighlight/DeleteAnnotation commands to UI callbacks +
    snapshot
- [ ] **M3.3** reeda-core + reeda-ui — Notes + notes list:
  - `AddNote` wired end-to-end (attach to highlight / standalone)
  - NotesScreen: per-book list of highlights+notes grouped by chapter, snippet,
    color chip, date; tap → jump (HIL-06)
- [ ] **M3.4** reeda-ui — Bookmarks:
  - Chrome ribbon toggle button; icon state from page-start CFI (filled when match)
  - Bookmarks list screen: tap → jump, delete (uses `ToggleBookmark`)
- [ ] **M3.5** reeda-core — Export highlights/notes (Markdown):
  - `export_markdown(book)` per HIGHLIGHTS_SPEC §4 format
  - Android share sheet (ACTION_SEND, text/plain); desktop writes
    `books/<id>/annotations.md`
- [ ] **M3.6** reeda-core — Persistence + invariance + docs:
  - Wire annotation CRUD to SQLite in App commands (insert/soft-delete/list on open)
  - HIL-08 font-size invariance golden tests (same CFI → same geometry per layout)
  - Restart persistence integration test
  - Update TODO.md + CHANGELOG.md + README.md + commit/push

---

## M4 — Full-text search (2 weeks) — _not started_
## M5 — Read aloud / TTS (3 weeks) — _not started_
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_

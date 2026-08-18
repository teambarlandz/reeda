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

## M2 — Library & metadata

**Exit criterion:** import 10 books, covers show, recent/continuar correct, delete works.

**Plan (task order):**

### M2.1 — Import pipeline: file storage + dedup
Copy imported EPUB bytes into `books/<id>/book.epub`, SHA-256 dedup check
(LIB-10), error classification (corrupt zip / missing OPF / unsupported version).
`App::import_book` → stage → hash → check → copy → parse metadata → insert DB
→ refresh library. Currently `import_from_bytes` keeps everything in memory;
this task moves to persistent file storage.

### M2.2 — Library grid Slint UI
LibraryScreen.slint: book cards with cover thumbnail, title, author, progress
bar. Sort buttons (recent / alphabetical). "Continue reading" section (last 8
opened). Empty-state remains for zero books. Tap card → OpenBook. Long-press →
context menu (edit metadata / delete). Import FAB (floating action button).

### M2.3 — Cover extraction from EPUB
During import, look for `<meta name="cover" content="..."/>` in OPF metadata,
resolve the manifest item to an image path, extract + decode to RGBA → save as
`covers/<id>.webp` (using image crate). Fallback: first `<img>` in first
chapter. No-cover: show initial letter placeholder.

### M2.4 — Progress save/restore via storage
On page turn, debounce 5s → `update_book_position(book_id, cfi, progress_pct)`
in SQLite. On `OpenBook`, read `last_position` → CFI → page lookup → restore
current_page. Persist on `CloseBook`, `onPause`, and chapter change.

### M2.5 — Metadata editing (title/author override)
`Command::UpdateBookMetadata { book_id, title, author }` → update DB + in-memory.
Slint: edit dialog triggered from library card long-press → pop up text fields
→ save. `StateSnapshot.library` reflects updated title/author.

### M2.6 — Settings screen v1
New `SettingsScreen.slint` accessible from library top-bar gear icon.
Theme picker (Light / Sepia / Dark), typography defaults (font size, line
height, margin, justify toggle). `Command::UpdateSettings` → persist to SQLite
`settings` table → apply theme to Slint window.

### M2.7 — Tests + integration
- Import pipeline: file copied to books/, dedup detects duplicate sha256
- Library grid: snapshot.library populated after import
- Cover extraction: cover_path set, file exists (test fixture)
- Progress: open → turn page → close → reopen → same page
- Metadata editing: title change persists in DB
- Settings: theme change persists, load_settings round-trips
- Clippy clean, fmt clean, 100+ total tests

### M2.8 — Update TODO.md + CHANGELOG.md + commit/push

---

## M3 — Highlighting & notes (3 weeks) — _not started_
## M4 — Full-text search (2 weeks) — _not started_
## M5 — Read aloud / TTS (3 weeks) — _not started_
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_

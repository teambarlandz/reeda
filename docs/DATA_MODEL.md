# Data Model & Storage Specification — Reeda

> Status: draft · Version: 0.2 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-core::storage` (SQLite via rusqlite, ADR-004).

## 1. Conventions

- **SQLite** single file `reeda.db`, **WAL** mode, `synchronous=NORMAL`,
  `foreign_keys=ON`, `busy_timeout=5000`.
- Primary keys: `TEXT` UUID v4 (sync-ready, ADR-004). Timestamps:
  RFC3339 UTC (`TEXT`), maintained by app code.
- **LWW sync fields** on all syncable tables: `updated_at`, `deleted_at`
  (soft delete). V2 sync uses these; v1 writes them anyway.
- Migrations: `reeda-core/migrations/NNNN_name.sql` embedded via
  `include_str!`, applied in order, recorded in `schema_migrations`.
- Writer: single dedicated thread; readers: pool per thread
  (TECHNICAL_DESIGN §4). All statements prepared at startup.

## 2. Tables

### 2.1 `books` — library records
| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | uuid |
| title | TEXT | from OPF, editable (MET-03) |
| author | TEXT | dc:creator (join " | ") |
| format | TEXT | `epub` \| `pdf` (CHECK) |
| file_path | TEXT | `books/<id>/book.<ext>` |
| cover_path | TEXT NULL | `covers/<id>.webp` |
| sha256 | TEXT | dedupe key (LIB-10), unique index |
| language | TEXT NULL | dc:language |
| publisher | TEXT NULL | |
| description | TEXT NULL | dc:description |
| published_at | TEXT NULL | dc:date |
| imported_at | TEXT | |
| last_opened_at | TEXT NULL | |
| last_position | TEXT NULL | Cfi (EPUB) / page (PDF) |
| progress_pct | REAL | 0..1 derived, stored |
| is_pdf_outline_loaded | INT | pdf outline cache flag |
| updated_at | TEXT | LWW |
| deleted_at | TEXT NULL | soft delete |

### 2.2 `chapters` — spine items (EPUB)
| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | uuid |
| book_id | TEXT FK | |
| spine_index | INT | order |
| title | TEXT | from nav/ncx |
| href | TEXT | resolved path |
| file_hash | TEXT | content hash for structure-drift detection |
| char_count | INT | |
| updated_at | TEXT | LWW |

### 2.3 `annotations` — highlights, notes, bookmarks
| Column | Type | Notes |
|--------|------|-------|
| id | TEXT PK | uuid |
| book_id | TEXT FK | |
| kind | TEXT | `highlight` \| `note` \| `bookmark` \| `pdf_highlight` (CHECK) |
| cfi | TEXT NULL | CfiRange (EPUB kinds) |
| page | INT NULL | PDF kind |
| rect | TEXT NULL | JSON [x,y,w,h] percent (PDF kind) |
| color | TEXT | yellow/green/blue/pink/null |
| text | TEXT NULL | note body / label |
| snippet | TEXT NULL | denormalized selection text (list screens, search) |
| sort_key | TEXT | chapter title + offset (list ordering) |
| created_at | TEXT | |
| updated_at | TEXT | LWW |
| deleted_at | TEXT NULL | |
| INDEX (book_id, kind), INDEX (book_id, sort_key) | | |

### 2.4 `bookshelves` + `bookshelf_books` (P1, schema now)
Collections: uuid PK, name, created/updated/deleted; join table
`(shelf_id, book_id, position)`. Sync-ready like others.

### 2.5 `settings` — KV store
`key TEXT PK, value TEXT` (JSON). Keys: `theme`, `font_family`,
`font_size_pt`, `line_height`, `margin`, `justify`, `tts_speed`,
`tts_pitch`, `tts_wakelock`, `tap_zones_layout`, `locale`,
`index_meta_version`, `first_run_done`, … Schema-versioned defaults live in
code (`SettingsSchema` struct + `serde`), persisted whole (typed snapshot).

### 2.6 `schema_migrations`
`version INT PK, applied_at TEXT`.

## 3. File layout (app-private `filesDir`)

```
reeda.db
books/<book_id>/
  book.epub | book.pdf     # canonical copy (FR-04)
  extracted/               # EPUB: container contents
  resources/               # images/fonts (EPUB_SPEC §4)
  annotations.md           # last export snapshot (HIL-06)
covers/<book_id>.webp
index/                     # Tantivy (SEARCH_SPEC §2)
cache/pages/               # pagination layout cache (LRU, killable)
cache/pdf/                 # raster cache (LRU ≤ 128 MB, killable)
tmp/                       # import staging; cleaned on failure
```

- Nothing in `cache/` is precious — all rebuildable.
- Backup (SET-02): zip of `reeda.db` + `books/` (+ index excluded).

## 4. Import transaction (EPUB) — crash-safe

1. Stage to `tmp/<uuid>/` (copy → unzip → validate).
2. DB: `BEGIN IMMEDIATE`; insert `books`; insert `chapters`; commit.
3. Move staged files into `books/<id>/` (rename, atomic per-file);
   `tmp` entry deleted on success, GC'd on startup if stale.
4. Index job enqueued (SEARCH_SPEC §4). Any step 2+ failure → rollback +
   file cleanup + `ImportError` classification surfaced to UI.

## 5. Progress persistence (FR-03)

- In-memory: current page/CFI updated per page turn (UI).
- Flush points: 5 s ticker while reading, `onPause`, `onStop`, chapter
  change, TTS page-advance. WAL ensures < 100 ms writes for a progress row.

## 6. Settings versioning

`settings` snapshot carries `schema_version`; migration path: load → bump →
defaults merge (forward-compatible, drops unknown keys with log). No raw
JSON in UI; typed `AppSettings` via serde.

## 7. Queries of note (prepared at startup)

- Library grid: `SELECT * FROM books WHERE deleted_at IS NULL ORDER BY
  last_opened_at DESC` (+sort variants, LIB-08).
- Continuar: `last_opened_at IS NOT NULL … LIMIT 8`.
- Annotations for book: by `book_id`, not deleted, `sort_key` ASC.
- Highlight lookup for render: per chapter+page via `cfi` prefix range scan
  (indexed).
- Search results: from Tantivy; book titles via join (SEARCH_SPEC §5).

## 8. Data integrity & tests

- `PRAGMA integrity_check` on startup (once per 7 days) → report.
- Unit tests: migration 0→N chain on empty DB + upgrade from each prior
  version with fixture data; LWW update semantics; import rollback
  injection tests (fail at staged steps); settings bump/drop behavior.
- Backup/restore round-trip test (device + host).

-- Initial schema: all core tables for Reeda v0.1 (DATA_MODEL.md §2).
-- Applied as the first migration on a fresh database.

PRAGMA foreign_keys = ON;

-- ══════════════════════════════════════════════════════════════════════
-- books — library records
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS books (
    id                      TEXT PRIMARY KEY,
    title                   TEXT NOT NULL,
    author                  TEXT,
    format                  TEXT NOT NULL CHECK (format IN ('epub', 'pdf')),
    file_path               TEXT NOT NULL,
    cover_path              TEXT,
    sha256                  TEXT NOT NULL,
    language                TEXT,
    publisher               TEXT,
    description             TEXT,
    published_at            TEXT,
    imported_at             TEXT NOT NULL,
    last_opened_at          TEXT,
    last_position           TEXT,
    progress_pct            REAL NOT NULL DEFAULT 0.0,
    is_pdf_outline_loaded   INTEGER NOT NULL DEFAULT 0,
    updated_at              TEXT NOT NULL,
    deleted_at              TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_books_sha256 ON books(sha256)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_books_last_opened ON books(last_opened_at DESC)
    WHERE deleted_at IS NULL;

-- ══════════════════════════════════════════════════════════════════════
-- chapters — EPUB spine items
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS chapters (
    id              TEXT PRIMARY KEY,
    book_id         TEXT NOT NULL REFERENCES books(id),
    spine_index     INTEGER NOT NULL,
    title           TEXT NOT NULL DEFAULT '',
    href            TEXT NOT NULL,
    file_hash       TEXT NOT NULL DEFAULT '',
    char_count      INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chapters_book ON chapters(book_id, spine_index);

-- ══════════════════════════════════════════════════════════════════════
-- annotations — highlights, notes, bookmarks
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS annotations (
    id              TEXT PRIMARY KEY,
    book_id         TEXT NOT NULL REFERENCES books(id),
    kind            TEXT NOT NULL CHECK (kind IN ('highlight', 'note', 'bookmark', 'pdf_highlight')),
    cfi             TEXT,
    page            INTEGER,
    rect            TEXT,
    color           TEXT,
    text            TEXT,
    snippet         TEXT,
    sort_key        TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_annotations_book_kind ON annotations(book_id, kind)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_annotations_book_sort ON annotations(book_id, sort_key)
    WHERE deleted_at IS NULL;

-- ══════════════════════════════════════════════════════════════════════
-- bookshelves + bookshelf_books — user-defined collections (schema now,
-- UI in M2)
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS bookshelves (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT
);

CREATE TABLE IF NOT EXISTS bookshelf_books (
    shelf_id    TEXT NOT NULL REFERENCES bookshelves(id),
    book_id     TEXT NOT NULL REFERENCES books(id),
    position    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (shelf_id, book_id)
);

-- ══════════════════════════════════════════════════════════════════════
-- settings — key/value store (JSON values)
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS settings (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

-- ══════════════════════════════════════════════════════════════════════
-- schema_migrations — tracks applied migrations
-- ══════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL
);

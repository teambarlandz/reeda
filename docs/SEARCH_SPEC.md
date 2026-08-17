# Full-Text Search Specification — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-search` (Tantivy — ADR-009). Requirements: PRD §3.5.

## 1. Scope

- **Library-wide full-text search** over EPUB text content (SEA-01) — the
  v1 feature. PDF search (PDF-06) stays in PDFium at page level (P2).
- Search targets: text content of all spine items, excluding footnotes,
  captions (option), table of contents entries (dedup noise), and
  boilerplate per book (settings later).

## 2. Index schema (Tantivy)

```
doc_id        u64   (internal primary)
book_id       str   (facet/filter)
spine_index   u32
block_index   u32
char_offset   u32   → locator reconstruction (open-at-match, SEA-03)
title         text  (boosted 2.0)
body          text  (default field; analyzer: language-aware, see §3)
chapter_title str   (display grouping)
```

- Segment-per-book approach rejected (many segments, heavy deletes) →
  **single writer, per-document delete by `book_id` term** on book deletion;
  commits on import-batch end and on app pause (debounced).
- Index path: `context.filesDir/index/` (derived data — rebuildable,
  excluded from backups).

## 3. Analysis & languages

- Analyzer: lowercase + unicode segmentation; stopwords per language
  (en first); stemming via `lindera` (rust-stemmers fallback) for en, fr,
  de, es, pt, nl, ru, ar (first languages, extendable).
- Per-document language from OPF `dc:language`; fallback auto-detect
  (language sniffing crate) → analyzer selection per document.
- CJK/Thai: tokenized by unicode segmentation (no space splitting).
- Diacritic folding: `NFD` + strip (option, default on) to make "café" ⇄
  "cafe" match.

## 4. Index lifecycle

| Event | Action |
|-------|--------|
| Book imported | Enqueue `index(book_id)`; background worker (debounce 2 s) |
| Book deleted | `delete_by_term(book_id)` + commit |
| Book re-imported (dedupe-update) | Replace documents for book_id |
| Font/metadata change | No reindex (index is content-only) |
| App start | Open index; if missing/corrupt → rebuild from library (progress event) |
| App upgrade (schema bump) | Rebuild once; version stamped in index meta file |

- Indexing throughput target: ≥ 2 MB text/min on mid-range device;
  builds run only when app is foreground or idle (battery-friendly).

## 5. Query behavior

- Query: tokenized + phrase support (`"exact phrase"`), AND by default;
  prefix/substring search on CJK; typo tolerance **P2** (fuzzy/levenshtein
  on terms ≤ 8 chars).
- Ranking: BM25 over `body` with `title` boost; results capped 200.
- Result payload: `(book_id, title, chapter_title, snippet with highlighted
  term, locator CfiRange of first hit)`.
- **Open-at-match** (SEA-03): locator → reader jump + term highlight
  (SEA-04): re-use annotation-style CFI highlight, transient (not stored).
- Search-within-book (SEA-05): same index, `book_id` filter + same ranking.

## 6. UI (with UI_UX.md)

- Entry: search icon on Library → full-screen search; query-as-you-type
  (debounce 250 ms); results grouped by book with chapter headings; recent
  searches (P1); empty/error states defined.
- In-reader search: chrome search icon → overlay panel; hits list; arrows
  prev/next hit with transient highlight.

## 7. Performance budgets

- 50-book fixture library (≈ 40 MB text): query p95 < 1 s (incl. first-hit
  render). Index build ≤ 10 s per 100 books, never blocking UI.
- Memory: Tantivy reader keeps hot segments; LRU by last-use, budget
  ≤ 64 MB.

## 8. Fixtures & tests

- Fixtures: multi-language corpus books, long books (1 M words), books with
  diacritics/emoji/ligatures, empty books.
- Tests: relevance sanity (expected top-N for queries), locator correctness
  (hit → open → text at position equals snippet), rebuild idempotence,
  delete lifecycle, phrase/prefix behavior, corpus-level golden rankings.

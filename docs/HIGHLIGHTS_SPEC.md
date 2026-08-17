# Highlights, Notes & Bookmarks Specification — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-core` (storage) + `reeda-epub` (CFI ranges) +
> `reeda-ui` (gestures). Requirements: PRD §3.4.

## 1. Concepts

- **Highlight** — text range (`CfiRange`) + color (4: yellow, green, blue,
  pink) + style (highlight / underline[P2]) + optional note.
- **Note** — free text attached to a highlight (HIL-04). One per highlight
  in v1 (multiple = P2).
- **Bookmark** — position marker (`Cfi`) with optional label; not
  text-anchored (HIL-05).
- **Core invariant (HIL-08):** all persisted anchors are CFI; rendering
  recomputes intersections on every layout. Never store page indices.

## 2. Text selection (EPUB)

1. Long-press on a page → Slint gesture begins selection; drag moves the
   caret; second handle selects end (or tap word = select word).
2. Word snapping via paginator word metadata (block/char offsets).
3. Selection popover (when finger lifts): **Highlight ▸ color, Add note,
   Copy, Dictionary[P2]**.
4. Selection bounds clamp to block boundaries; cross-block selection spans
   chapters → auto-expand to chapter start/end (documented).

## 3. Highlight interaction (HIL-07)

- Tap a rendered highlight → popover: edit color (4 swatches), add/edit
  note, delete (confirm), share.
- Rendered highlight: colored background over the CFI range (alpha 0.25)
  + thin underline in highlight color; note marker (small dot) if present.
- Only highlights intersecting the current page are drawn (recomputed per
  layout).

## 4. Notes & highlights list (HIL-06)

- Screen: per-book list of highlights+notes grouped by chapter, with snippet
  text, color chip, date; tap → jump to location.
- Export: **Markdown** (default) and plain text via Android share sheet
  (`ACTION_SEND`, text/plain). Format:
  ```markdown
  # Book Title — Highlights & Notes
  ## Chapter 3
  > "Passage text…"  *[p. 42]*
  > **Note:** my thought
  ```
- Export covers `books/<id>/annotations.md` snapshot; no styling loss.

## 5. Bookmarks

- Toggle button in chrome (ribbon icon). Stored `(book_id, cfi, label?)`.
- Bookmarks list screen: tap → jump; swipe → delete.
- Bookmark icon state derived from current page CFI (page-start CFI equal →
  filled).

## 6. Anchoring algorithm (EPUB)

```
1. Selection → LocatorRange { spine, block_start, char_start, block_end, char_end }
2. Serialize → CfiRange via cfi::to_cfi(range)
3. Persist (annotations table)
4. On any render(pages, typography):
   for each page, for each annotation in book:
     intersect(cfi_range, page_start_cfi, page_end_cfi)
     → clipped render range (block, char offsets)
5. Idempotent: same CFI → same geometry per layout (golden tests)
```

Clamping rules (EPUB_SPEC §7): CFI inside a table/img → nearest paragraph
boundary; CFI at end-of-chapter → last block; orphaned CFI (book re-parsed
with changed structure) → find nearest block by spine+position heuristics,
or drop with report (never crash). Tested via fixtures with edited books
(structure drift simulation).

## 7. PDF highlights (P2 — see PDF_SPEC §6)

- Overlay model: `(book_id, page, rect_percent: [x,y,w,h], color, note)`,
  stored in our DB; rendered as a translucent Slint Rectangle over the
  rasterized page. No file mutation.

## 8. Data model

See DATA_MODEL.md §3 (`annotations` table) — schema supports: uuid PK,
book_id FK, kind (highlight/note/bookmark), cfi/range JSON, color, label,
created/updated (LWW), deleted_at (soft delete, sync-ready).

## 9. Testing

- Unit: CFI round-trips, intersection math (property tests: random ranges
  on fixtures), clamp rules, export formatting golden.
- Integration: highlight → restart → same geometry (device test); font-size
  change invariance (HIL-08); delete/undo flows.
- UI: gesture tests on emulator (long-press coords, drag) with seeded
  positions.

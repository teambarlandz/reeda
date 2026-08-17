# EPUB Specification — Reeda

> Status: draft · Version: 0.2 · Owner: @teambarlandz · Last updated: 2026-08-17
> Defines exactly what EPUB support means in Reeda: parsing, CSS subset,
> pagination, CFI, and conformance tests. Implementation: `reeda-epub`.

## 1. Supported formats

- **EPUB 2.0.1** (OEBPS) and **EPUB 3.0 / 3.1 / 3.2** (`container.xml`,
  `OPF`/package-doc). Legacy/obscure variants best-effort: unknown package
  versions produce `ImportError::UnsupportedVersion` with a clear message.
- **DRM-free only** (ADR-010): encrypted `.acsm`/Adobe-DRM or any
  obfuscation → clear error at import.
- Encodings: UTF-8 (required by spec); non-UTF-8 files → strict error
  (counted, reported, not silently mangled).

## 2. Package parsing (container → DocumentModel)

```
EPUB = ZIP
 ├─ mimetype (first entry, must be "application/epub+zip", stored)
 ├─ META-INF/container.xml → rootfile path
 └─ <rootfile> (OPF)
     ├─ metadata: dc:title, dc:creator, dc:language, dc:identifier,
     │            dc:publisher, dc:date, dc:description, meta:cover
     ├─ manifest: id → (href, media-type)  [itemref validation]
     ├─ spine: ordered reading order, optional linear="no"
     └─ guide (EPUB2) / landmarks (EPUB3)
 └─ nav: EPUB3 nav.xhtml (epub:type="toc") | EPUB2 toc.ncx
```

- Manifest media-type whitelist for content: `application/xhtml+xml`,
  `text/html` (converted), images (png/jpeg/gif/webp/svg — svg best-effort),
  `audio/*` (P2), fonts (`application/font-*`, `font/*` — subsetting P2).
- All paths resolved against the OPF base dir, URL-decoded, normalized.
- **Zip-slip guard**: every entry path must, after normalization, stay under
  the container root (reject `..`, absolute, drive-letter, backslash-escaped).
- **Bomb guard**: total uncompressed size cap (default 512 MB) and per-entry
  ratio cap (100×); exceeding → `ImportError::TooLarge`.

## 3. XHTML → document model

- Parse with `html5ever` (HTML5 semantics — EPUB XHTML is HTML5-compliant),
  then map to a typed tree:
  - Block nodes: `p, h1-h6, div, blockquote, li, pre, table(simplified)`
  - Inline: `em, strong, i, b, u, s, sub, sup, code, span, a, br`
  - Special: `img, figure+figcaption, hr, footnote (epub:type="footnote")`
  - Ignored (with debug warn): `script, style, form, iframe, video`
- HTML entities, `xml:lang`, `dir`, namespaces handled by html5ever.
- Links: internal `#cfi` / href-based cross-references resolved to CFI
  targets at load time (EPR-11).
- Footnotes (EPR-09, P2): captured as popover payloads, rendered inline at
  page end (approximation) in v1.

## 4. Images & resources

- Images are extracted to `books/<id>/resources/` at import (mimetype
  validated, magic-byte verified, max dimension 16k×16k).
- CSS-referenced resources (backgrounds, fonts) resolved relative to their
  CSS file. Unknown/missing → broken-image placeholder + warn.

## 5. CSS subset (ADR-007) — supported properties

Cascade model: author CSS (stylesheets linked in spine item + `<style>`
blocks) over user-agent defaults over **user typography settings**
(font-family/size from the reader controls). Media queries: only
`all`/`screen`; `print` ignored. No @page, no floats (fallback to block),
no position/transform, no animations, no tables beyond simple grid.

| Group | Properties |
|-------|-----------|
| Font | `font-family` (mapped to Slint font stack or bundled fonts), `font-size` (px, %, em, rem, keywords), `font-weight`, `font-style`, `font-variant` (small-caps best-effort), `text-transform` (uppercase/lowercase/capitalize) |
| Text | `color`, `background-color` (text-level; page bg from theme), `text-align`, `text-decoration` (u/line-through, no blink), `line-height` (number, px, %), `letter-spacing`, `text-indent`, `word-spacing`, `white-space` (normal/pre/pre-wrap — no nowrap) |
| Layout | `margin` (block-level, incl. `margin: 0 auto` centering), `padding` (block), `width`/`max-width` (block), `vertical-align` (sub/super/baseline on inlines), `display` (none/block/inline) |
| Images | `width`, `height` (px/%), `object-fit` (cover/contain), `float` → inline-block approximation |
| Page | `page-break-before/after` → chapter boundaries only (best-effort) |

**Deterministic rules**:
1. Only inline + embedded `<style>` + linked `.css` (same container) apply.
2. `!important` honored; specificity per CSS2.1 with html5 parsing.
3. Unknown property/value → ignored + one-time debug log (never affects
   layout stability).
4. `rem` rooted at reader font-size (document root font-size override → user
   typography).
5. No JS, no `content:`, no counters, no custom fonts in v1 (P2).

## 6. Pagination (deterministic)

- Inputs: `DocumentModel` slice + `PageLayout { width, height, margin,
  Typography }`. Output: `Pages` — ordered list of page descriptors
  (`first_block`, `first_char`, `last_char`, `image_refs`, `link_targets`,
  `footnote_refs`, `highlight_intersections`).
- Algorithm: block-aware greedy fill. Blocks ≥ page-height (tables, big
  images) split across pages. Orphan/widow control (2 lines) — soft rule.
- Line measurement delegated to Slint's text layout at the same
  width/typography as the reader (single source of truth for metrics).
  Paginator is a worker-pool job; results cached by
  `(book_id, layout_hash)` with an LRU of ~8 layouts.
- **Invariant:** identical inputs → identical `Pages` (golden tests).

## 7. CFI positions (ADR-005)

- Canonical locator: EPUB CFI. We produce and consume:
  - `epubcfi(/6/10[chapter.xhtml]!/4/2/10)` — paragraph-level location.
  - Range form for annotations: `epubcfi(...,/4/2/10, /4/2/52)`.
- Mapping to our model: spine index + node path + character offset. We
  normalize CFI to our `Locator { spine_index, block_index, char_offset }`
  internally; CFI strings are the persistence + interchange format.
- Edge cases: CFI pointing into table/img → clamp to nearest paragraph
  boundary (documented, tested).
- Progress: store CFI of current page-start (EPR-12). Page % displayed to the
  user is computed from `page_index / total_pages`.

## 8. Conformance & test fixtures

- Test set (`reeda-epub/fixtures`): Gutenberg-sourced sample books, EPUB 2/3
  samples from IDPF/W3C (epub3-samples), adversarial cases (zip-slip,
  bombs, malformed XML, huge entities, missing OPF, non-UTF8, 0-page
  chapters, bidirectional text).
- Golden pagination tests: fixed fixtures → exact CFI-per-page tables.
- Fuzzing: `cargo-fuzz` on container/xhtml/css entry points (CI nightly).
- Readahead check: parse every fixture, assert `ImportReport` counts.

## 9. Related

- [DATA_MODEL](DATA_MODEL.md) (storage of DocumentModel-derived data)
- [TECHNICAL_DESIGN](TECHNICAL_DESIGN.md) §2.2 (paginator API)
- [HIGHLIGHTS_SPEC](HIGHLIGHTS_SPEC.md) §6 (CFI ranges)

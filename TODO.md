# TODO — Reeda

> Master planning index. Every task below is traced to a milestone in
> [ROADMAP.md](docs/ROADMAP.md) and a spec in `docs/`. Statuses:
> `[ ]` = pending, `[~]` = in progress, `[x]` = done, `[-]` = cancelled.

---

## M0 — Foundations ✅

Done. See git log for full history.

---

## M1 — EPUB reader core (3–4 weeks)

**Exit criterion:** read a full Gutenberg EPUB, turn pages, change font/size/theme, resume exactly where you left off.

- [~] **M1.1** reeda-epub — Container/ZIP + OPF parsing (`container.rs`, `opf.rs`)
  - Zip open with zip-slip guard + decompression bomb guard
  - `META-INF/container.xml` → rootfile path
  - OPF metadata (dc:title, dc:creator, dc:language, dc:identifier, dc:publisher, dc:date, dc:description, meta:cover)
  - OPF manifest (id → href + media-type, validation)
  - OPF spine (ordered reading order, linear="no")
- [ ] **M1.2** reeda-epub — Nav/TOC parsing (`nav.rs`)
  - EPUB3 `nav.xhtml` (epub:type="toc") → `TableOfContents`
  - EPUB2 `toc.ncx` → `TableOfContents`
  - Unified TOC tree structure
- [ ] **M1.3** reeda-epub — XHTML → document model (`xhtml.rs`, `document.rs`)
  - html5ever parsing of EPUB XHTML
  - Block nodes: p, h1-h6, div, blockquote, li, pre
  - Inline: em, strong, i, b, u, sub, sup, code, span, a, br
  - Special: img, figure, hr
  - Ignored: script, style, form, iframe, video
  - `DocumentModel` (chapters, blocks, images, links)
- [ ] **M1.4** reeda-epub — CSS subset engine (`css.rs`, `style.rs`)
  - Parse inline + embedded `<style>` + linked `.css`
  - Supported: font-size, font-weight, font-style, color, text-align, text-decoration, line-height, margin, padding, display
  - Cascade: author CSS → user-agent defaults → user typography settings
  - `ComputedStyle` per node
- [ ] **M1.5** reeda-epub — CFI position model (`cfi.rs`)
  - CFI parse/serialize (`epubcfi(/6/4[chap03]!/4/2/1:42)`)
  - Range form for annotations
  - `Locator { spine_index, block_index, char_offset }` internal representation
- [ ] **M1.6** reeda-epub — Paginator (`paginator.rs`)
  - `PageLayout { width, height, typography }`
  - `paginate(doc, layout) -> Pages` — block-aware greedy fill
  - `page_containing(pages, cfi) -> PageIndex`
  - `cfi_of_page_start(pages, idx) -> Cfi`
  - Orphan/widow control (2 lines, soft rule)
  - Deterministic: identical inputs → identical Pages
- [ ] **M1.7** reeda-core — Wire EPUB open into App dispatch
  - `Command::OpenBook` triggers EPUB parse → DocumentModel → paginator
  - `StateSnapshot` includes current page text, chapter list, progress
  - Progress save/restore via CFI
- [ ] **M1.8** reeda-ui — Reader screen integration
  - Slint bindings: page text, chapter list, progress bar
  - Typography drawer (font family, size, line-height, theme)
  - Page turn via tap zones + swipe
  - Chapter navigation drawer
- [ ] **M1.9** Tests — unit tests + test EPUB fixtures
  - Minimal test EPUB fixture (Gutenberg-style, 2 chapters)
  - Container/OPF parse tests
  - Nav parse tests
  - XHTML → DocumentModel tests
  - CSS cascade tests
  - CFI round-trip tests
  - Golden pagination tests
- [ ] **M1.10** Update TODO.md + CHANGELOG.md + commit/push

---

## M2 — Library & metadata (2 weeks) — _not started_
## M3 — Highlighting & notes (3 weeks) — _not started_
## M4 — Full-text search (2 weeks) — _not started_
## M5 — Read aloud / TTS (3 weeks) — _not started_
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_

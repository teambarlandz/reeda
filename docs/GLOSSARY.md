# Glossary — Reeda

> Canonical terminology. When in doubt, use the term defined here. Append-only.

| Term | Definition |
|------|------------|
| **AAB / APK** | Android App Bundle (Play delivery) vs Android Package (installable file). We publish AAB to Play, APK for sideload/test. |
| **ACS / ADE** | Adobe Content Server / Adobe Digital Editions — Adobe's DRM. Not supported (ADR-010). |
| **CFI** | Canonical Fragment Identifier (`epub:cfi`) — EPUB standard locator, position + range forms. Canonical position model (ADR-005). |
| **Chapter** | One spine item in an EPUB OPF; a navigational + pagination unit. |
| **Chrome** | Non-content UI (toolbars, overlays). Auto-hides while reading. |
| **Command bus** | The UI → core dispatch channel (`reeda-core::app::Command`). |
| **Cover** | Book cover image extracted at import, stored as WebP, shown in library. |
| **Dedupe** | Import-time SHA-256 check: same file → update, not duplicate (LIB-10). |
| **DocumentModel** | `reeda-epub`'s typed chapter content (blocks, spans, images, links) after XHTML→CSS cascade. |
| **DRM-free** | Unencrypted EPUB/PDF importable by us. Only such files in v1. |
| **EPUB 2 / EPUB 3** | OEBPS 2.x / EPUB 3.x package format: ZIP + OPF + XHTML + nav. |
| **FTS** | Full-text search. |
| **HIL-08 invariant** | Highlights/notes/progress anchored by CFI, never page index; must survive re-layout and restart (PRD 3.4). |
| **Import pipeline** | Hashing → copy → parse → extract → index → done (TECHNICAL_DESIGN §3.4). |
| **KPI** | Key Performance Indicator (see PRD §9). |
| **Locator** | Canonical reference into book content: CFI for EPUB, page + rect for PDF (v1.1). |
| **LWW** | Last-write-wins — sync conflict strategy planned for v2 (schema field `updated_at`). |
| **Narration** | TTS read-aloud: chunking, state machine, word sync (TTS_SPEC.md). |
| **NCX / nav.xhtml** | EPUB 2 table of contents (NCX) vs EPUB 3 navigation document. |
| **OPF** | Open Packaging Format — the EPUB package document (metadata, manifest, spine). |
| **Pagination** | Deterministic layout of a chapter into `Pages` given viewport + typography (pure function). |
| **PDFium** | Chromium's PDF engine; used via `pdfium-render` for rasterization (ADR-006). |
| **Raster cache** | LRU of rasterized PDF pages (memory budgeted, PERFORMANCE.md). |
| **Reader session** | The open-book runtime: current page, position, open annotation/selection state. |
| **SAF** | Storage Access Framework — Android file picking without broad storage permission. |
| **Slint** | Rust-native UI toolkit used for all UI (ADR-001). |
| **Spine** | The ordered list of reading documents in an OPF. |
| **StateSnapshot** | Serializable full app state the UI renders from (diffed after commands). |
| **Tantivy** | Pure-Rust search library powering `reeda-search` (ADR-009). |
| **TTS** | Text-to-speech; Android `TextToSpeech` via JNI (ADR-008). |
| **Utterance** | One speak() call; carries ID used in progress callbacks. |
| **Wake-lock** | Screen-on policy while narrating (user option, TTS-04). |
| **WAL** | SQLite Write-Ahead Logging — durability mode used for the DB. |
| **Zip-slip** | Path-traversal attack via archive entry names; guarded in `reeda-epub`. |

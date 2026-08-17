# Product Requirements Document — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17

## 1. Overview

**Reeda** is a mobile book reader for Android whose goal is feature parity with
Apple Books for the core reading experience: an elegant library, reflowable
EPUB reading, PDF viewing, highlighting and notes, full-text search, and
read-aloud (text-to-speech) — implemented entirely in Rust (Slint UI, Rust
backend) to satisfy a single-stack engineering preference.

### 1.1 Goals (non-negotiable)

- G1: 100% Rust codebase (Slint UI; no Kotlin/Java UI, no webview UI).
- G2: Android as the sole platform for v1 (minSdk 26, target latest stable).
- G3: Read EPUB 2/3 and PDF documents locally, offline, with no accounts.
- G4: Apple Books–class reading experience: typography, themes, highlight,
      notes, bookmarks, progress, search.
- G5: Read-aloud (TTS) for EPUB content with background playback.

### 1.2 Non-goals (v1)

- NG1: DRM-protected books (Adobe ACS/ADE, FairPlay, Kindle). Explicitly out.
- NG2: iOS support (architecture keeps it possible; not scheduled).
- NG3: Storefront / book purchasing.
- NG4: Cloud sync & multi-device sync (local storage only; schema designed to
      allow sync later).
- NG5: Audio books and magazine/ComicBook formats (CBR/CBZ/PDF-lite).

## 2. Personas

| Persona | Description | Key needs |
|---------|-------------|-----------|
| **Avid Reader (Alex)** | Reads 2–4 books/month on the bus | Fast resume, dark theme, large library, search |
| **Student (Sam)** | Annotates heavily for study | Highlights, notes, export, TTS for review |
| **Commuters (Chris)** | Wants hands-free reading | Reliable read-aloud, wake-lock, speed control |
| **Accessibility user (Ari)** | Uses TalkBack + large fonts | Full a11y, adjustable typography, contrast themes |

## 3. Feature requirements

Priorities: **P0** = must have (v1) · **P1** = should have · **P2** = nice to have.

### 3.1 Library & import

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| LIB-01 | Import `.epub` and `.pdf` from SAF (Storage Access Framework) file picker | P0 | `ACTION_OPEN_DOCUMENT` via Android bridge |
| LIB-02 | Import from shared/opened files (other apps → Reeda) | P0 | Intent `VIEW`/`SEND` handling |
| LIB-03 | Library grid with cover art | P0 | Covers extracted at import |
| LIB-04 | Auto-detect empty library → onboarding screen | P0 | |
| LIB-05 | Recently read section | P1 | |
| LIB-06 | Shelves/collections (user-defined) | P1 | |
| LIB-07 | Delete & "remove from library" | P0 | |
| LIB-08 | Sort: title, author, recent | P1 | |
| LIB-09 | Filter: search box in library | P1 | Metadata-level |
| LIB-10 | Duplicate detection (hash-based) | P1 | |

### 3.2 EPUB reading

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| EPR-01 | Reflowable pagination (single scrolling column of pages) | P0 | Text is re-rendered on font/size change |
| EPR-02 | Page-turn gestures (tap zones, swipe) | P0 | See UI_UX.md |
| EPR-03 | Font family & size controls (fixed range, per-book persistence) | P0 | |
| EPR-04 | Line height, margins, alignment, justification controls | P1 | |
| EPR-05 | Themes: Light, Sepia, Dark/Night; auto by system | P0 | |
| EPR-06 | Table of contents navigation | P0 | From `nav.xhtml`/`toc.ncx` |
| EPR-07 | Reading progress bar & jump-to-page | P1 | Page % based |
| EPR-08 | Landscape reading | P1 | Re-pagination |
| EPR-09 | Footnotes/popovers | P2 | |
| EPR-10 | Images inline (float within paragraph) | P1 | EPUB 3 |
| EPR-11 | Internal links (TOC + cross-refs) | P0 | |
| EPR-12 | Re-open at exact position after close/crash | P0 | CFI-based progress |

### 3.3 PDF reading

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| PDF-01 | Render pages via PDFium (rasterized) | P0 | |
| PDF-02 | Vertical scroll + pinch zoom, fit-to-width default | P0 | |
| PDF-03 | Page indicator / jump to page | P1 | |
| PDF-04 | Outline/bookmarks view | P1 | PDF TOC |
| PDF-05 | Text selection & copy | P2 | PDFium text API |
| PDF-06 | Search in PDF | P2 | PDFium search |
| PDF-07 | Highlight in PDF | P2 | Overlay annotations |
| PDF-08 | Night theme (invert/sepia filter) | P1 | Render-time filter |

### 3.4 Highlighting, notes, bookmarks

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| HIL-01 | Select text (drag handles) | P0 | Slint gesture handling |
| HIL-02 | Highlight with 4 colors (yellow/green/blue/pink) | P0 | |
| HIL-03 | Underline highlight style | P2 | |
| HIL-04 | Attach a note to a highlight | P0 | |
| HIL-05 | Bookmarks (position markers, not text-anchored) | P0 | |
| HIL-06 | "Highlights" list screen per book + export | P1 | Export: plain text / Markdown |
| HIL-07 | Tap highlight → edit color, edit/delete note, delete | P0 | |
| HIL-08 | Highlight persists across font changes (anchored by CFI range) | P0 | **Core invariant** |

### 3.5 Search

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| SEA-01 | Full-text search across library | P0 | Tantivy index |
| SEA-02 | Results grouped by book, ranked | P1 | |
| SEA-03 | Tap result → open book at match (EPUB only) | P0 | |
| SEA-04 | Highlight matches in reader | P1 | |
| SEA-05 | Search within current book | P1 | |
| SEA-06 | Rebuild index on import/delete; incremental | P1 | |

### 3.6 Read aloud (TTS)

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| TTS-01 | Narration from current position or chapter start | P0 | |
| TTS-02 | Play/pause/stop, skip fwd/back (chapter or 15s), speed 0.5–2.5× | P0 | |
| TTS-03 | Background playback with notification + media controls | P0 | Foreground service |
| TTS-04 | Wake lock / keep screen policy options | P1 | |
| TTS-05 | Auto-scroll & word highlight during narration | P1 | Synced to TTS UtteranceProgress |
| TTS-06 | Voice selection via system settings | P1 | Delegate to Android TTS settings |
| TTS-07 | TTS in PDF | P2 | Requires text extraction |
| TTS-08 | Audio focus handling (duck/pause on other audio) | P0 | |

### 3.7 Settings & sync

| ID | Feature | Priority | Notes |
|----|---------|----------|-------|
| SET-01 | Default theme, typography defaults | P0 | |
| SET-02 | Library backup/restore (zip of DB + books) | P2 | |
| SET-03 | "Sync" architecture placeholder (CRDT-ready schema) | P2 | Documented, not built |

## 4. Functional requirements (cross-cutting)

- FR-01 **Resume**: app cold-start → last open book rendered in < 1.2 s (see PERFORMANCE.md).
- FR-02 **Offline**: no network calls at runtime in v1. No telemetry.
- FR-03 **Progress durability**: position saved on page turn and every 5 s during
  reading; fsync policy per DATA_MODEL.md.
- FR-04 **File integrity**: imported books are copied into app-private storage,
  never read from the picker URI after import.
- FR-05 **Rotation**: state preserved across rotation without losing position.
- FR-06 **Background kill**: TTS continues via foreground service only when
  narration is active.

## 5. UX requirements (summary)

- UX-01: One-handed reading defaults (tap zones: left = back, right = forward,
  center = chrome). See [UI_UX.md](UI_UX.md).
- UX-02: All chrome auto-hides during reading.
- UX-03: Typography must be adjustable in ≤ 2 taps from the reader.
- UX-04: All destructive actions require confirmation.

## 6. Acceptance criteria (v1 cut)

1. A user can import 3+ EPUBs and 1+ PDF from Downloads, read them end-to-end.
2. Highlight + note survive app restart and font-size changes (HIL-08 invariant).
3. Read-aloud works from lock screen and background for a full chapter.
4. Full-text search across 50 books returns ranked results in < 1 s.
5. 95th percentile page-turn latency < 33 ms for an average EPUB chapter.
6. No crashes in a 24 h soak test with TTS + rotation churn (TESTING.md).

## 7. Out of scope & future

- v1.1: iOS, PDF highlight/search, footnotes popovers.
- v2: Sync, DRM-free storefront, audio books, CBZ/CBR.
- Never: DRM support (licensing, patents, platform terms).

## 8. Risks

| Risk | Mitigation |
|------|------------|
| Slint text rendering limits vs EPUB CSS | Scope CSS subset up-front (EPUB_SPEC.md §5); custom layout if needed (ADR-007) |
| PDFium Android binary supply | `pdfium-render` prebuilt strategy documented (PDF_SPEC.md §7) |
| TTS word-boundary sync drift | Use `UtteranceProgressListener.onRangeStart`; allow resync |
| Large libraries slow Tantivy builds | Background index queue, debounced (SEARCH_SPEC.md §4) |
| Android foreground service restrictions | Read-aloud uses `foregroundServiceType=mediaPlayback` |

## 9. Metrics (post-launch)

- DAU/MAU, books opened/day, avg session length, TTS sessions/day,
  highlight creation rate, crash-free sessions (ANR + native).

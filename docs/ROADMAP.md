# Roadmap — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Work is planned per-milestone. Milestone exits are gated by the criteria
> below + doc completeness per TODO.md lifecycle rules.

## M0 — Foundations (target: 2–3 weeks)

**Goal:** the app shell runs on an Android emulator/device and CI is green.

- [ ] GitHub repo + branch protection (`main`, PR-required, 1 review)
- [ ] Cargo workspace: `reeda-core`, `reeda-epub`, `reeda-pdf`, `reeda-tts`,
      `reeda-search`, `reeda-ui` crates scaffolded, docs-complete
- [ ] Android build environment documented (PLATFORM.md) & CI building a debug APK
- [ ] Slint app shell: window, theme system (Light/Sepia/Dark), navigation shell,
      empty-library onboarding
- [ ] Android bridge crate: SAF file picker, intent handling, permissions
      (storage, notification, foreground service)
- [ ] Storage layer: SQLite (rusqlite) schema migrations v1 (DATA_MODEL.md)
- [ ] Tests: unit test harness for core, emulator smoke test (app launches)

**Exit:** `cargo apk run` shows the shell with an empty library + import button.

## M1 — EPUB reader core (3–4 weeks)

- [ ] EPUB 2/3 archive reading: zip, container.xml, OPF metadata, spine,
      nav.xhtml + ncx TOC (EPUB_SPEC.md)
- [ ] Content model: linearized chapter stream; XHTML → structured text model
      (headings, paragraphs, images, links, footnotes)
- [ ] CSS subset engine (EPUB_SPEC.md §5): font-size, weight, style, color,
      alignment, margins, line-height, images
- [ ] Paginator: measure + paginate per viewport/font settings; deterministic
      pagination given (text, width, height, font config)
- [ ] Reader UI: page turn (swipe + tap zones), typography drawer, themes,
      progress bar, chapter nav
- [ ] CFI progress save/restore (EPUB_SPEC.md §7)

**Exit:** read a full Gutenberg EPUB, turn pages, change font/size/theme,
resume exactly where you left off.

## M2 — Library & metadata (2 weeks)

- [ ] Import pipeline: SAF picker + share intent → copy to app storage → parse
      metadata (title, author, cover, lang, dates)
- [ ] Library grid + list, recent section, sort, delete, duplicate detection
- [ ] Metadata editing (title/author override)
- [ ] Settings screen v1 (theme default, typography defaults)

**Exit:** import 10 books, covers show, recent/continuar correct, delete works.

## M3 — Highlighting & notes (3 weeks)

- [ ] Text selection engine on paginated text (word-accurate ranges, CFI-anchored)
- [ ] Highlight rendering + 4 colors + underline; tap-to-edit; delete
- [ ] Notes attach to highlights; notes list screen
- [ ] Bookmarks; bookmarks list
- [ ] Export highlights/notes (Markdown/plain) via share sheet
- [ ] Position-invariant highlights (HIL-08): survive font changes & restart

**Exit:** highlight a passage, kill the app, relaunch → highlight + note intact
at correct position with different font size.

## M4 — Full-text search (2 weeks)

- [ ] Tantivy index schema (tokenized text, per-book/position fields)
- [ ] Index build on import (background queue), incremental update, delete
- [ ] Search UI: library-wide + in-book, ranked results, open-at-match, term
      highlighting in reader

**Exit:** 50-book fixture library; query finds ranked results < 1 s.

## M5 — Read aloud (TTS) (3 weeks)

- [ ] TTS bridge: Android TextToSpeech via JNI shim (TTS_SPEC.md)
- [ ] Narration state machine (idle/paused/speaking), speed, skip, chapter jump
- [ ] Foreground service + media notification (play/pause/skip/speed), audio
      focus, background/lock-screen operation
- [ ] Word-highlight sync (onRangeStart) + auto page turn during narration
- [ ] Settings: voice, pitch, speed defaults

**Exit:** read a chapter aloud, lock the phone, control from notification,
word highlight tracks speech.

## M6 — PDF support (3 weeks)

- [x] PDFium integration (`pdfium-render`), page rasterization + caching
- [x] Scroll/zoom/fit gestures, page indicator, jump-to-page
- [x] Outline view, night filter
- [ ] (P2 if time) selection/copy, PDF search

**Exit:** PDF renders fast, zooms smoothly, jumps correctly. — MET (M6.5,
2026-08-19): continuous page canvas, fit-width/zoom, jump dialog, outline
panel, night/sepia render filters; deferred P2 items tracked in TODO.md.

## M7 — Hardening & ship (3 weeks)

- [x] Accessibility pass (TalkBack labels, selection a11y, large fonts)
- [x] Localization framework + Standard UK/American English first, plurals, RTL-ready
- [x] Performance pass vs budgets (PERFORMANCE.md)
- [x] Security review (DRM_SECURITY.md), backup rules, crash reporting (opt-in)
- [x] Play Store assets: icon, screenshots, store listing, privacy policy
- [ ] NarrationService (foreground media notification, audio focus,
      wake-lock) — TTS_SPEC §2, needs device (M7g)
- [ ] Build per-ABI APKs → tag v1.0.0 → GitHub Release (APKs + sha256
      + release notes) — NOT Play (user decision 2026-08-19)

**Exit:** v1.0.0 released on GitHub (per-ABI APKs + sha256 + release
notes), device-verified.

## Future (post-v1)

- v1.1: PDF search/highlight, footnotes, iOS evaluation
- v2.0: sync (CRDT-ready schema), CBZ/CBR, audio books, widget

## Velocity & tracking

- 2-week sprints; issues labelled `M0..M7`, `epic`, `bug`, `a11y`.
- Definition of Done (DoD): code + tests + docs updated + CI green + TODO.md
  statuses refreshed.
- Estimates: T-shirt sizing (S/M/L/XL) on issues; epic burndown in milestones.

# Architecture Decision Records — Reeda

> Status: active · Each entry: **Status** (Accepted / Superseded / Rejected) ·
> **Context** · **Decision** · **Consequences**. Append-only; supersede, never
> edit history.

---

## ADR-001 · Pure Rust UI with Slint

- **Status:** Accepted (2026-08-17)
- **Context:** Requirement G1: Rust frontend *and* backend on Android. Options
  were: (a) Slint native rendering; (b) Tauri/Dioxus webview (HTML/CSS UI —
  easier EPUB rendering, but UI not Rust and ships a webview); (c) Kotlin
  Compose UI + Rust core (industry-standard, not Rust UI).
- **Decision:** Use **Slint** for all UI. Android via `cargo-apk` +
  `android-activity`. EPUB/PDF content is rendered by our own engines, not a
  webview.
- **Consequences:** We own the full rendering stack (cost: CSS subset, text
  layout via Slint's cosmic-text). No webview = smaller APK, no WebView
  security surface. EPUB/CSS fidelity is limited to our subset (EPUB_SPEC.md
  §5); re-evaluate if fidelity becomes a blocker (see ADR-007).

## ADR-002 · Android-only for v1 (no iOS)

- **Status:** Accepted
- **Context:** One maintainer, mobile target is Android (user requirement).
- **Decision:** Android (minSdk 26) is the only platform in v1. Platform
  differences are isolated behind the `platform/` trait layer.
- **Consequences:** iOS possible later without touching engines/UI crates.
  Slint's iOS support is experimental today — do not promise iOS in v1 docs.

## ADR-003 · No async runtime in v1; std threads + channels

- **Status:** Accepted
- **Context:** Parsing/indexing/rasterization are CPU-bound and off the UI
  thread. `tokio` adds complexity for no I/O-bound win in a local-only app.
- **Decision:** A small worker pool (`std::thread::scope`-style, mpsc
  channels, generation counters) in `reeda-core`. Introduce `tokio` only
  when network sync lands (v2).
- **Consequences:** Simpler debugging, smaller binary. If v2 sync needs async
  I/O, `tokio` can wrap the existing pool without UI changes.

## ADR-004 · SQLite (rusqlite, bundled) as the system of record

- **Status:** Accepted
- **Context:** Need transactional metadata (library, annotations, settings)
  with migrations, WAL durability, and a path to sync.
- **Decision:** `rusqlite` with bundled SQLite, WAL, versioned embedded
  migrations in `reeda-core/migrations`.
- **Consequences:** One SQLite file is easy to back up; schema designed
  sync-ready (UUIDs, LWW timestamps, soft-delete). Rejected: redb (younger,
  fewer tooling), sled (unmaintained), JSON files (no queries).

## ADR-005 · CFI-compatible positions for EPUB state

- **Status:** Accepted
- **Context:** Page numbers are unstable across fonts/viewports; highlights
  and progress must survive re-layout (HIL-08, EPR-12).
- **Decision:** Adopt EPUB CFI (`epub:cfi`) as canonical locator, including
  range form for annotations; paginate CFI ⇄ page deterministically.
- **Consequences:** Interoperable with other readers; more complex locator
  code (isolated in `reeda-epub::cfi`). Page numbers remain UI-only derived
  values.

## ADR-006 · PDFium via `pdfium-render` for PDF

- **Status:** Accepted
- **Context:** PDF rendering is a hard problem; we must not write a PDF
  rasterizer. Candidates: pdfium-render (Chromium engine, pure wrapper),
  muPDF (C, binding work), lopdf (parse-only).
- **Decision:** `pdfium-render` with Android prebuilt libpdfium, pinned
  version, vendored in CI (PDF_SPEC.md §7).
- **Consequences:** Solid text/outline APIs for v1.1 features; binary supply
  requires a pinned artifact + integrity checks.

## ADR-007 · CSS subset in `reeda-epub` (no browser engine)

- **Status:** Accepted
- **Context:** Full CSS/HTML fidelity needs a browser engine (webview or
  servo) — conflicts with ADR-001 and pure-Rust goals. Apple Books-grade
  typography needs *core* CSS only.
- **Decision:** Implement a deterministic CSS subset (EPUB_SPEC.md §5) that
  covers ≥ 95 % of prose books: headings, emphasis, alignment, margins,
  line-height, images, footnotes, links, colors, backgrounds (page level).
  Unknown rules are ignored with a debug warning; never crash.
- **Consequences:** Complex books may look plainer; monitor the
  "readability report" feature (render a chapter, list unsupported
  constructs) in M1 to validate the 95 % claim.

## ADR-008 · Android TTS with JNI; minimal Java shim

- **Status:** Accepted
- **Context:** Best on-device TTS on Android is `android.speech.tts.
  TextToSpeech` (many engines, voices, word boundaries via
  `onRangeStart`). Rust has no direct binding.
- **Decision:** A ~100-line Java `Activity`/helper shim + `jni` crate calls
  in `reeda-tts::android_bridge`. Foreground service
  (`mediaPlayback`) for lock-screen narration.
- **Consequences:** Small non-Rust surface contained in one crate; all
  narration logic (chunking, state machine, resync) stays in Rust.

## ADR-009 · Tantivy for full-text search

- **Status:** Accepted
- **Context:** Library-wide ranked full-text search (SEA-01). Candidates:
  tantivy (pure Rust, Lucene-like), rusqlite FTS5 (simple, tied to SQLite).
- **Decision:** **Tantivy** in `reeda-search`; index at app-private path,
  rebuilt incrementally per book; SQLite FTS5 rejected (ranking/quality).
- **Consequences:** Heavier dependency (+compile time, +APK); fine for v1.
  Index is derived data — always rebuildable from books.

## ADR-010 · No DRM in v1

- **Status:** Accepted
- **Context:** DRM (Adobe ACS, FairPlay) is legally/technically heavy and
  conflicts with a fully offline, open-source app.
- **Decision:** v1 supports only DRM-free EPUB/PDF. Refuse opening
  ACSM/Adobe-DRM files with a clear message.
- **Consequences:** Some users' purchased books won't import; document
  clearly. DRM never planned (PRD NG1).

## ADR-011 · i18n via custom lightweight catalogs (no gettext-rs in v1)

- **Status:** Superseded (M7c)
- **Context:** Full gettext infrastructure is overkill pre-v1.1; we still need
  plurals + RTL-readiness now.
- **Decision (original):** Embedded catalog format (JSON/XLIFF-like) + plural
  rule helpers in `reeda-ui::i18n`; migrate to `gettext`-style catalogs or
  `fluent` when languages expand (LOCALIZATION.md).
- **M7c superseding decision:** Adopted Slint's **native localization**
  instead of a hand-rolled catalog. UI strings are wrapped in Slint's
  `@tr("…")` (msgid = English text directly, no key indirection), compiled
  into the generated Rust via `slint-build` `with_bundled_translations()`
  from gettext `.po` files (`translations/<lang>/LC_MESSAGES/reeda-ui.po`),
  with runtime locale auto-detection and plural-rule support built into
  Slint 1.17. No custom i18n crate or key-lookup code is shipped.
- **Consequences:** Zero custom catalog code to maintain; new languages are
  just a new `.po` file plus a rebuild. Trade-off: catalog format and
  plural/RTL behavior are bound to the Slint version (pinned via Cargo.lock).

---

## Open questions (to resolve in M0)

- OQ-1: License (MIT vs Apache-2.0 vs MIT/Apache dual) — decide in M0.
- OQ-2: Crash reporting vendor (opt-in, privacy-first) — decide in M7.
- OQ-3: Icon/branding direction — decide in M2 (no designer yet; Slint
  component style chosen: Material3-like custom, see UI_UX.md).

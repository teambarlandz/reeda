# Reeda — Master Project Plan & Documentation Index

> **Reeda** is a mobile book reader (Apple Books–style) built 100% in Rust:
> Slint UI on Android, with EPUB & PDF rendering, text-to-speech ("read
> aloud"), highlighting, notes, bookmarks, and full-text search.
>
> This file is the single source of truth for project planning. It exhaustively
> enumerates **every form of documentation** the project uses, in the order the
> documents are produced, with their purpose, audience, and status.
>
> Legend: `[x]` done · `[~]` in progress · `[ ]` planned · `[-]` dropped

---

## 1. Project charter & planning

| Doc | Purpose | Audience | Status |
|-----|---------|----------|--------|
| `TODO.md` (this file) | Master index of all planning & documentation work | Everyone | [~] |
| `README.md` | Public face: what the app is, badges, quick start, doc map | Users & contributors | [ ] |
| `docs/PRD.md` | Product Requirements Document: goals, personas, feature matrix vs Apple Books | Product, all devs | [ ] |
| `docs/ROADMAP.md` | Phased milestones (M0–M6) with exit criteria | PM, all devs | [ ] |
| `docs/GLOSSARY.md` | Canonical terminology (EPUB, KPI, CFI, TTS, etc.) | Everyone | [ ] |

## 2. Technical documentation

| Doc | Purpose | Audience | Status |
|-----|---------|----------|--------|
| `docs/ARCHITECTURE.md` | High-level system architecture: crates, threads, IPC, Android integration | Architects, senior devs | [ ] |
| `docs/TECHNICAL_DESIGN.md` | Detailed design: data flow, module boundaries, error handling, async model | All devs | [ ] |
| `docs/ADR.md` | Architecture Decision Records — every significant decision & rationale | Architects, reviewers | [ ] |
| `docs/DATA_MODEL.md` | Storage schema: library, books, highlights, notes, progress, settings | Backend/devs | [ ] |

## 3. Feature specifications

| Doc | Purpose | Audience | Status |
|-----|---------|----------|--------|
| `docs/EPUB_SPEC.md` | EPUB 2/3 parsing & rendering strategy (zip, XHTML, CSS subset, reflow) | Devs | [ ] |
| `docs/PDF_SPEC.md` | PDF rendering via PDFium, page rasterization, zoom, reflow-vs-fixed | Devs | [ ] |
| `docs/TTS_SPEC.md` | Read-aloud: Android TextToSpeech bridge, narration state, controls | Devs | [ ] |
| `docs/HIGHLIGHTS_SPEC.md` | Highlighting, notes, bookmarks, export (Apple Books parity) | Devs | [ ] |
| `docs/SEARCH_SPEC.md` | Full-text search (Tantivy), index lifecycle, UI behavior | Devs | [ ] |
| `docs/UI_UX.md` | Screens, navigation, gestures, theming, typography, Slint component map | UI devs, designers | [ ] |

## 4. Platform & operations

| Doc | Purpose | Audience | Status |
|-----|---------|----------|--------|
| `docs/PLATFORM.md` | Android build environment: NDK, cargo-ndk, cargo-apk, permissions, min SDK | DevOps, devs | [ ] |
| `docs/BUILD_CI.md` | CI/CD: GitHub Actions, workspace checks, APK/AAB signing, release flow | DevOps | [ ] |
| `docs/TESTING.md` | Test strategy: unit, integration, golden tests, device matrix, TDD on core | QA, devs | [ ] |
| `docs/PERFORMANCE.md` | Performance budgets: launch, page turn, memory, battery | All devs | [ ] |
| `docs/ACCESSIBILITY.md` | Screen-reader hooks, font scaling, contrast, TalkBack + Rust a11y | All devs | [ ] |
| `docs/DRM_SECURITY.md` | DRM stance (no DRM in v1), privacy, storage encryption, hardening | Security, PM | [ ] |
| `docs/LOCALIZATION.md` | i18n/l10n: gettext-style catalogs, plurals, RTL, date formats | All devs | [ ] |

## 5. Delivery & governance

| Doc | Purpose | Audience | Status |
|-----|---------|----------|--------|
| `docs/CONTRIBUTING.md` | Contribution workflow, code style, PR checklist, review rules | Contributors | [ ] |
| `docs/RELEASE.md` | Versioning, changelog policy, Play Store publishing, beta channels | DevOps, PM | [ ] |
| `docs/CHANGELOG.md` | Human-readable version history | Everyone | [ ] |

---

## 6. Documentation lifecycle rules

1. **Doc-first development.** A feature is not "started" until its spec section is
   written; it is not "done" until its doc status is `[x]`.
2. **Single source of truth.** Diagrams live in their owning doc; other docs link,
   never copy.
3. **ADR hygiene.** Any change in dependencies, architecture, or formats must add
   an ADR entry with context, decision, and consequences.
4. **Doc review.** Specs are reviewed in the same PR as code they describe.
5. **Living docs.** `TODO.md` statuses are updated in every PR that touches its
   topics.

## 7. Project milestones (summary — see ROADMAP.md)

| Milestone | Theme | Exit criteria |
|-----------|-------|---------------|
| M0 | Foundations | Workspace, CI, docs green, app shell launches on emulator |
| M1 | EPUB reader | Open EPUB, paginate, page-turn, reflow, typography |
| M2 | Library & metadata | Import, library grid, metadata extraction, recent/shelves |
| M3 | Highlighting & notes | Highlight, annotate, bookmark, view & export |
| M4 | Search | Full-text index + search UX |
| M5 | Read aloud (TTS) | Narration, playback controls, wake-lock, word sync |
| M6 | PDF support | PDFium rasterization, zoom, scroll |
| M7 | Hardening & ship | a11y, i18n, perf, Play Store release |

## 8. Immediate next steps (first session)

- [x] Decide stack: **Slint (pure Rust UI) + Android**, Rust backend everywhere
- [x] Create this documentation index
- [ ] Write all `[ ]` docs listed above (see sections 1–5)
- [ ] Scaffold Cargo workspace (`crates/reeda-*`) and Android project files
- [ ] Create GitHub repository `teambarlandz/reeda` and push
- [ ] Execute M0 (see ROADMAP.md)

---

_Last updated: 2026-08-17 · Owner: @teambarlandz · Status key: [x] done · [~] in progress · [ ] planned · [-] dropped_

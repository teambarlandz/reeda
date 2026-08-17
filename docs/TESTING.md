# Testing Strategy — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Principle: **test the engines hard, smoke the UI.** The render/pagination/
> search logic is pure Rust and fully testable headless; Android UI is
> exercised via emulator smoke + goldens.

## 1. Test pyramid

| Layer | What | Where | Runs |
|-------|------|-------|------|
| Unit | parsing, CSS, CFI, pagination math, chunker, state machines, storage | crate `#[cfg(test)]` | CI `ci.yml` |
| Property/fuzz | randomized CFI/range intersections, zip parsers, xhtml/css | `cargo-fuzz`, `proptest` | CI nightly |
| Integration | import pipeline, annotation lifecycle, index lifecycle, TTS state with fake host | `tests/` per crate, headless | CI |
| Golden | pagination tables, export formats, screenshot per theme/screen | `reeda-ui` goldens + emulator | CI (emulator job) |
| Device | TTS real engine, notifications, rotation, foreground service, Doze | emulator/device manual + smoke job | `workflow_dispatch` / release gate |

## 2. Host-side harness

- `reeda-core` tests against in-memory SQLite (`:memory:`) + `TempDir` file
  tree; a `TestPlatform` records TTS/notification/SAF calls → assertions
  (TECHNICAL_DESIGN §6).
- `reeda-epub` fixture generator (deterministic, seeded): books of N
  chapters, odd encodings, images, malicious zips (zip-slip, bombs,
  malformed). Golden pagination = exact `Pages` tables per fixture+layout.
- `reeda-search` corpus fixtures: multi-language, 1 M-word book; golden
  top-N queries.
- `reeda-tts`: chunker unit tests + state-machine tests with `FakeTtsHost`
  (TTS_SPEC §8); no real TTS on host.

## 3. Integration scenarios (headless, per milestone)

- M1: import → paginate → change font → paginate → CFI resume →
  highlights survive. Golden: CFI table equality across font sizes.
- M3: select range → highlight → note → export markdown → restart app →
  geometry identical (HIL-08 invariant test, automated).
- M4: 50-book import → index build → queries (phrase, prefix, CJK,
  diacritics) → delete book → hits gone.
- M5: narration run-through with fake host; pause/resume/speed; chapter
  boundary; error path ×3 → pause.

## 4. Property & fuzz tests

- CFI parse/serialize round-trip (proptest on random valid-ish strings).
- Range intersection: random ranges over fixture text → clip correctness.
- Container parser fuzz targets: `container_fuzz` (zip), `xhtml_fuzz`,
  `css_fuzz` — corpus seeded with real-world EPUBs; invariants: no panic,
  no unbounded memory (assert depth/size caps).
- Paginator fuzz: random layouts/typography over fixtures → pagination is
  total (every char assigned), monotone, reproducible.

## 5. UI & goldens

- Emulator job (API 34, Pixel 6a profile): boot → install APK → launch →
  import fixture book → capture screenshots (library, reader light/sepia/
  night, Aa panel, highlights list, search, TTS bar, PDF viewer) →
  diff against baseline (pixel tolerance 0.5 %, image-size normalized).
- Baselines stored in `tests/goldens/` (committed); updated deliberately
  via PR (never silently).
- Gesture smoke on emulator: page-turn swipe, tap zones, long-press
  selection, TTS notification buttons (via `adb` uiautomator or Slint
  test backend).
- Rotation matrix: portrait↔landscape in reader mid-TTS (no crash, position
  preserved — FR-05).

## 6. Device matrix (release gate)

| Tier | Devices | Checks |
|------|---------|--------|
| P0 | Pixel 6a (API 34), Pixel 8 (API 35) | Full smoke + soak 24 h (TTS + rotation churn) |
| P1 | Low-end API 26 (e.g. Galaxy A10e) | Perf budgets (PERFORMANCE.md), TTS basic |
| P2 | Tablets (Lenovo Tab API 34) | Landscape reader, multi-column grid |

Manual test checklist in `docs/RELEASE.md` §5 (updated each release).

## 7. Coverage goals

- `reeda-epub`, `reeda-core::storage`, `reeda-tts::engine`, `reeda-search`
  ≥ 85 % line coverage (llvm-cov in CI, badge).
- UI: coverage not applicable; golden + smoke instead.
- Metrics reported in PRs via comment (coverage diff).

## 8. Crash reporting & monitoring (v1.1)

- Opt-in crash collection (native + JVM via `rustc-demangle` mapped) —
  privacy-first, see DRM_SECURITY.md. v1.0 ships without; soak test via CI
  device matrix instead.

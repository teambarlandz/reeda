# Localization (i18n/l10n) — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-ui::i18n` (ADR-011). v1 ships **English only** but
> the framework, plural rules, and RTL handling are in place from M0.

## 1. Architecture

- **Catalog format**: JSON files per locale, flat keys with context:
  `ui/translations/en.json`, `<locale>.json`. Structure:
  ```json
  { "reader.aa_panel.title": "Typography", "common.close": "Close",
    "progress.percent": { "one": "{pct}% read", "other": "{pct}% read" } }
  ```
- Keys are stable, never user-facing; IDs referenced in `.slint` via
  `tr("key")` macro → compiled lookup table at build time (Slint's
  `tr` support).
- **Plural rules**: ICU-ish minimal set (`one/other/zero/two/few/many`
  subset needed by target languages) implemented in Rust; no external
  ICU4X in v1 (P2 upgrade path documented).
- **Placeholders**: `{name}` substitution with type-checked args
  (compile-time macro).

## 2. Locale resolution

1. System locale (Android `LOCALE` → `lang[-region]`) → closest catalog
   (exact > base language > `en` fallback).
2. Book-level language affects: TTS voice default (TTS_SPEC §7),
   search analyzer (SEARCH_SPEC §3) — independent of UI locale.

## 3. RTL support

- Layout direction from locale (ar, he, fa, ur…): Slint mirroring
  (page-turn direction flips: right-side = previous), tap zones mirrored,
  progress bar direction, selection handles.
- EPUB content `dir`/`bidi` handled by the text stack (html5 + CSS
  `direction` best-effort; bidi algorithm via Slint text engine).
- Verification: golden screenshots for an RTL locale (ar) per screen
  (TESTING.md §5).

## 4. Formats

- Dates (imported_at, annotations): locale-aware via `intl`-style pattern
  (short date + 12/24 h per locale) — small hand-rolled formatter v1.
- Percent (progress), speed (0.5×–2.5×): decimal separator per locale.
- Time (TTS remaining): localized units.

## 5. Workflow & tooling

- `scripts/extract_strings.ps1` scans `.slint` + Rust for `tr("…")` → syncs
  `en.json` (source of truth); other locales manual/translation service.
- CI: missing-key check (every locale must resolve every key; unused keys
  removed); placeholder-mismatch check.
- New language = new `ui/translations/<locale>.json` + plural rules entry +
  RTL flag + golden update. No code changes expected.

## 6. Testing

- Unit: catalog resolution, plural selection, placeholder typing.
- Golden: each locale × key screens (screenshot job), overflow check at
  fixed width (German/Japanese longest-strings fixture).
- Device: system locale set via `adb shell setprop persist.sys.locale ar`
  → smoke.

## 7. v1 scope

- English (en) ship; en-GB as variant (spelling), ar RTL readiness tested,
  fr/es/de/pt as first translations in v1.1 (PRD allows).
- All user-facing strings are tr()-wrapped from M0 — no hardcoded text
  except the app name.

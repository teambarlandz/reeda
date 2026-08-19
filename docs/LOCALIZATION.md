# Localization (i18n/l10n) — Reeda

> Status: implemented (M7c) · Version: 1.0 · Owner: @teambarlandz
> Last updated: 2026-08-19
> Implementation: Slint's native localization (`@tr` + gettext `.po`
> catalogs bundled at build time). Supersedes the draft JSON-catalog design
> in ADR-011. v1 ships **Standard UK/American English**; the framework,
> plural rules, and locale auto-detection are in place for more languages.

## 1. Architecture

- **Macro**: all user-facing strings in `.slint` are wrapped in Slint's
  `@tr("…")`. The msgid IS the English text (no key indirection), so the
  source `.slint` files stay readable and searchable. Strings containing
  `{`/`}` or interpolations that Slint cannot treat as translatable are
  left unwrapped.
- **Catalogs**: gettext `.po` files per locale under
  `crates/reeda-ui/translations/<lang>/LC_MESSAGES/reeda-ui.po`. The
  translation domain is the crate name (`CARGO_PKG_NAME` = `reeda-ui`),
  set automatically by `slint-build`.
- **Bundling**: `crates/reeda-ui/build.rs` compiles `ui/AppRoot.slint`
  via `slint_build::compile_with_config` with
  `with_bundled_translations("translations")` and
  `with_default_translation_context(slint_build::DefaultTranslationContext::None)`
  (no per-component msgctxt). Slint 1.17 enables the compiler's
  `bundle-translations` feature by default, so the generated
  `out/AppRoot.rs` embeds `_SLINT_TRANSLATED_STRINGS` and calls
  `set_bundled_languages(_SLINT_BUNDLED_TRANSLATIONS)` in `AppRoot::new()`.
- **Plural rules**: supported by Slint's `@tr` syntax
  (`@tr("n items" | "%n items", n)`); plural forms are taken from each
  `.po` file's `Plural-Forms` header (default `n != 1`). No custom
  plural code is shipped.
- **Contexts** (msgctxt) and placeholders with format args are supported by
  Slint but unused so far; add them only when a language needs
  disambiguation or reordering.

## 2. Locale resolution

1. At startup `set_bundled_languages` selects the catalog from the system
   locale: exact match (`en-GB`) first, then base-language match (`en` for
   `en-US`), else the default (empty) catalog.
2. A language can be forced at runtime with
   `slint::select_bundled_translation("en-GB")` (after the first component
   is created); this also updates the decimal separator.
3. Book-level language affects TTS voice default (TTS_SPEC §7) and the
   search analyzer (SEARCH_SPEC §3) independently of the UI locale.

## 3. RTL support

- RTL-ready by design (Slint mirrors layouts and text direction when the
  locale declares RTL), but no RTL locale is shipped in v1. When one is
  added, verify page-turn direction, tap zones, progress bar direction,
  and selection handles per screen (TESTING.md §5).

## 4. Formats

- Decimal separator comes from Slint's bundled-translation runtime
  (`decimal_separator_for_locale`). Percent (progress) and TTS speed use
  `to-string`-style formatting in the `.slint` files.
- Dates (imported_at, annotations) and time (TTS remaining) are currently
  formatted in Rust (`reeda-ui`) with fixed US style; locale-aware
  formatting is a P2 follow-up.

## 5. Workflow & tooling

- `crates/reeda-ui/translations/en/LC_MESSAGES/reeda-ui.po` is the identity
  catalog (msgstr = msgid) and the source of truth for extractable strings.
- New locale = new `translations/<lang>/LC_MESSAGES/reeda-ui.po` + rebuild;
  the catalog is regenerated/verified from the `@tr` strings in
  `ui/*.slint`. No code changes expected.
- CI: `cargo build -p reeda-ui` fails if a catalog is malformed (a
  diagnostic is pushed by the compiler). Missing translations silently fall
  back to the msgid (English), so gaps are visible but non-fatal.

## 6. Testing

- Unit: `cargo test -p reeda-ui` — the generated component embeds the
  translation tables; verify `set_bundled_languages` / `_SLINT_BUNDLED_TRANSLATIONS`
  appear in `out/AppRoot.rs` after a build.
- Manual: launch the app with a different system display language
  (or call `select_bundled_translation("en-GB")` in a debug build) and check
  the visible strings (e.g. "colour" for en-GB).
- Device: system locale set via `adb shell setprop persist.sys.locale en-GB`
  → smoke.

## 7. v1 scope

- English (en) identity catalog ships; en-GB is a bundled spelling variant
  ("colour" for highlight colors). Further languages (fr/es/de/pt) land in
  v1.1+ per PRD.
- The app name ("Reeda") is wrapped in `@tr` and therefore translatable.
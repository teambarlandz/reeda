# Release & Publishing — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> From "tag a version" to "live on Google Play".

## 1. Versioning

- **SemVer** (`X.Y.Z`): major = breaking UX/format; minor = features; patch
  = fixes. Pre-release: `-beta.N`, `-rc.N` on Play tracks.
- Source of truth: `VERSION` in `Cargo.toml` (workspace root, single
  version for all crates in v1) + git tag `vX.Y.Z`; CI asserts they match.
- Changelog: [CHANGELOG.md](CHANGELOG.md) — Unreleased section merged into
  the release section at tagging (keep-a-changelog style).

## 2. Release checklist (every release)

1. `main` green (ci + build-apk), all P0 device checks passed
   (TESTING.md §6).
2. `docs/RELEASE.md` manual checklist executed on a P0 device:
   - Fresh install → import EPUB+PDF → read 10 min → highlight+note →
     restart → verify (HIL-08) → TTS 2 min → notification controls →
     rotate during TTS → Doze during TTS → backup/restore.
3. Changelog updated; TODO.md statuses refreshed.
4. Tag `vX.Y.Z` (annotated) → `release.yml` builds + signs APKs
   (BUILD_CI.md §4) → GitHub Release draft with assets.
5. Upload to Play (below) → monitor first 48 h (crashes via Play
   Console ANR/CR — no third-party crash tool in v1.0).

## 3. Google Play publishing (v1.0+, via Play Console web upload)

- **Tracks**: internal testing → closed beta (10–100 testers) → open beta
  → production. Staged rollout 10 % → 100 % (v1.0+).
- **AAB vs APK**: v1.0 uploads **APKs** (arm64 + x86_64) — fine at this
  scale; switch to **AAB** when App Bundle features needed (v1.1).
- Package id: `io.reeda.app` (final decision in M0 — affects forever;
  reverse-domain must be owned).
- Target devices: phones + tablets; min API 26; no Chromebook exclusion
  (free win, verify later).
- App icon: adaptive (foreground/background layers, mipmaps); screenshots:
  2 phones + 1 tablet, English + localized later (LOCALIZATION.md).
- **Store listing** (drafted M7): short description, full description,
  feature list (DRM-free, offline, read-aloud, highlights, search), privacy
  policy URL (hosted page — see §7).

## 4. Signing & key management

- Release key: generated once, backed up offline (never in repo — secrets
  in CI only, BUILD_CI.md §5). Key loss = cannot update app.
- Rotation: documented via Play Console "app signing by Google Play"
  (upload key) — decision in M0; prefer **Play App Signing** (safe).

## 5. Manual test checklist (stored here, updated per release)

- [ ] Fresh install via Play internal track (no sideload shortcuts)
- [ ] Import: SAF picker (epub+pdf), share intent, duplicate import
- [ ] Reader: 5 themes × 3 font sizes page-turn smoothness
- [ ] Highlight 4 colors + note → export markdown → share
- [ ] Search 3 queries incl. phrase + CJK (if corpus)
- [ ] TTS: start from middle, notification controls, lock screen, focus
      duck (start music app), speed 2.5×
- [ ] Rotation churn 20×; kill during TTS; battery drain spot-check
- [ ] Backups: settings → backup → wipe → restore
- [ ] a11y: TalkBack full pass + 200 % font scale (ACCESSIBILITY.md §6)

## 6. Post-release duties

- Watch Play Console: crashes/ANRs (0-crash bar: < 0.1 % sessions),
  ratings & reviews triage weekly, staged rollout control.
- Security patches to pinned deps (`cargo audit`) applied within 2 weeks
  for `high` severity.
- Hotfix process: `fix/x.y.z` branch from tag → patch → new tag.

## 7. Store legal artifacts

- **Privacy policy**: hosted page (GitHub Pages or own domain), covering:
  no data collection in v1, local-only storage, optional crash reporting
  (v1.1), TTS engine note, backup behavior. Link in Play listing + in-app
  Settings → About.
- **Data safety form**: "No data collected" (v1.0).
- License: decision in M0 (ADR OQ-1); app LICENSE + third-party notices
  file bundled (Slint, rusqlite, tantivy, pdfium-render, etc. — generated
  by `cargo-about` in CI).

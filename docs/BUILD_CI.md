# CI/CD — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> GitHub Actions. Workflow files: `.github/workflows/{ci,build-apk,release}.yml`.

## 1. Workflow overview

| Workflow | Trigger | Jobs | Artifacts |
|----------|---------|------|-----------|
| `ci.yml` | PR + push `main` | fmt, clippy, test (host), doc-build, fuzz smoke | — |
| `build-apk.yml` | push `main`, manual | Android debug APK (arm64 + x86_64) | APKs, sha256 |
| `release.yml` | tag `v*` | Signed release AAB/APKs, Play publish (manual step) | release assets |

Branch protection: `main` requires `ci.yml` green + 1 review; no direct
pushes (CONTRIBUTING.md).

## 2. CI job — `ci.yml`

- Runner: `ubuntu-latest` (host tests) — plus `windows-latest` for
  path/workspace parity check (light job, optional).
- Steps: checkout → `dtolnay/rust-toolchain@stable` → **cache** (`Swatinem/
  rust-cache`) → `cargo fmt --check` → `cargo clippy --workspace
  --all-targets -- -D warnings` → `cargo test --workspace` →
  `cargo doc --no-deps` (broken-links) → nightly fuzz smoke (manual trigger,
  `cargo fuzz run` 30 s per target on hosted corpus).
- Fast-fail: all lint steps in one job to reuse cache; test job parallel.

## 3. Android build job — `build-apk.yml`

- Runner: `ubuntu-latest` (Linux NDK toolchain standard).
- Env: `ANDROID_NDK_HOME` (NDK r27 pinned via `android-actions/setup-android`
  or explicit download), Rust targets installed via rust-toolchain.toml.
- Steps:
  1. Fetch + verify `libpdfium.so` per ABI (PDF_SPEC §7; hashes in
     `third_party/pdfium/SHA256SUMS`).
  2. `cargo install cargo-apk cargo-ndk --locked`.
  3. Build per ABI → `target/ndk/<abi>/.../reeda-ui.apk` → merge with
     `android/` assets via `cargo apk` (debug signing auto).
  4. Upload `reeda-debug-<abi>.apk` + `sha256sums.txt` (12 h retention for
     PR artifact links; pinned for release).
- Matrix: `[arm64-v8a, x86_64]` (32-bit arm optional nightly).

## 4. Release workflow — `release.yml`

1. Tag `vX.Y.Z` (semver, RELEASE.md) → build release APKs per ABI with
   `cargo-apk --release`.
2. Sign: `keystore.jks` from `RELEASE_KEYSTORE` secret; `zipalign` +
   `apksigner` (build-tools pinned version) → signed APKs.
3. GitHub Release (v1.0.0 target — **not** Play): changelog excerpt
   (CHANGELOG.md section), per-ABI APKs + `sha256sums.txt`,
   provenance (attest-build-provenance action). Desktop zip from
   `scripts/package.ps1` attached too.
4. Play/F-Droid (deferred, v1.1+ if adopted): AAB via `bundletool`
   (Play requires AAB for delivery) + upload via web console or fastlane
   — documented in RELEASE.md §3 (marked Play-only).

## 5. Secrets (GitHub)

| Secret | Used by | Note |
|--------|---------|------|
| `RELEASE_KEYSTORE` | release.yml | base64 of JKS |
| `KEYSTORE_PASSWORD` / `KEY_ALIAS` / `KEY_PASSWORD` | release.yml | |
| `PLAY_SERVICE_ACCOUNT_JSON` (v1.1) | release.yml | Play API upload |

Secrets never appear in logs; sign step echoes only fingerprints.

## 6. Caching strategy

- `Swatinem/rust-cache@v2` keyed on `Cargo.lock` + toolchain; monthly
  prune. Android target cache separate (ndk build dir).
- PDFium binaries cached by version hash (not re-downloaded).

## 7. Test integration

- `ci.yml` runs host unit/integration; `build-apk.yml` runs a 2-device
  matrix smoke (launch + screenshot diff) via `reactivecircus/android-
  emulator-runner` — guarded to `workflow_dispatch` (slow).
- Golden screenshot job (TESTING.md §5) runs on emulator, artifacts
  uploaded for visual review.

## 8. Definition of Done (CI terms)

A PR is mergeable when: `ci.yml` green, docs touched are reviewed, TODO.md
statuses updated, no new clippy warnings, migrations covered by tests.

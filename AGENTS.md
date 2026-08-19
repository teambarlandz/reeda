# AGENTS.md — Reeda session state & conventions

> Handoff document for AI agents. Read this first. Last updated: 2026-08-19
> (by the agent that completed M7e/M7f; next work: M7g).

## Objective

Finish M7 (hardening) and ship **v1.0.0 via GitHub Releases** (user
decision 2026-08-19: **not** Google Play). Exit: release tag `v1.0.0` with
per-ABI APKs + sha256 + release notes on GitHub.

## Repo facts

- Remote: `https://github.com/teambarlandz/reeda.git`, branch `main`.
  Commit + push explicitly only when asked.
- Workspace layout: `crates/{reeda-core,reeda-epub,reeda-pdf,reeda-search,
  reeda-tts,reeda-ui}`, `android/` (manifest + Java shim + res/),
  `third_party/pdfium/`, `docs/`, `scripts/`, `.github/workflows/`.
- Tests: 214 green (M7d). Run everything with:
  `$env:PDFIUM_LIBRARY_PATH = "C:\Users\USER\Downloads\reeda\third_party\pdfium\win-x64\pdfium.dll"`
  then `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`.
- PowerShell 5.1: no `??=`; cargo stderr + `$ErrorActionPreference="Stop"`
  throws NativeCommandError — use `"Continue"`; prefer 600000 ms timeouts.

## Progress (all committed/pushed)

- **M0–M6 done** (EPUB reader, search, TTS engine + JNI bridge, PDF via
  PDFium). `git log` for detail.
- **M7a** `f5216d0` desktop packaging (pdfium.dll bundled, `scripts/package.ps1`)
- **M7b** `1cfc076` accessibility pass (labels, roles; 212 tests)
- **M7c** `d1f1896` localization (Slint @tr + bundled gettext catalogs;
  en + en-GB embedded; runtime auto-select)
- **M7d** `c73334f` performance pass (PDF raster cache wired; benches +
  `scripts/bench_desktop.ps1`; 214 tests)
- **M7e** `a23fda1` security (ADR OQ-2 crash reporting: none in v1;
  cargo-audit CI job; clippy `undocumented_unsafe_blocks=deny`; Android
  backup rules `android/res/xml/{backup_rules,data_extraction_rules}.xml`)
- **M7f** `d986513` app icon + store assets (`android/res/drawable/
  ic_launcher_foreground.xml`, `scripts/make_icons.ps1`, `docs/store/`)
  — Play-specific listing/privacy-policy are now **optional** (only if
  Play/F-Droid later; no longer a v1 blocker).

## Current state — M7g (in progress, interrupted)

User aborted the toolchain install mid-way on this machine and moved to
another device. **Do NOT reinstall toolchains unless the user asks.**

State on the previous machine (may not apply here):
- NDK r27 zip + cmdline-tools zip downloads were **truncated/aborted**
  (`C:\Users\USER\Downloads\android-ndk-r27-windows.zip` ~55 MB of ~640 MB;
  `cmdline-tools.zip` ~57 MB of ~130 MB).
- `rustup target add aarch64-linux-android x86_64-linux-android` FAILED:
  "detected conflict ... lib\rustlib\aarch64-linux-android\lib\
  libaddr2line-*.rlib" (partial install). Fix: remove the target dir
  (`rustup target remove aarch64-linux-android`, or delete the rlib) then
  re-add. `x86_64-pc-windows-msvc` is the only installed target.
- `cargo-apk` / `cargo-ndk` / clang / ANDROID_SDK_ROOT: all absent.
- **`third_party/pdfium/` has ONLY `win-x64/pdfium.dll`** — the Android
  build needs `libpdfium.so` per ABI (arm64-v8a, x86_64) from
  `bblanchon/pdfium-binaries` releases (pinned tag + sha256, PDF_SPEC §7),
  placed in `android/src/main/jniLibs/<abi>/`.

### M7g must-dos (next agent)

1. **Write `android/src/io/reeda/app/NarrationService.java`** — the
   manifest ALREADY declares `<service android:name="io.reeda.app.
   NarrationService" android:foregroundServiceType="mediaPlayback"/>` but
   the class does NOT exist → `cargo-apk` build will fail. Implement per
   TTS_SPEC §2 (foreground service started when narration starts; media
   notification with play/pause/stop/skip-back/skip-forward/speed actions;
   PendingIntents → JNI → Rust engine; audio focus GAIN/LOSS/DUCK; partial
   wake-lock). Notification actions need a new `HostEvent::Control` variant
   in `crates/reeda-tts/src/engine.rs` (`handle_event` maps to existing
   `pause/resume/stop/set_rate`), and new JNI entry point(s) in
   `crates/reeda-tts/src/android_bridge.rs` + `start/stop` service calls
   from `AndroidTtsHost` (store a GlobalRef to the app context — currently
   not retained). Manifest permissions for all of this are ALREADY present
   (FOREGROUND_SERVICE, FOREGROUND_SERVICE_MEDIA_PLAYBACK, WAKE_LOCK,
   POST_NOTIFICATIONS).
2. **Toolchain** (on a machine with disk space): install NDK r27 (pinned,
   BUILD_CI.md §3), Android SDK cmdline-tools + `build-tools;35.0.0` +
   `platforms;android-35` (targetSdk 35, minSdk 26 in manifest), `cargo
   install cargo-apk cargo-ndk --locked`, rustup android targets, fetch
   libpdfium.so per ABI → `android/src/main/jniLibs/<abi>/`.
3. **Build the APK**: `cargo apk build --release --target
   aarch64-linux-android` (+ x86_64). Verify `android/res` (icon mipmaps,
   backup rules) and the manifest are packaged.
4. **GitHub release**: tag `v1.0.0`, attach per-ABI APKs + `sha256sums.txt`
   + CHANGELOG excerpt + privacy policy link (host `docs/store/
   privacy_policy.md`). Consider signing per BUILD_CI.md §4-5 (keystore via
   GitHub secret, never in repo).
5. **Device verification** (RELEASE.md §5 checklist): import SAF, reader
   themes, highlights, search, TTS notification controls, rotation, backup.
   Screenshots for README can come from a device too.

### Docs to update when M7g lands

ROADMAP.md M7 line 102 + M7 exit ("v1.0.0 published on Google Play" →
"v1.0.0 released on GitHub"); RELEASE.md (Play steps → GitHub releases,
screenshots optional, privacy policy hosted for Settings→About);
BUILD_CI.md §4 release workflow (drop Play upload manual step, AAB note);
TODO.md M7g item; CHANGELOG.md M7g entry. `docs/store/listing.md` is
Play-only — keep for later, not a v1 blocker.

## Conventions & decisions

- Languages: Standard UK + American English first (Nigerian English
  optional) — M7c.
- Crash reporting: none in v1 (ADR OQ-2). DRM-free only. No telemetry.
- Every `unsafe` block needs a SAFETY comment (clippy denies).
- Keep responses short; batch commands; user is impatient with long
  investigations — don't re-verify already-verified things.
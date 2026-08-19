# Android Platform & Build Environment — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Covers everything needed to build Reeda for Android on any OS
> (primary dev: Windows).

## 1. Supported targets

| ABI | Rust target | Devices |
|-----|-------------|---------|
| arm64-v8a | `aarch64-linux-android` | Modern devices (primary) |
| armeabi-v7a | `armv7-linux-androideabi` | Legacy 32-bit (CI only, optional) |
| x86_64 | `x86_64-linux-android` | Emulators |
| x86 | `i686-linux-android` | Old emulators (optional) |

- **minSdk 26** (Android 8.0) — TTS `onRangeStart` (API 26), notification
  channels (26), SAF. targetSdk: latest stable at release time.
- Gradle is **not** used: `cargo-apk` + `android-activity` produce APKs
  directly from Rust. Gradle/AGP only if we later need Play Asset Delivery.

## 2. Toolchain setup (Windows primary)

```powershell
# 1. Rust + Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# 2. NDK (via Android Studio SDK Manager, or sdkmanager)
#    NDK r27+, path e.g. %LOCALAPPDATA%\Android\Sdk\ndk\27.x.x
$env:ANDROID_NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018"
$env:ANDROID_SDK_ROOT = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_HOME   = $env:ANDROID_SDK_ROOT

# 3. cargo-ndk + cargo-apk
cargo install cargo-ndk cargo-apk

# 4. Rust tools (repo pins versions — rust-toolchain.toml)
rustup component add llvm-tools      # cargo-apk/zipalign tooling
```

Linux/macOS: same steps, `export` instead of `$env:`; macOS additionally
`brew install android-ndk` alternative.

`rust-toolchain.toml` (repo root):
```toml
[toolchain]
channel = "stable"
targets = ["aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android"]
profile = "minimal"
```

## 3. Build commands

```powershell
# Host (desktop dev, stub platform) — fastest inner loop
cargo run -p reeda-ui --features platform-desktop

# PDFium: fetch the prebuilt DLL, then run (bundling; see scripts/package.ps1)
powershell -ExecutionPolicy Bypass -File scripts/fetch_pdfium.ps1
# Dev runs pick the DLL up via PDFIUM_LIBRARY_PATH or system search path
# (see docs/PDF_SPEC.md §7); `scripts/package.ps1` builds a portable
# release zip with pdfium.dll bundled next to reeda-ui.exe.

# Android debug APK (arm64)
cargo apk run -p reeda-ui                    # installs to device/emulator

# Release APK for a specific ABI
cargo ndk -t arm64-v8a -o target/ndk build --release -p reeda-ui
cargo apk build -p reeda-ui --release

# All ABIs (CI)
cargo apk build -p reeda-ui --release        # see BUILD_CI.md for the loop
```

## 4. Android project files (`android/`)

```
android/
  AndroidManifest.xml      # package id, minSdk, permissions, activity,
                           # foregroundService declarations, intent-filters
  res/                     # icons (mipmap), strings.xml (app_name), themes
  src/                     # Java/Kotlin shim (≤ ~100 lines, see ADR-008)
  jniLibs/<abi>/libpdfium.so   # added by CI (PDF_SPEC §7)
```

Key manifest elements:
- `<application android:label="Reeda" android:icon="@mipmap/ic_launcher"
  android:allowBackup="true" android:dataExtractionRules=...>`
  (backup policy: exclude index/cache, include db+books — see §8)
- Activity: exported, `android:screenOrientation` unset (rotation allowed),
  `configChanges` default (we handle re-layout ourselves).
- Intent filters: `VIEW`/`SEND` for `application/epub+zip`,
  `application/pdf`, `*/*` (share-to-Reeda, LIB-02).
- Permissions (minimum, requested at runtime):
  `POST_NOTIFICATIONS` (13+), `FOREGROUND_SERVICE`,
  `FOREGROUND_SERVICE_MEDIA_PLAYBACK`, `WAKE_LOCK`.
  **No storage permission** — SAF covers imports (LIB-01).

## 5. Runtime permission flow

- `POST_NOTIFICATIONS`: requested on first TTS/notification usage (on
  Android 13+), denial → narration works but notification silent; guide
  user to settings.
- Foreground service: started with `startForeground()` within 5 s of
  `NarrationState::Loading`; uses channel `narration` (importance LOW).

## 6. Signing (debug & release)

- Debug: `cargo-apk` auto-generates `debug.keystore`.
- Release: repo documents but **never commits** secrets:
  - `RELEASE_KEYSTORE` (base64, GitHub secret) → decoded in CI to
    `keystore.jks`; password/passwords as secrets.
  - `apksigner` from build-tools; alignment via `zipalign`.
  - APK signing config lives in `.github/workflows/release.yml` only
    (BUILD_CI.md §5).

## 7. Emulator & device testing

```powershell
# AVD via emulator CLI; or Android Studio
cargo apk run -p reeda-ui
adb logcat -s Reeda RustAndroidGlue     # app logs
adb shell am start -a android.intent.action.VIEW -d "file://..." -t "application/epub+zip"
```
- Golden device profiles: Pixel 6a (API 34), Pixel 8 (API 35), a low-end
  API 26 device for perf checks (PERFORMANCE.md).

## 8. Backup & data extraction rules

- `android:dataExtractionRules` (API 31+) + `android:fullBackupContent`
  (API 23–30): allow device backup of `reeda.sqlite` + `books/` +
  `covers/`; exclude `index/` (rebuildable, avoids backup bloat).
  Implemented in M7e: `android/res/xml/data_extraction_rules.xml` +
  `backup_rules.xml`, wired into the manifest.
- Cloud backup of reading progress is desirable; document DRM caveat
  (none — DRM-free only).
- Opt-out flag `android:allowBackup` per user setting (P1).

## 9. Known constraints & risks

- `cargo-apk` is not officially supported by Slint on all NDK versions;
  pin NDK r27 (verified combo documented in CI matrix).
- No Gradle = no native Play libraries (billing, licensing); not needed v1.
- Java shim growth is capped by review rule (ADR-008) — any shim change
  requires ADR update.
- Cold start: avoid JNI init on UI thread; lazy-init TTS/pdfium (PERFORMANCE).

## 10. Checklist (M0 exit)

- [ ] Host `cargo check` green on all crates
- [ ] `cargo apk run` shows app shell on Pixel 6a emulator
- [ ] CI artifact: signed? debug APK per ABI (BUILD_CI.md)
- [ ] Permission flows tested (notification, fg service)
- [ ] Backup rules verified via `adb backup` / device settings

# TODO — Reeda v0.2 stabilization & UI/UX alignment

> Working tracker for the current session. Incremental: items are checked off
> as they are completed. Scope: fix the Windows crash (<3 s) + dead Import
> button, audit architecture against standard Rust SE practice, align the UI
> with docs/UI_UX.md + docs/UI_UX-CONTEXT.md, and leave the app runnable.

## 1. Diagnosis (current bugs)

- [x] 1.1 Reproduce: run current `target/release/reeda-ui.exe`, observe launch,
      window, and console output.
      → App launches and stays responsive; no crash in testing; a Windows
      console window ships with the GUI release exe.
- [x] 1.2 Test the Import / "Import your first book" buttons (UI automation
      click) and confirm whether callbacks fire.
      → REPRODUCED: click fires nothing, no file dialog, app stays responsive.
- [x] 1.3 Identify root cause of the <3 s crash reported on Windows.
      → CLARIFIED BY USER: the <3 s crash is on the **Android phone**, not
      Windows. The dist/ APKs are v0.1.0 — built BEFORE the v0.1.1 Android
      crash fix (commit b190508: event log, exception clearing, ndk_context
      init). Phone was running the pre-fix APK. Action: review the v0.1.1
      fix code, rebuild a fresh APK, user sideloads to verify (see 5.2b).
- [x] 1.4 Identify why Add-book buttons appear inert ("design only").
      → ROOT CAUSE: the running exe was built Aug 19 from code that predates
      commit 43d439c (Aug 21) which introduced `on_import_requested`,
      `dep:rfd`, and the FileDialog. No rfd rlib exists in target/ — the
      callback was never registered, so clicks were silently dropped.
- [x] 1.5 Audit startup path (`run()` in reeda-ui/src/lib.rs): persistence
      load (`load_books` / `load_settings_from_db`), error surfacing.
      → FOUND: (a) `load_books()`/`load_settings_from_db()` never called —
      library/settings always empty at startup; (b) `Event::Error` /
      `Event::ImportFailed` discarded — ErrorDialog never opens; (c)
      `book-opened(id)` flips screens but never dispatches `OpenBook` —
      tapping a cover opens an empty reader; (d) startup theme hardcoded to
      Light, ignoring persisted setting.

## 2. Architecture review (all-Rust SE principles)

- [x] 2.1 Confirm frontend/backend separation (reeda-ui ↔ reeda-core command
      bus) matches docs/ARCHITECTURE.md; note violations.
      → YES, sound: reeda-ui never touches state directly; it dispatches
      Commands and renders StateSnapshots; engines live in their own crates
      (reeda-epub/reeda-pdf/reeda-search/reeda-tts); persistence in
      reeda-core (SQLite + BookStore). The violations were *unwired* paths
      in the UI layer (see §1.5), not architectural.
- [x] 2.2 Fix found violations — see §3 fixes.
- [x] 2.3 Verify workspace builds clean: `cargo fmt --check` ✓,
      `cargo clippy --workspace` ✓ (exit 0), `cargo test --workspace`
      ✓ 220 passed / 0 failed (PDFIUM_LIBRARY_PATH must point at
      third_party/pdfium/win-x64/pdfium.dll on this machine).

## 3. Fixes

- [x] 3.1 Crash root cause: Windows build from HEAD does not crash (launch +
      import + open verified). Phone <3 s crash matches the pre-v0.1.1 APK;
      fix exists in source (b190508) — fresh APK needed for verification
      (§5.2b).
- [x] 3.2 Dead Import buttons fixed:
      - root cause was a stale Aug 19 exe built before commit 43d439c added
        the rfd file-dialog wiring; rebuilt from HEAD.
      - NEW: Import FAB (+) added to the populated library view
        (LibraryScreen.slint) per UI_UX.md §3 — previously books could only
        be imported while the library was empty.
      - Android fallback registers on_import_requested and shows an explicit
        "not supported yet" error dialog instead of silence.
- [x] 3.3 Startup persistence wired: `load_books()` + `load_settings_from_db()`
      called in `run()`; persisted theme applied instead of hardcoded Light.
      Verified live: existing book reappears as "1 books" after restart.
- [x] 3.4 `Event::Error` / `Event::ImportFailed` now surface through the
      ErrorDialog (`show_error_events`) on import, search, reader-search,
      CLI import, and open-book paths.
- [x] 3.5 `book-opened(id)` now dispatches `Command::OpenBook` via new
      `open-book(string)` callback — previously tapping a cover only flipped
      screens to an empty reader. Verified: Scan PDF opens and renders.
- [x] 3.6 Release GUI builds no longer show a console window
      (`windows_subsystem = "windows"` gated to `not(debug_assertions)`).

## 4. UI/UX alignment (docs/UI_UX.md + UI_UX-CONTEXT.md)

- [ ] 4.1 Theme tokens match spec palettes (Hygge/Steel/Gilded) — verify or
      update Theme.slint; no raw hex outside tokens.
- [ ] 4.2 Library screen vs spec §3.1/§5.1: FAB done (§3.2); remaining gaps:
      search input field (currently icon-only), grid/list toggle, Continue
      Reading hero, bottom tab bar.
- [ ] 4.3 Reader chrome behavior: tap zones, auto-hide timing per spec §2.
- [x] 4.4 Touch targets ≥ 48 dp (FAB is 56 px; cards/buttons audited OK:
      top-bar icons 36 px are below 48 dp — tracked under 4.2).

## 5. Verification & handoff

- [x] 5.1 Deleted stale build artifacts (`target/`, `dist/` incl. the stale
      v0.1.0 APKs/zips) and rebuilt release from fixed source.
- [x] 5.2 Windows end-to-end verified via UI automation + pixel sampling:
      launch ✓ → import dialog opens (empty-state CTA and populated-state
      FAB) ✓ → EPUB import round-trip ✓ → book open renders PDF page ✓ →
      persistence across restart ✓. 220 tests green.
- [x] 5.2b Android: CI now builds signed debug APKs (new `apk` job in
      build-apk.yml, Linux port of scripts/build_apk.ps1). Fresh artifacts
      verified green (run 32489811103) and downloaded to dist/:
      reeda-0.1.1-arm64-v8a.apk + reeda-0.1.1-x86_64.apk.
      ON-DEVICE STEP (user): UNINSTALL the old Reeda first — the v1.0.0
      release APK was signed with a different debug key, so Android will
      refuse an in-place update ("App not installed"). Then sideload the
      arm64-v8a APK and confirm no <3 s startup crash.
- [x] 5.3 Full test suite green (`cargo test --workspace`, 220/220).
- [x] 5.4 Summary of changes + remaining risks:

## 6. Session summary

### Root causes found
| Symptom | Cause | Fix |
|---|---|---|
| Windows Import button dead ("design only") | Exe built Aug 19 predated commit 43d439c that added the rfd file-dialog wiring — callback never registered | Rebuild from HEAD (verified: dialog opens) |
| Book tap opened empty reader | AppRoot book-opened only flipped screens, never dispatched OpenBook | New open-book(string) callback → Command::OpenBook (verified: PDF renders) |
| Library/settings lost every restart | run() never called load_books()/load_settings_from_db() | Wired at startup; persisted theme applied too (verified live) |
| Errors invisible | Event::Error/ImportFailed discarded; ErrorDialog never opened | show_error_events() across import/search/open paths |
| No way to add 2nd book | Import only reachable from empty-library state | Always-visible 56px FAB per UI_UX.md §3 (verified) |
| Console window behind GUI | Default windows subsystem | windows_subsystem="windows" in release |
| Phone crash <3 s | v1.0.0 GitHub release ships stale v0.1.0 APKs built BEFORE the v0.1.1 crash fix (b190508) | Fresh v0.1.1 APKs via new CI job; sideload to verify |

### Architecture verdict
The all-Rust structure already follows standard SE practice: reeda-ui
(Slint frontend) ↔ reeda-core command bus (backend) with engines isolated
in reeda-epub/reeda-pdf/reeda-search/reeda-tts and persistence behind
BookStore/Database. All bugs were unwired UI-layer paths, not design flaws.

### Remaining risks / open work
- On-device confirmation of the Android startup fix still pending (user).
- UI_UX-CONTEXT.md gaps tracked in §4: search input field, grid/list
  toggle, Continue Reading hero, bottom tab bar, 36px top-bar icons below
  the 48dp target, theme palette tokens vs Hygge/Steel/Gilded.
- Tests require PDFIUM_LIBRARY_PATH=third_party/pdfium/win-x64/pdfium.dll
  locally (CI sets it automatically).

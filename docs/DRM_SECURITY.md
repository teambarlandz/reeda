# Security, Privacy & DRM — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17

## 1. Security posture

- **No network in v1** → no transport attack surface at runtime. All
  functionality is local.
- **Zero third-party telemetry/analytics/ads** by default (PRD FR-02).
  Optional crash reporting (opt-in, v1.1+) must be anonymous, minimal
  (stack + version + device class), deletable, and documented in the
  privacy policy.
- App data isolation: books/db/index live in app-private `filesDir`;
  no MediaStore writes; no broad storage permission (SAF only).
- **Zip-slip & decompression bombs** defended in `reeda-epub`
  (EPUB_SPEC §2); all external-file parsers fuzz-tested (TESTING.md §4).
- Inputs validated: manifest media-types whitelisted, image magic-byte
  verified, PDF opened only via PDFium (sandboxed by our size caps).

## 2. DRM stance (ADR-010)

- v1 supports only DRM-free EPUB/PDF. Adobe ACSM/ADE and FairPlay/DRM PDFs
  are **rejected with a clear message** at import: "This book is protected
  by DRM and cannot be opened." We do not attempt removal (illegal under
  most jurisdictions; out of scope forever).
- We do not implement any DRM scheme, watermarking, or license servers —
  Reeda is fully offline and DRM-free.

## 3. Privacy

- Reading positions, highlights, and notes never leave the device in v1.
- Android backup: only `reeda.db` + `books/` + `covers/` (PLATFORM.md §8);
  excluded: search index, caches. Settings control backup opt-out (P1).
- TTS: content spoken on-device; system TTS engines (including optional
  cloud ones the *user* chooses in Android settings) are the user's own
  choice — we surface a notice when a non-local engine is active (P1).
- Play Store data safety form answers documented in RELEASE.md §7 (shared
  with reviewers: "no data collected" for v1).

## 4. At-rest protection

- SQLite app-private (default sandbox). Optional passphrase encryption
  (SQLCipher) tracked as **P1** — design: `AppSettings.encryption` +
  prompt on unlock; keys via Android Keystore (`KeyStore` with
  AES/GCM, hardware-backed when available).
- Notes/highlights are plaintext in v1 (sandbox-only). Revisit with
  passphrase feature.

## 5. Hardening checklist

- `unsafe` audit: only JNI + pdfium FFI (TECHNICAL_DESIGN §8); every
  `unsafe` block has a SAFETY comment; CI lint denies undocumented unsafe
  (clippy `undocumented_unsafe_blocks`).
- Dependencies pinned (`Cargo.lock`) + `cargo audit` in CI weekly
  (vulnerability gate: fail on `high`).
- Release APK: signing via secrets (BUILD_CI.md §5), minify/R8 N/A
  (no Java UI), debuggable=false in release, no debug logging in release
  (log crate stripped via `release_max_level=off` for sensitive paths).
- Intent security: exported activity only handles VIEW/SEND with our MIME
  whitelist; content URIs opened once → copied (FR-04); no data URIs.
- Backup of exported files: users export intentionally (share sheet).

## 6. Threat model summary

| Threat | Mitigation |
|--------|-----------|
| Malicious book file (zip bomb, traversal) | EPUB_SPEC §2 guards + fuzz |
| Malicious PDF | PDFium + size caps + fuzz |
| Local attacker with device access | Android sandbox; optional SQLCipher P1 |
| Man-in-the-middle | No network (v1) |
| App tampering | Play signing (v1.1) + integrity API evaluation |
| Memory corruption | 100 % Rust, unsafe audit |

## 7. Compliance notes

- Accessibility (ACCESSIBILITY.md) is a Play requirement — tracked in M7.
- Privacy policy + data-safety form: drafted in M7 before store listing
  (RELEASE.md §7).
- No copyrighted content shipped; app never bundles books.

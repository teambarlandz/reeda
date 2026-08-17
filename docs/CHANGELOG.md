# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer](https://semver.org/) (see RELEASE.md).

## [Unreleased]

### Added

- Project scaffolding: Cargo workspace with `reeda-core`, `reeda-epub`,
  `reeda-pdf`, `reeda-search`, `reeda-tts`, `reeda-ui` crates.
- Full documentation set (see [TODO.md](TODO.md)): PRD, architecture,
  roadmap, technical design, ADRs, feature specs (EPUB, PDF, TTS,
  highlights, search), data model, platform/build/CI/testing/performance/
  accessibility/security/localization/release guides.
- Public GitHub repository.
- **M0 domain layer** (`reeda-core`): IDs, Book, Chapter, Annotation, AppSettings models;
  Command/Event enums; Platform trait with Desktop + Android stubs; App dispatch + snapshot;
  SQLite storage with migrations, CRUD queries, WAL mode.
- **M0 CI**: `ci.yml` (fmt, clippy, test, doc-build) and `build-apk.yml` (Android debug APK).
- **M0 Slint UI shell** (`reeda-ui`): Theme.slint (Light/Sepia/Night palettes), AppRoot.slint
  (Library↔Reader navigation), LibraryScreen.slint (empty-state onboarding + import button),
  ReaderScreen.slint (page canvas + chrome overlay), Dialogs.slint (error dialog).
- **M0 Android stubs**: UI-layer SAF picker, intent reader, permission request stubs gated
  behind `platform-android` feature.

### Changed

- (none yet)

### Fixed

- (none yet)

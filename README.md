# Reeda

**A book reader for Android — built 100% in Rust.**

Reeda is a mobile reading app in the spirit of Apple Books: import EPUB and PDF
books, read them with beautiful typography, highlight and annotate, search your
library, and have your books read aloud to you — all powered by a pure-Rust
stack (Slint UI on the Android platform).

## Status

> 🚧 **Pre-alpha — M4 full-text search complete.** Search your library from the
> Library screen (debounced, open-at-match) and within a book from the reader
> chrome (prev/next with wrap) — Tantivy-backed BM25 ranking with English
> analysis. M5 (PDF reader) is next.

| Area | State |
|------|-------|
| Planning & specs | Done |
| Rust workspace + domain layer | Done |
| CI (host + Android APK) | Done |
| Android app shell (M0) | Done |
| EPUB reader core (M1) | Done |
| Library & metadata (M2) | Done |
| Highlighting & notes (M3) | Done |
| Full-text search (M4) | Done |
| Play Store release | Not started |

## Feature map (Apple Books parity)

- Import & manage an EPUB/PDF library with cover art and metadata
- Reflowable EPUB reading with typography controls (font, size, margins, theme)
- PDF reading with zoom and pan
- Highlighting in 4 colors, inline notes, bookmarks
- Full-text search across the library
- **Read aloud** (text-to-speech) with playback controls
- Reading progress sync, per-book resume
- Dark/sepia/night themes

See the full product spec: [docs/PRD.md](docs/PRD.md)

## Repository layout

```
├── TODO.md                 # Master planning index — start here
├── docs/                   # All documentation (specs, design, ops)
├── crates/
│   ├── reeda-core/         # Domain models, services, app state
│   ├── reeda-epub/         # EPUB 2/3 parsing & rendering engine
│   ├── reeda-pdf/          # PDFium-based PDF rendering
│   ├── reeda-tts/          # Text-to-speech bridge (Android TTS)
│   ├── reeda-search/       # Full-text search index (Tantivy)
│   └── reeda-ui/           # Slint application frontend (Android target)
├── android/                # Android manifests & packaging (cargo-apk)
└── .github/workflows/      # CI/CD
```

## Building

> Full setup instructions: [docs/PLATFORM.md](docs/PLATFORM.md)

Prerequisities: Rust stable, Android NDK, `cargo-ndk`, `cargo-apk`.

```sh
# host-side check of the workspace
cargo check -p reeda-core

# Android debug APK (requires NDK + cargo-apk, see PLATFORM.md)
cargo apk run -p reeda-ui --release
```

## Documentation

Every document the project maintains is listed and tracked in
**[TODO.md](TODO.md)**. Highlights:

- [PRD — product requirements](docs/PRD.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)
- [Architecture decision records](docs/ADR.md)

## License

TBD — see [docs/LEGAL] placeholder in RELEASE.md. Open-source license under
discussion (M0).

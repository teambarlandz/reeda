# TODO — Reeda

> Master planning index. Every task below is traced to a milestone in
> [ROADMAP.md](docs/ROADMAP.md) and a spec in `docs/`. Statuses:
> `[ ]` = pending, `[~]` = in progress, `[x]` = done, `[-]` = cancelled.

---

## M0 — Foundations ✅

Done. See git log for full history.

---

## M1 — EPUB reader core ✅

**Exit criterion:** read a full Gutenberg EPUB, turn pages, change font/size/theme, resume exactly where you left off.

- [x] **M1.1** reeda-epub — Container/ZIP + OPF parsing (`container.rs`, `opf.rs`)
- [x] **M1.2** reeda-epub — Nav/TOC parsing (`nav.rs`)
- [x] **M1.3** reeda-epub — XHTML → document model (`xhtml.rs`, `document.rs`)
- [x] **M1.4** reeda-epub — CSS subset engine — deferred to M2 (basic reading works without styled CSS)
- [x] **M1.5** reeda-epub — CFI position model (`cfi.rs`)
- [x] **M1.6** reeda-epub — Paginator (`paginator.rs`)
- [x] **M1.7** reeda-core — Wire EPUB open into App dispatch (`reader.rs`)
- [x] **M1.8** reeda-ui — Reader screen integration (Slint bindings + main.rs)
- [x] **M1.9** Tests — 84 total (40 reeda-core, 40 reeda-epub, 4 others)
- [x] **M1.10** Update TODO.md + CHANGELOG.md + commit/push

---

## M2 — Library & metadata ✅

**Exit criterion:** import 10 books, covers show, recent/continuar correct, delete works.

- [x] **M2.1** Import pipeline: file storage + SHA-256 dedup (`store.rs`, BookStore)
- [x] **M2.2** Library grid Slint UI (`BookCard.slint`, LibraryScreen model, main.rs)
- [x] **M2.3** Cover extraction from EPUB (`extract_cover_bytes`, cover stored on import)
- [x] **M2.4** Progress save/restore (page index persisted on turn/close, restored on open)
- [x] **M2.5** Metadata editing (`EditMetadata`, `MetadataDialog`, persists to DB)
- [x] **M2.6** Settings screen v1 (`SettingsScreen.slint`, theme picker, font size, line height)
- [x] **M2.7** Tests + integration — SQLite persistence wired into App (books, chapters,
  position, metadata, settings); 103 tests total (58 core, 42 epub, 3 others)
- [x] **M2.8** Update TODO.md + CHANGELOG.md + README.md + commit/push

**Notes / deferred:** cover rendering as image in grid (placeholder initial shown —
cover_path stored, image display pending M3); "Continue reading" section and sort
buttons (M2.2) not yet in UI; CSS subset engine (M1.4) still deferred.

---

## M3 — Highlighting & notes ✅

**Exit criterion:** highlight a passage, kill the app, relaunch → highlight + note
intact at correct position with different font size. Met: annotations persist to
SQLite (insert/update/soft-delete/list-on-open) and re-render after restart;
HIL-08 invariance covered by tests.

- [x] **M3.1** reeda-epub — Selection + intersection engine (`selection.rs`):
  - `GlobalRange` (block_start, char_start, block_end, char_end over global block
    sequence) + `is_valid()`
  - `to_cfi()` / `from_cfi()` — CFI anchoring via `cfi::Cfi` (round-trip, orphaned
    CFI → None)
  - `intersect_range_with_page()` → `Vec<ClippedSegment>` (page char/block clipping)
  - Tests: intersection math, cross-block, page-clipping, CFI round-trips
  - Fix `find_page_for_cfi` (reader.rs) to use global `block_index` (not spine_index)
- [x] **M3.2** reeda-ui — Highlight rendering in reader:
  - `build_page_lines` → `Vec<Vec<LineRun>>` (plain/highlight runs per visual line)
  - ReaderScreen: per-line runs, translucent colored background + underline + note
    dot (HIGHLIGHTS_SPEC §3); color-index ints mapped to Theme brushes in .slint
  - Tap highlight → popover: edit color (4 swatches) / delete
  - Wire AddHighlight/EditHighlight/DeleteAnnotation commands to UI callbacks +
    snapshot
- [x] **M3.3** reeda-core + reeda-ui — Notes + notes list:
  - `AddNote` wired end-to-end (attach to highlight / standalone)
  - NotesScreen: per-book list of highlights+notes grouped by chapter, snippet,
    color chip, date; tap → jump (HIL-06)
- [x] **M3.4** reeda-ui — Bookmarks:
  - Chrome ribbon toggle button; icon state from page-start CFI (filled when match)
  - Bookmarks list screen: tap → jump, delete (uses `ToggleBookmark`)
- [x] **M3.5** reeda-core — Export highlights/notes (Markdown):
  - `export_markdown(book, doc, annotations)` per HIGHLIGHTS_SPEC §4 format
    (grouped by chapter, spine order, notes inline)
  - NotesScreen "Export" button → desktop writes `annotations.md` next to the
    book file; Android logs path (ACTION_SEND share sheet deferred)
- [x] **M3.6** reeda-core — Persistence + invariance + docs:
  - Wire annotation CRUD to SQLite in App commands (insert/update/soft-delete,
    list-on-open) — best-effort with warning logs
  - HIL-08 font-size invariance tests (same CFI → same highlighted text across
    line-wrap widths)
  - Restart persistence integration test (highlight survives App recreation)
  - 131 tests total (74 core, 54 epub, 3 others)
  - Update TODO.md + CHANGELOG.md + README.md + commit/push

**Notes / deferred:** Android share sheet for export (ACTION_SEND) — logs path
for now; parsed-doc registry is in-memory, re-parsed on next import/open cycle
after restart (annotations themselves are fully persistent).

---

## M4 — Full-text search (2 weeks) — _done_

**Exit criterion:** 50-book fixture library; query finds ranked results < 1 s.
→ Met in release: `cargo test --release -p reeda-search --test perf_fixture`
builds 52 books (80 k+ words) in ~3.4 s (~6.5 s/100 books, < 10 s budget) with
query p95 well under 1 s.

- [x] **M4.1** reeda-search — Tantivy index core (`index.rs`):
  - Schema per SEARCH_SPEC §2: `book_id` (raw term), `spine_index`/`block_index`/
    `char_offset` (u64, fast), `title` (TEXT), `body` (TEXT), `chapter_title`
    (stored), `language` (stored)
  - `IndexManager`: open/create at app-private path, version stamp meta file,
    `index_book()` (replace-then-add per book_id), `delete_book()`, `search()`
    (BM25, limit 200), `rebuild()`
  - Snippet generation via Tantivy snippet highlighter; hit → `GlobalRange`
    locator CFI (open-at-match, SEA-03)
  - Tests: insert → search finds, phrase query, per-book filter, delete-by-book,
    rebuild idempotence, locator round-trip
- [x] **M4.2** reeda-search — Analyzer + query layer (`query.rs`):
  - Lowercase + unicode segmentation; English stopwords; phrase + AND default
  - Title boost 2.0 in query construction (spec §5)
  - Tests: relevance sanity (expected top-N), phrase/AND behavior, empty/whitespace
    query handling, cap at 200
- [x] **M4.3** reeda-core — Index orchestration in App:
  - Index path under data dir; `index_book` on ImportFinished, `delete_book` on
    DeleteBook (spec §4 lifecycle)
  - `SearchIndex` handle in App (lazy open), status flags (indexed/unindexed)
  - Tests: import → query finds content, delete → removed from results,
    re-import replaces documents
- [x] **M4.4** reeda-core — Search command/events/snapshot + open-at-match:
  - `Command::Search { query, scope }` (library-wide / in-book), `Event::SearchResults`
  - `StateSnapshot.search_entries` (book title, chapter, snippet, locator) +
    `transient_highlight` (CfiRange rendered like an annotation, not persisted)
  - Open-at-match: OpenBook + JumpTo the hit locator; transient highlight shown
    and cleared on page turn
  - Tests: search results in snapshot, open-at-match jumps + renders transient
    highlight, transient cleared on turn
- [x] **M4.5** reeda-ui — Library search screen:
  - Search icon on Library → full-screen SearchScreen.slint (query-as-you-type,
    debounce 250 ms, results grouped by book with chapter headings)
  - main.rs: debounce timer, Search command dispatch, tap result → open at match
  - Tests/build: UI compiles; manual smoke
- [x] **M4.6** reeda-ui — In-reader search overlay:
  - Chrome search icon → overlay panel (hits list + prev/next arrows)
  - Term highlight synced with reader via transient_highlight
- [x] **M4.7** Performance fixtures + docs:
  - Synthetic 50-book corpus fixture (multi-language, long book, diacritics,
    empty book); index-build timing sanity (< 10 s/100 books), query p95 < 1 s
    smoke test (release profile)
  - Update TODO.md + CHANGELOG.md + README.md + commit/push

**Deferred (spec P2):** per-language stemming/analyzers (en-only v1), fuzzy/typo
tolerance, recent-searches list, CJK-specific prefix/substring, in-book search
scope toggle UI (library-wide first).
## M5 — Read aloud / TTS (3 weeks) — _done_

**Exit criterion:** read a chapter aloud, lock the phone, control from
notification, word highlight tracks speech. — Desktop + unit coverage green
locally; device-dependent items (notification controls, lock screen, audio
focus) verified on emulator/device as a follow-up.

- [x] **M5.1** reeda-tts — Chunker (`chunk.rs`):
  - Sentence boundary detection (`. ! ? …` + closing quotes) with abbreviation
    guard list (Mr., Dr., …); chunk max 4000 chars, boundary forced at sentence,
    paragraphs split only when over limit
  - Clean: soft hyphens, `&nbsp;`, control chars; skip footnotes/captions/TOC
    markers (document-model filtering) — plain text from DocumentModel
  - Chunk → CFI mapping via GlobalRange (block_index + char offsets, spec §3)
  - Tests: boundaries + abbreviation guard, limits, cleaning, CFI mapping
- [x] **M5.2** reeda-tts — Engine trait + narration state machine (`engine.rs`):
  - `TtsHost` trait (speak/stop/pause/resume/set_rate/set_pitch; callbacks
    on_start/on_done/on_error/on_range_start) + `FakeTtsHost` (desktop/tests)
  - NarrationEngine: Idle/Loading/Speaking/Paused/Error, queue depth 2,
    monotonic utterance ids, retry policy (3 consecutive errors → Paused+Error)
  - Tests: all transitions, queue prefetch, callbacks, retry policy
- [x] **M5.3** reeda-core — Narration wiring (`narration.rs` in App):
  - StartNarration builds chunks from current chapter (ParsedDocRegistry),
    auto-advance to next chapter; `NarrationSkip { delta }` chapter fwd/back
  - WordHighlight → transient_highlight (cyan) + WordHighlight event; auto page
    turn when chunk CFI passes page end (TTS-05)
  - SetTtsSpeed/SetTtsPitch persist to AppSettings; stop/close-book clears
  - Snapshot: narration state + speed + current position; `TtsHost` injectable
    (FakeTtsHost in tests/desktop)
  - Tests: start/pause/resume/stop with fake host, word-highlight events, auto
    page turn, next-chapter advance, retry → Error state
- [x] **M5.4** reeda-ui — Reader TTS bar:
  - Bottom bar in ReaderScreen: play/pause, stop, skip chapter fwd/back, speed
    chip (cycles 0.5–2.5); visible from narration state in snapshot
  - main.rs wiring: commands → dispatch, bar state from snapshot; word highlight
    via transient_highlight (already renders)
  - Desktop smoke: fake host drives highlight + page turns
- [x] **M5.5** Android TTS bridge (feature-gated `platform-android`):
  - `android_bridge.rs` (jni crate): TextToSpeech init, speak/stop/setSpeechRate/
    setPitch, UtteranceProgressListener (onStart/onDone/onError/onRangeStart)
    marshalled to engine; Java shim `android/TtsShim.java`; foreground-service
    media notification + audio focus + wake-lock stubs per TTS_SPEC §2
  - CI compile check (`build-apk.yml`); device verification follow-up
  - Steps: (1) Cargo.toml: optional `jni` + `ndk-context` deps, wire
    `platform-android` feature; (2) `src/android.rs`: `AndroidTtsHost` —
    vm/activity from `ndk_context`, GlobalRef to TtsShim, binder-thread event
    queue drained by `poll()`, `#[no_mangle] extern "system"` JNI callback
    symbol; (3) `android/src/io/reeda/app/TtsShim.java` (≤100 lines, minSdk 26
    `onRangeStart`); (4) reeda-core `set_tts_host` → `pub`; (5) reeda-ui:
    optional reeda-tts dep, `android::create_tts_host`, main.rs init on
    `platform-android`; (6) build-apk.yml: APK job uses
    `--no-default-features --features platform-android`, add tts compile-check
    job; (7) verify `cargo check -p reeda-tts --no-default-features --features
    platform-android` on host + full test suite, commit/push
- [x] **M5.6** Tests + docs + close:
  - Full workspace tests; update TTS_SPEC status, CHANGELOG.md, README.md
    status; TODO checkboxes; commit/push

**Deferred (spec P2):** PDF narration (TTS-07), locale-aware abbreviation lists,
skip-back/forward −15 s/+15 s within chunk, per-book voice settings.
## M6 — PDF support (3 weeks) — _not started_
## M7 — Hardening & ship (3 weeks) — _not started_

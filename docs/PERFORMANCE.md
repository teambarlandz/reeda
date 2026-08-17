# Performance Specification — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Budgets are gate criteria for milestones; measured on a mid-range device
> (P0 tier, TESTING.md §6).

## 1. Budgets (summary)

| Metric | Budget | Where measured |
|--------|--------|----------------|
| Cold start → library | < 900 ms (50th), < 1.5 s (95th) | Pixel 6a, release build, warm disk cache |
| Cold start → reader (last book) | < 1.2 s | same |
| Page turn (EPUB) | < 33 ms p95 (render), < 90 ms p95 (incl. pagination hit) | reader, avg chapter |
| Typography change re-pagination | < 150 ms p95 (avg chapter), < 400 ms p95 (long chapter) | Aa panel |
| Resume after restart | < 600 ms to first page | — |
| Search 50-book library | < 1 s p95 end-to-end | search screen |
| Index build | ≤ 10 s per 100 books (background, battery-aware) | import |
| PDF page first raster | < 250 ms p95 @ fit-width | PDF viewer |
| PDF page cached raster | < 8 ms p95 (blit) | scroll |
| Heap (EPUB, 1 book open) | ≤ 200 MB | profiler |
| Heap (PDF, raster cache) | ≤ 128 MB cache + pages ≤ 256 MB total | profiler |
| Battery | TTS narration drain < 15 %/h; reading < 5 %/h | device test |

## 2. Start-up plan

1. `main` → minimal Slint window paints **before** any I/O (theme + splash).
2. Background: open DB (prepared statements), load settings, list library
   (first 50 rows for grid), async cover decode (WebP, decoded at grid
   size, cached).
3. Reader resume: paginate last book in worker; render first page once
   ready (skeleton shimmer until then).
4. Lazy init: PDFium binds on first PDF open; TTS initializes on first
   narration; Tantivy opens at import/search (never at boot).
5. JNI on UI thread → prohibited (lint rule, PLATFORM.md §9).

## 3. Rendering (EPUB)

- Pagination runs off-thread; **single source of truth for metrics** = the
  actual Slint text measurement pass (EPUB_SPEC §6) — we feed paginator the
  same font stack; no double shaping.
- Long chapters (> 200 pages): paginate in ~64-page slices, render window
  = current page ± 1 (absolute layout, no full-chapter Slint elements).
- Layout cache LRU keyed `(book_id, layout_hash)` — font/size/width/margin
  changes only invalidate the affected book.
- De-dup: pages re-rendered only when their slice's `Pages` hash changes.

## 4. Images (EPUB)

- Decode at display size × DPR (never full-size); WebP/JPEG decode in
  worker; small images (< 32×32) upscaled once.
- Per-book image cache ≤ 32 MB LRU; memory warning → clear.

## 5. PDF pipeline

- Raster at `scale = ceil(display_dpr × zoom)` capped 4096 px/axis
  (PDF_SPEC §3); bucket-based cache (fit-width, 100 %…500 %).
- Prefetch neighbors on idle; eviction on memory pressure
  (`onTrimMemory` → drop 50 %).
- Scroll view uses chunked bitmap uploads (avoid full-view textures).

## 6. Search & indexing

- Index build in background worker with progress; debounce 2 s; skip while
  TTS active (battery); pause on app background (resume on foreground).
- Query hot path: single-threaded Tantivy read, cap 200 results, snippet
  generation bounded (first 2 matches).

## 7. Database & storage

- WAL + `synchronous=NORMAL`; progress flush ≤ 100 ms writes; prepared
  statements reused; `busy_timeout=5000` with retry-log.
- Book files on flash: extracted EPUB folder reads are page-sliced; PDF
  reads from `copy-on-write` mmap (pdfium handles internally).

## 8. Memory management

- `onTrimMemory` levels: TRIM_MEMORY_RUNNING_* → clear LRU caches
  (PDF raster 100 %, pagination 50 %, images 100 %).
- OOM-guard: allocations in raster paths checked; cache budgets enforced
  per byte-count (not entry-count).
- Profile targets: RSS ≤ 300 MB during 2-h reading session with TTS.

## 9. Tooling & measurement

- Host: `cargo bench` micro-benchmarks (paginator, CFI, chunker) with
  baseline CI job (regression gate > 15 %).
- Android: Perfetto traces around page-turn/TTS-start; `dumpsys
  meminfo`; `adb shell dumpsys batterystats` for drain; startup via
  `adb shell am start -W`.
- CI gate: budgets from §1 enforced by `scripts/bench_android.ps1`
  (release build, Pixel 6a; fails PR on regression).

## 10. Known risks

- Slint text measurement cost with huge chapters → slice strategy (§3).
- WebP decode on low-end devices → hardware decoder path P1.
- Tantivy first-query latency on cold index → pre-warm on app foreground
  after import (background).
- PDFium raster on x86 emulator is slow — perf gates run on arm64 device.

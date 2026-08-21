# PDF Specification — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-pdf` (PDFium via `pdfium-render`, ADR-006).

## 1. Supported formats

- PDF 1.x and 2.0 (via PDFium), including encrypted PDFs with empty user
  password (owner-password only). Password-protected PDFs → clear error
  dialog in v1 (password prompt is P2).
- Linearized and non-linearized; incremental updates handled by PDFium.
- DRM/DRM-encrypted PDFs (Adobe DRM on PDF) → rejected with clear message
  (ADR-010).

## 2. Document model

- `PdfDocument { path, page_count, page_sizes, outline }` opened lazily.
- Page size: points → px at 96 dpi base (72 pt = 96 px), used for
  fit-to-width and aspect-correct rasterization.
- Outline: PDF bookmarks tree → Slint list (PDF-04). Implemented (M6.5):
  `reeda-pdf::outline::extract_outline` flattens the tree pre-order into
  `{ title, page_index, depth }`; `PdfView.outline` exposes jumpable entries
  in the snapshot; reader chrome shows an outline panel ("≡" toggle) with
  depth indentation and tap-to-jump. Links inside pages (GoTo/URI) → P2
  (tap-to-navigate).

## 3. Rasterization

- `pdfium-render` `PdfBitmap` at target size:
  `render_scale = device_pixel_ratio × zoom × fit_factor`.
- Target max dimension cap: 4096 px/axis (memory guard, PERFORMANCE.md).
- **Night/sepia theme**: render-time filter — luminance-preserving tint
  applied to RGBA (night = invert-ish dark, sepia = warm curve); PNG alpha
  composited on theme background. Re-render only on theme/zoom change.

## 4. Viewport & gestures

- v1: adaptive reading mode — default continuous vertical scroll with pinch
  zoom (0.25×–5×), double-tap zoom toggle (fit-width ↔ 100 %), fit-to-width
  default; layout switches to single-pagepread mode with 3D page curl physics
  and zero UI chrome when user taps center of viewport (per UI_UX-CONTEXT.md §3.2).
- Page transition: pages are fixed-size rectangles stacked vertically in scroll
  mode; in page-curl mode, transition uses dynamic 3D conical page curl shader
  (wgpu/Skia) with dynamic displacement during drag gestures.
- Page indicator overlay (bottom, auto-hide), jump dialog with page number.
- Landscape: no reflow — PDF pages just fit width (aspect preserved); in
  page-curl mode, landscape presents facing-page spread with 3D curl between
  spreads.

## 5. Caching & memory (PDF-01 budgets)

- LRU raster cache keyed `(page, scale_bucket, theme)`, budget **≤ 128 MB**
  (configurable via settings; default). Buckets: fit-width, 100 %, 150 %,
  200 %, 300 %, 400 %, 500 % — zoomed pages reuse nearest bucket ≥ target.
- Prefetch: next/prev pages at current bucket on idle (worker pool).
- Eviction: LRU; on OOM signal → drop cache to 50 % and re-raster on demand.

## 6. Text features (P2 in v1, spec now)

- Text extraction per page (`PdfPage::text`) with word rectangles
  (`text_rects`) → powers: selection+copy (PDF-05), search (PDF-06),
  highlight overlay (PDF-07), TTS (TTS-07).
- Highlight overlay: PDFium `PdfAnnotation` highlight (native, persists in
  file copy) OR internal overlay stored in our DB (survives re-import but
  not other apps' edits). Decision: **overlay in our DB** for v1.1
  (no file mutation, no PDFium annotation API risk); revisit with ADR if
  needed.
- Search: PDFium text search API (`PdfTextSearch`) for in-page matching;
  results map to page + word rects → open + highlight.

## 7. PDFium binary supply (CI + builds)

- `pdfium-render` requires `libpdfium.so` per ABI. Strategy:
  - Fetch prebuilt from `bblanchon/pdfium-binaries` release (pinned
    commit/tag + sha256) in CI; vendor into `third_party/pdfium/`.
  - Local dev: script `scripts/fetch_pdfium.ps1` (Windows) / `.sh`
    (Linux/macOS) with hash verification.
  - Cargo feature `static` links; default = dynamic load via
    `Pdfium::bind_to_library`.
- **Desktop packaging (M7 decision):** bundle the DLL with the app — no
  runtime download. `scripts/package.ps1` builds the release binary and
  copies `pdfium.dll` next to `reeda-ui.exe` into a portable zip. Windows
  DLL search order finds the application directory first, so
  `Pdfium::bind_to_system_library` (the fallback in `reeda-pdf::document`)
  loads it with zero configuration; verified by the reeda-pdf test suite
  run without `PDFIUM_LIBRARY_PATH` and by the packaged-exe smoke test.
- Android: copy per-ABI `.so` into `android/src/main/jniLibs/<abi>/` during
  build (cargo-apk `additional-libs` config).

## 8. Fixtures & tests

- Fixtures: PDF 1.4/1.7/2.0 files, multi-page with outlines, images, scanned
  (no text layer), encrypted-empty-password, malicious (huge page sizes,
  zero pages, broken xref).
- Tests: open/count, raster golden (hash of page pixels at fixed scale),
  outline structure, memory budget eviction, zoom bucket selection,
  theme filter output sanity.

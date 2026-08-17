# UI/UX Design — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Implementation: `reeda-ui` (Slint). Principles: Apple Books-class reading
> experience with one-handed operation and zero chrome during reading.

## 1. Design language

- **Typography-first**: reading surfaces are text on calm backgrounds.
  Type scale: 14–24 pt reading sizes, system/bundled fonts (Noto Serif for
  book text default; Sans for UI).
- **Themes** (PRD EPR-05): Light (#F7F4EC paper), Sepia (#F1E8D8), Night
  (#101418). UI accent: single reading-green `#2E8B57`; highlight colors:
  yellow `#FFE94D`, green `#9FE8B0`, blue `#A8D8F0`, pink `#F7B7D6`
  (alpha ~0.3 over text).
- Components: Slint custom widgets (no external kit in v1) with a Material3
  -inspired layout: 48 dp touch targets, 12 dp radius cards, 16 dp grid
  margins, elevation via soft shadows.
- Motion: 150–250 ms ease-out transitions; page-turn uses a 200 ms slide
  (rightward for back) + optional curl effect P2.
- Haptics: subtle on selection/highlight actions (P1).

## 2. Navigation model

```
┌─ Root (single activity, Slint stack) ──────────────┐
│ Library ──▶ Reader(EPUB | PDF) ──▶ (chrome: TTS, highlights,
│   │            settings, bookmarks, search)
│   └──▶ Search (full-screen, overlay)
│   └──▶ Settings (bottom sheet / page)
│   └──▶ Onboarding (first run: empty library CTA)
└─────────────────────────────────────────────────────┘
```
- Back behavior: reader → library; in reader: back = close chrome first,
  then exit book (with position saved).
- Reader chrome (auto-hide after 2.5 s idle): top = back, book title, TOC;
  bottom = progress bar, font/theme (Aa), TTS, bookmarks, search, highlights.

## 3. Library screen

- Grid of covers (2 columns portrait, 3–4 landscape); rows "Continue
  Reading" (horizontal scroll, covers + progress ring), "Recent", shelves
  (P1).
- Long-press cover → action sheet (Open, Edit metadata, Move to shelf[P1],
  Delete[confirm], Export backup[P1]).
- Top bar: app name, search icon, import (+) FAB (bottom-right).
- Empty state: illustration + "Import your first book" button + hint about
  file types.
- Import UX: progress dialog with stage name + cancel; error dialog with
  cause + "Try again".

## 4. Reader — EPUB

- Full-bleed text; margins configurable (12–40 dp).
- **Tap zones** (UX-01): left 25 % = previous, right 25 % = next, center
  50 % = toggle chrome. Zones configurable (left/right swap).
- Gestures: swipe left/right page turn (with edge bounce); long-press =
  selection; two-finger tap = chrome; pinch = font size (P2).
- **Aa panel** (bottom sheet): font family, size slider (14–24 pt),
  line-height (1.2–1.9), margins, justify toggle, theme (Light/Sepia/Night/
  Auto), night auto at 22:00 (P1).
- Progress: bottom bar shows % + chapter; tap → chapter jump sheet.
- TTS bar (when narrating): appears above bottom bar; word being read
  underlined in accent color; auto-scroll.

## 5. Reader — PDF

- Vertical continuous scroll; pinch zoom; page indicator pill (bottom
  center); outline button in chrome; jump-to-page sheet; night/sepia filter
  applied at render (PDF_SPEC §3). Zoom lock-on-chrome (chrome hidden when
  zoomed in, P2).

## 6. Selection & highlights (EPUB)

- Long-press word → selection handles (top/bottom drag dots), popover
  (floating, above selection): Highlight ▸ (4 colors), Note, Copy.
- Highlight tap → popover: color swatches, Note icon, Delete (with undo
  snackbar 4 s), Share (P2).
- Highlights screen: grouped list, color chips, snippet, tap → jump.

## 7. Search UX

- Library search: full-screen; query field; results grouped by book
  (book cover thumb + chapter + snippet with `<mark>` accent); recent
  searches below; no-results state with suggestion.
- In-reader: overlay panel; hit list; prev/next arrows; transient highlight.

## 8. Settings screen

Sections: Reading (theme default, typography defaults, tap zones,
line-height), Narration (speed, pitch, wake-lock, auto-continue chapters),
Library (backup/restore, clear cache), About (version, licenses, privacy
policy link). All changes apply live + persist.

## 9. Accessibility (summary — see ACCESSIBILITY.md)

- TalkBack: all interactive elements have labels/roles (Slint accessibility
  API); reading text exposed as readable; selection a11y actions
  (copy/select all).
- Font scaling: respects system font scale (1.0–2.0) for UI; reading font
  size independent (user-controlled, persisted).
- Contrast: all themes ≥ 4.5:1 for body text; focus rings visible.
- Reduced motion: respects `MotionAccessibility` (disable page-turn slide).

## 10. Orientation & rotation

- Library: portrait + landscape layouts (responsive grid).
- Reader: portrait default; landscape allowed (re-pagination, position
  preserved — FR-05). PDF: rotation re-fits width.
- State: rotation preserves reader position, selection (cancelled safely),
  TTS (continues, TTS_SPEC §7).

## 11. Slint component map (`reeda-ui/ui/`)

```
AppRoot.slint          # stack + theme provider
  LibraryScreen.slint  # grid, shelves, FAB, dialogs
  ReaderScreen.slint   # page canvas (Text / ScrollView), chrome overlays
    AaPanel.slint · SelectionPopover.slint · TtsBar.slint · ProgressBar.slint
  SearchScreen.slint · HighlightsScreen.slint · BookmarksScreen.slint
  SettingsScreen.slint · Dialogs.slint (confirm/error/progress)
  Theme.slint          # palette tokens, typography tokens
```
Naming: kebab-case files, PascalCase components; all visuals via tokens
(`Theme.slint`), no raw hex in screens.

## 12. Design verification

- Design review checklist in PR template (contrast, touch targets,
  empty/error states, rotation, a11y labels).
- Golden screenshots: per-screen, per-theme, three device sizes
  (360×800, 412×915, 800×1280) on emulator CI (TESTING.md §5).

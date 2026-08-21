# Accessibility Specification — Reeda

> Status: draft · Version: 0.1 · Owner: @teambarlandz · Last updated: 2026-08-17
> Reeda targets full WCAG 2.1 AA on Android (TalkBack), with reading-specific
> accommodations. Owner: everyone (a11y is in DoD).

## 1. Principles

1. Every interactive element is reachable + labeled (Slint accessibility
   API: `accessible-role`, `accessible-label`, `accessible-description`,
   `accessible-action`).
2. Reading is possible with: TalkBack on, 200 % font scale, high-contrast
   theme, and reduced motion.
3. No information is conveyed by color alone.
4. Touch targets: 24×24 px for action bars; 16×16 px for inline indicators;
  tappable text highlighted separately.

## 2. TalkBack integration

- Library: grid items = buttons with label "Title, by Author, 42 % read";
  import FAB labeled; long-press menus expose actions.
- Reader: reading area exposed as **readable** element — TalkBack reads
  the current page's text (via Slint a11y text); the page is one scrollable
  unit; page-turn actions = a11y actions ("next page", "previous page").
- Selection: TalkBack "select word/paragraph" actions map to our selection
  engine (a11y action → CFI range); highlight popover elements labeled.
- Chrome auto-hide: Reading overlays (header/scrubber) fade after 2.5 s of
  tap/drag inactivity; Floating Action Hub dims to 30 % opacity after 4.5 s.
  When TalkBack is active, chrome remains fully visible — auto-hide is
  suspended for accessibility, restored after TalkBack session ends.
- Progress bar: `progress` role + percentage announced on change.
- TTS bar: play/pause/stop/speed announced; narration start announces
  chapter title.

## 3. Typography & contrast

- UI font scale 1.0–2.0 respected (system); reader font size independent
  but defaults follow system scale (user can override per-book).
- Contrast: body ≥ 4.5:1 (Light/Sepia), Night ≥ 7:1 for text; UI text
  ≥ 4.5:1; focus indicators ≥ 3:1 against adjacent. Verified by golden
  checker (script over Theme.slint tokens).
- Line length ≤ 90 chars at max scale; line-height ≥ 1.4; spacing
  preserved under magnification (no clipping — golden test at 200 %).

## 4. Motion & interaction

- `reduced-motion` (Slint/MotionAccessibility): page-turn becomes instant;
  no slide animations; selection popover static.
- Haptics: off when system haptics off (respect global).

## 5. Input methods

- Full keyboard/D-pad navigation (Slint focusable list); TalkBack swipe
  gestures map cleanly; double-tap-and-hold actions avoided.
- Alternative to pinch zoom (PDF): zoom in/out buttons in chrome (≥ 44 dp).

## 6. Testing & checks (in DoD)

- Automated: `scripts/a11y_check.rs` (CI) — scans `.slint` for unlabeled
  interactive elements, contrast tokens vs thresholds, touch-target sizes.
- Emulator: TalkBack walkthrough of all screens (scripted, recording);
  font-scale 2.0 golden screenshots; reduced-motion screenshot parity.
- Manual checklist: reading session with TalkBack end-to-end (import → read
  → highlight → TTS) in RELEASE.md §5.

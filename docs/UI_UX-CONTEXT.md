# UI/UX & System Architecture Specification: Apple Books-Style E-Reader (Rust + Slint)

## 1. Executive Summary & Design Philosophy
This document serves as the comprehensive UI/UX context and technical architecture specification for a cross-platform EPUB/PDF reader built in **Rust** using **Slint**. The application mirrors the refined functionality of Apple Books while introducing an adaptive, single-thumb mobile layout and a customizable Floating Action Hub.

### Core Pillars
1. **Typography-First Reader Canvas**: Zero UI chrome during active reading; absolute focus on body text reflow and legibility.
2. **Tactile Micro-Interactions**: Physics-driven 3D page curl, interactive text selection overlays, and gesture-aware toolbars.
3. **Adaptive Dual-Layout Engine**: Context-aware UI layouts that dynamically reconfigure between Desktop/Tablet (spaced top bar) and Mobile (thumb-optimized stacked header).
4. **Pin-to-Hub Accessibility**: User-customizable floating tools that group, expand, and dim automatically to maximize screen real estate.
5. **Rust-Native Architecture**: High-efficiency text layout, multi-threaded rendering, and persistent state management via SQLite.

---

## 2. Visual System & Design Tokens

### 2.1 Color Palettes

#### Day Theme (Scandinavian Hygge)
*Ideal for everyday reading in standard lighting environments.*
* **Canvas / Background**: `#F8F8F8`
* **Surface / Card Fill**: `#F0EAD6`
* **Primary Accent / Warm Tone**: `#D4C1A5`
* **Muted Text / Subtle Accent**: `#A28E78`
* **Body Text**: `#1A1A1A`

#### Focus Theme (Industrial Steel)
*Ideal for high-contrast environments and outdoor daylight.*
* **Canvas / Background**: `#CED4DA`
* **Surface / Container**: `#ADB5BD`
* **Secondary Surface / Border**: `#6C757D`
* **Dark Accent / Text**: `#343A40`
* **Body Text**: `#111625`

#### Night Theme (Gilded Indulgence)
*Designed for dark environments to eliminate glare and eye strain.*
* **Canvas / Night Background**: `#1A1A1A`
* **Surface / Card Fill**: `#2E3A4F`
* **Secondary Background**: `#EBEBEB`
* **Warm Accent / Highlight**: `#F7E7CE`
* **Body Text**: `#E5E7EB`

### 2.2 Iconography Rules
* **Style Standard**: Minimalist **Outline (Line Style, Style 1)** or **Two-Tone (Style 3)**.
* **Stroke Consistency**: Uniform 1.5px to 2.0px vector paths.
* **Sizing**: Standardized 24x24px grid for action bars; 16x16px for inline indicators.
* **Prohibited**: No 3D, skeuomorphic, glossy, or heavily filled glyphs within reading controls.

### 2.3 Typography Engine Rules
* **Serif Engine (Default Body)**: *Georgia*, *Palatino*, or *Lora* for reflowed content.
* **Sans-Serif Engine (UI Controls)**: *SF Pro*, *Inter*, or *Roboto* for chrome and navigation.
* **Hierarchy Standards**:
  * **Book Title (Header)**: Bold Serif, 22pt–26pt.
  * **Section/Chapter Heading**: Semi-bold Serif, 16pt–18pt.
  * **Body Text**: Regular Serif, 10pt–12pt (User slider scaled).
  * **UI Labels & Captions**: Medium Sans-Serif, 9pt–11pt.
* **Readability Bounds**:
  * Line height default: **1.5x - 1.6x** font size.
  * Measure: 60 - 75 characters per line (auto-margin calculation).
  * Justification toggleable (Fully Justified with hyphenation or Flush Left).

---

## 3. Screen Blueprint & Adaptive Layout Specs

### 3.1 Main Library Dashboard (`My Library`)
* **Header Bar**:
  * Search input with live query filtering.
  * View mode toggle (Grid / List), Filter icon, Profile avatar.
* **Hero Widget ("Continue Reading")**:
  * Prominent card displaying active book cover, metadata, percentage completion bar, and a `RESUME` button.
* **Book Grid View**:
  * Responsive 4 to 5 column layout with adaptive cover scaling.
  * Cards display cover render, subtle drop-shadow, title, author, and progress line.
* **Bottom Activity Summary**:
  * **Reading Goals**: Daily streak counter (e.g., "38 min/day") and progress ring.
  * **Bookmarks & Notes**: Quick counts for active bookmarks, highlights, and notes.
* **Global Navigation**:
  * Persistent tab bar on mobile (`Library`, `Browse`, `Audiobooks`, `Search`, `Settings`) or side menu on desktop.

### 3.2 Active Reading View Canvas
* **Clean State**: Edge-to-edge reading canvas with auto-calculated margins and zero UI chrome.
* **Interaction Trigger**: Single tap anywhere in the viewport center toggles overlay elements.

### 3.3 Controls & Popovers

#### Reading Settings (`Aa` Sheet)
* **Theme Swatches**: Quick circular selectors for Day (Hygge), Focus (Steel), and Night (Gilded).
* **Font Size Controls**: Decrement (`A-`) and Increment (`A+`) buttons with center scale percentage.
* **Font Selection**: Horizontal scroll or dropdown for font family switching.
* **Spacing & Margin Sliders**: Granular controls for line height, side margins, and paragraph spacing.

#### Text Selection Contextual Menu
* Floating bubble appearing immediately above user-selected text boundaries.
* Action buttons: `Highlight` (Color swatches: Yellow, Green, Pink, Blue), `Add Note`, `Define` (Dictionary popover), `Search`, `Copy`.

#### Desktop / Laptop Layout (`window-width >= 600px`)
* **Top Header**:
  * Top-Left: `[< Back]` button to return to Library.
  * Top-Center: Current Chapter Title.
  * Top-Right: `[≡]` Main Control Menu icon.

#### Mobile Portrait Layout (`window-width < 600px`)
* **Top-Left Stacked Controls**:
  * Row 1: `[< Back]` button anchored at the top-left.
  * Row 2: `[≡ Nav]` Main Control Menu icon stacked directly beneath the Back button.
  * Default state for `[≡ Nav]`: Semi-transparent (`30%` opacity) to remain unobtrusive during reading.
* **Interaction**: Tapping `[≡ Nav]` opens a bottom sheet or left drawer menu containing all reading tools.

---

## 4. Pinned Floating Action Hub (FAB) Architecture

### 4.1 Concept & Grouping Logic
To keep the canvas clean while offering quick access to tools, users can **pin** specific actions (e.g., Read Aloud, Quick Note, Search) from the Main Menu to a persistent Floating Action Hub.

* **Single/Multiple Pins**:
  * **1 Pin**: Renders as a single floating icon on the lower-right margin.
  * **2+ Pins**: Grouped into a single floating parent icon. Tapping the parent expands the pinned tools radially/linearly.
* **Auto-Dimming & Transparency Physics**:
  * **Inactivity**: After 4 to 5 seconds of touch inactivity, the floating hub dims to **30% opacity**.
  * **Re-activation**: Hovering or tapping near the hub restores it to **100% opacity**.

### 4.2 Active Background Tasks (e.g., Read Aloud / TTS)
* **Status Indication**: When a service like *Read Aloud* is running, the parent floating icon stays subtly illuminated with an active accent stroke (`#D4C1A5`).
* **Sub-Menu Controls**: Tapping an active task icon expands sub-controls (Pause/Play, Speed `1.0x - 2.0x`, Speaker selection).
* **Auto-Collapse**: Sub-controls automatically fold back into the main floating bubble and dim to 30% opacity after 5 seconds while keeping the audio service active.

### 4.3 Main Menu Synchronization Rule
* **Instant Hide**: The moment the user opens the Main Menu (`[≡ Nav]`), the Floating Action Hub transitions to **0% opacity (completely invisible)**.
* **Restore**: Closing the Main Menu restores the Floating Action Hub to its 30% idle opacity.

---

## 5. Expected Layout


### 5.1. Main Library Dashboard (My Library)

Theme Active: Scandinavian Hygge (#F8F8F8 Background)

+-----------------------------------------------------------------------+
|  [Q Search books, authors...]          (Grid/List)  [Filter]   ( S )  |
|-----------------------------------------------------------------------|
|  CONTINUE READING                                                     |
|  +-----------------------------------------------------------------+  |
|  |  +-------+  SAPIENS: A Brief History of Humankind               |  |
|  |  | Cover |  Yuval Noah Harari                                   |  |
|  |  | Image |  Progress: 43% [========---------]                   |  |
|  |  +-------+  [ RESUME READING ]                                  |  |
|  +-----------------------------------------------------------------+  |
|                                                                       |
|  BOOK COLLECTION                                          5 Columns   |
|  +--------+   +--------+   +--------+   +--------+   +--------+       |
|  | ATOMIC |   |EDUCATED|   |MIDNIGHT|   |  DUNE  |   | CIRCE  |       |
|  | HABITS |   |        |   |LIBRARY |   |        |   |        |       |
|  |        |   |        |   |        |   |        |   |        |       |
|  +--------+   +--------+   +--------+   +--------+   +--------+       |
|  Atomic...    Educated     The Mid...   Dune         Circe            |
|  78% [===-]   54% [==-]    91% [====]   12% [-]      07% [-]          |
|                                                                       |
|  +-----------------------------------+-----------------------------+  |
|  | READING GOALS                     | BOOKMARKS & NOTES           |  |
|  | [38 min/day] Streak: 12 Days      | 3 Active Bookmarks          |  |
|  | Daily Goal: [==========---] 80%   | 14 Highlights, 5 Notes      |  |
|  +-----------------------------------+-----------------------------+  |
|-----------------------------------------------------------------------|
|  [ Library ]     [ Browse ]     [ Audio ]     [ Search ]   [Settings] |
+-----------------------------------------------------------------------+

### 5.2.1. Active Reading View Overlay (Bigger View Ports)

Theme Active: Gilded Indulgence (#1A1A1A Canvas) — Triggered via Tap

+-----------------------------------------------------------------------+
|  [< Library]            Chapter 4: The Great Horizon        (≡)  (Aa) |
+-----------------------------------------------------------------------+
|                                                                       |
|      The autumn wind carried the faint scent of pine across the       |
|   valley. As night began to settle over the hills, a solitary         |
|   figure stood at the edge of the overlook, watching the distant      |
|   lights of the city flicker into life one by one.                    |
|                                                                       |
|      "We don't have much time," she whispered, adjusting her          |
|   coat against the sharp evening chill.                               |
|                                                                       |
|   +---------------------------------------------------------------+   |
|   |  [ Yellow ] [ Green ] [ Blue ] |  [ Add Note ]  [ Copy ]  (X) |   |
|   +---------------------------------------------------------------+   |
|      "If we don't reach the crossing before twilight, the river       |
|   will be completely impassable until morning."                       |
|                                                                       |
|                                                               \~~~~~  |
|                                                                \ 3D   |
|                                                                 \ Curl|
|                                                                  \~~~~|
+-----------------------------------------------------------------------+
|  [<]  o=======================================------------------  [>] |
|       Page 212 of 450                         12 mins left in ch.     |
+-----------------------------------------------------------------------

### 5.2.2. Active Reading View & Main Control Menu (`[≡ Nav]`) Hierarchy (Smaller View Ports)

The Main Menu acts as the master command center, housing all feature access and pinning controls:

```
+-------------------------------------------------------------------+
| [< Library]                                                       |
|                                                                   |
| MAIN READING MENU                                                 |
+-------------------------------------------------------------------+
| 📖 NAVIGATION                                                     |
|    ├── Table of Contents (Chapters)                               |
|    ├── Bookmarks Gallery                                          |
|    └── Highlights & Notes Gallery                                 |
|-------------------------------------------------------------------|
| Aa DISPLAY & TYPOGRAPHY                                           |
|    ├── Theme Swatches (Hygge / Steel / Gilded)                    |
|    ├── Font Family Picker                                         |
|    ├── Font Size (A- / A+) & Line Spacing Sliders                 |
|    └── Page Margin & Text Alignment Controls                      |
|-------------------------------------------------------------------|
| 📌 TOOLS & FEATURE PINNING                                        |
|    ├── 🎧 Read Aloud (TTS Engine)                [ Pin to Hub 📌 ]|
|    ├── 📝 Quick Note Creator                     [ Pin to Hub 📌 ]|
|    ├── 🔍 Search Inside Book                     [ Pin to Hub 📌 ]|
|    ├── 🔖 Toggle Bookmark                        [ Pin to Hub 📌 ]|
|    └── 💡 Dictionary / Lookup                    [ Pin to Hub 📌 ]|
|                                                                   |
|                                                           \~~~~~  |
|                                                            \ 3D   |
|                                                             \ Curl|
|                                                              \~~~~|
+-------------------------------------------------------------------+
|  [<]  o=======================================--------------  [>] |
|       Page 212 of 450                     12 mins left in ch.     |
+-------------------------------------------------------------------+

```

---

## 6. Technical Architecture: Slint + Rust

```
+-------------------------------------------------------------------+
|                        SLINT UI LAYER                             |
|  - Layout Trees, Touch Area Events, Timers, State Animations      |
|  - Adaptive Layout Switching (Desktop vs Stacked Mobile)          |
+---------------------------------+---------------------------------+
                                  | (Slint Property Bindings & Callbacks)
                                  v
+-------------------------------------------------------------------+
|                     RUST READER CORE ENGINE                       |
|                                                                   |
| +-------------------------+     +-------------------------------+ |
| | EPUB / PDF Parser       |     | Cosmic-Text Layout Engine     | |
| | (epub-rs / mupdf)       |     | - Text Wrapping & Hyphenation | |
| +------------+------------+     | - Vector Glyphs Calculation   | |
|              |                  +---------------+---------------+ |
|              v                                  |                 |
| +-----------------------------------------------+---------------+ |
| | GPU Texture Render & Physics Engine                           | |
| | - 3D Page Curl Shader (wgpu / Skia Integration)               | |
| | - Background Text-To-Speech (TTS) Engine                      | |
| +-------------------------------+-------------------------------+ |
|                                 |                                 |
|                                 v                                 |
| +---------------------------------------------------------------+ |
| | Persistence Layer (SQLite via rusqlite)                       | |
| | - Library Index, Bookmarks, Annotations, User Pinned Actions  | |
| +---------------------------------------------------------------+ |
+-------------------------------------------------------------------+
```

### 6.1 Slint State Logic Snippet
```slint
export component ReaderCanvas inherits Window {
    in-out property <bool> is-mobile: root.width < 600px;
    in-out property <bool> main-menu-open: false;
    in-out property <bool> hub-dimmed: true;
    in-out property <float> hub-opacity: main-menu-open ? 0.0 : (hub-dimmed ? 0.3 : 1.0);

    // Auto-dim timer for Floating Action Hub
    Timer {
        interval: 4500ms;
        running: !root.main-menu-open;
        triggered() => { root.hub-dimmed = true; }
    }

    // Top Header Layout
    VerticalLayout {
        if is-mobile : VerticalLayout {
            // Stacked Mobile Header Layout
            TouchArea { /* Back Button (<) */ }
            TouchArea { 
                /* Nav Button (≡) stacked below Back */
                opacity: root.main-menu-open ? 1.0 : 0.3;
                clicked => { root.main-menu-open = !root.main-menu-open; }
            }
        }
        if !is-mobile : HorizontalLayout {
            // Spaced Desktop Header Layout
            TouchArea { /* Back Button (<) */ }
            Text { text: "Chapter Title"; }
            TouchArea { /* Nav Button (≡) */ }
        }
    }
}
```

---

## 7. Implementation Roadmap

### Phase 1: Engine & Adaptive Layout Setup
* [ ] Initialize Slint project with modular `.slint` components.
* [ ] Set up theme color tokens and typography hierarchy.
* [ ] Build adaptive viewport container (Desktop spaced top bar vs. Mobile stacked header).
* [ ] Integrate Rust EPUB (`epub-rs`) and PDF parsing pipeline.

### Phase 2: Core Interactivity & Floating Hub
* [ ] Build `My Library` view (Cover grid, Continue Reading card, Goals summary).
* [ ] Construct reading canvas with center-tap overlay toggling.
* [ ] Implement Main Menu (`[≡ Nav]`) drawer/sheet with tool pinning logic.
* [ ] Build Floating Action Hub with auto-grouping, 4.5s auto-dimming timer, and 0% opacity override when Main Menu opens.

### Phase 3: GPU Micro-Interactions & Persistence
* [ ] Implement 3D page curl dynamic displacement shader via `wgpu`/Skia bridged to Slint.
* [ ] Build Text-To-Speech (TTS) background engine thread in Rust with Slint audio sub-menu controls.
* [ ] Implement SQLite (`rusqlite`) storage for book indexes, reading progress, bookmarks, notes, and user-selected pinned hub actions.

## 8. Slint Component Map & Hierarchy

This map outlines the modular component tree for your `.slint` codebase, detailing how properties flow from the top-level application container down to micro-components.


AppWindow (Main Entry)
│
├── ThemeState (Global Singleton)
│     ├── active-palette: ColorTokens (Hygge / Steel / Gilded)
│     └── current-typography: FontConfig
│
├── AppNavigation (View Switcher / Router)
│     │
│     ├── LibraryView (Dashboard Screen)
│     │     ├── SearchHeader
│     │     │     ├── SearchInput
│     │     │     ├── ViewToggleBtn (Grid/List)
│     │     │     └── ProfileAvatar
│     │     │
│     │     ├── ContinueReadingHeroCard
│     │     │     ├── BookCoverImage
│     │     │     ├── MetadataText
│     │     │     ├── ProgressBar
│     │     │     └── ResumeButton
│     │     │
│     │     ├── BookGridContainer
│     │     │     └── BookCardItem [Repeater]
│     │     │           ├── CoverShadowContainer
│     │     │           ├── DynamicProgressIndicator
│     │     │           └── ContextMenuTrigger (Three-Dots)
│     │     │
│     │     ├── ActivityGoalsWidget
│     │     │     ├── StreakCounterCard
│     │     │     └── BookmarksSummaryCard
│     │     │
│     │     └── GlobalBottomTabBar (Mobile Navigation)
│     │           └── TabBarItem [Repeater]
│     │
│     └── ReaderViewCanvas (Active Reading Workspace)
│           │
│           ├── RenderViewport (Rust-Backed Content Canvas)
│           │     ├── GPUPageTexture (wgpu/Skia Curl Displacement)
│           │     └── TextHighlightOverlay (Bounding Rect Vectors)
│           │
│           ├── AdaptiveHeaderOverlay (Controlled by is-mobile)
│           │     ├── DesktopHeader (window-width >= 600px)
│           │     │     ├── BackButton (<)
│           │     │     ├── ChapterTitleText
│           │     │     └── MainMenuBtn (≡)
│           │     │
│           │     └── MobileStackedHeader (window-width < 600px)
│           │           ├── BackButtonRow (<)
│           │           └── NavButtonRow ([≡ Nav] - 30% Idle Opacity)
│           │
│           ├── ReaderFooterOverlay
│           │     ├── ChapterScrubberSlider
│           │     ├── PageMetricText ("Page X of Y")
│           │     └── TimeRemainingText
│           │
│           ├── FloatingActionHub (FAB Tool Group)
│           │     ├── HubParentBubble (Fades to 0% when Main Menu is open)
│           │     ├── PinnedToolItem [Repeater] (Single or Grouped Pop-out)
│           │     └── ActiveTaskDrawer (Audio/TTS Sub-controls: Speed, Speaker, Play)
│           │
│           └── MainReadingMenuDrawer (Full Sheet / Left Drawer)
│                 ├── ChapterNavigationList
│                 ├── BookmarksAndNotesGallery
│                 ├── TypographySettingsSheet (Aa Controls)
│                 │     ├── ThemeSwatchSelector
│                 │     ├── FontSizeStepper
│                 │     └── MarginSlider
│                 └── FeaturePinningList ([ Pin to Hub 📌 ])
│                       └── PinnableToolRow [Repeater]
│
└── ContextualSelectionMenu (Floating Text Selection Bubble)
├── HighlightColorPalette (Yellow, Green, Blue, Pink)
├── QuickNoteBtn
├── LookupBtn
└── CopyBtn

### Component Data Flow & State Responsibility Table

| Component | State Ownership | Primary Responsibilities |
| :--- | :--- | :--- |
| `AppWindow` | Main Window | Handles global layout bounds, window dimensions, and delegates active view (`Library` vs `Reader`). |
| `ThemeState` | Global Singleton | Supplies background, surface, text colors, and active font metrics to all child components. |
| `AdaptiveHeaderOverlay` | `ReaderViewCanvas` | Dynamically switches between horizontal desktop spacing and vertical mobile stacked rows based on `root.width`. |
| `FloatingActionHub` | Native Slint Timer + Rust State | Manages 4.5s idle dimming (`30%`), handles grouped expansion physics, and sets opacity to `0%` when `main-menu-open == true`. |
| `MainReadingMenuDrawer` | Slint / Rust Binding | Houses all core reading controls and emits pinning callbacks to update the user's active hub array in SQLite. |
| `RenderViewport` | Rust GPU Core | Receives drag gesture coordinates from Slint `TouchArea` to render hardware-accelerated 3D page curl effects and text bounding boxes. |

## Further Info

Here is the refined, feature-complete version of your specification section. It integrates all our previously agreed features—including the Pinned Floating Action Hub (FAB), 3D Page Curl Shader, Adaptive Dual-Layout (Stacked Mobile Header), and Color Tokens—directly into your existing structure while elevating the UX details.
### 1. Navigation Model & View Architecture
┌─ Root (Single Window, Slint Router Stack) ──────────────────────────────────┐
│ Library Dashboard ──▶ Reader View (EPUB | PDF Engine)                       │
│   │                     ├── Chrome Overlays (Header Bar, Scrubber Footer)   │
│   │                     ├── Pinned Floating Action Hub (Active/Idle FAB)    │
│   │                     └── Main Control Menu Drawer (Sheet/Popover)        │
│   ├──▶ Search Screen (Full-bleed Overlay with live hit highlighting)         │
│   ├──▶ Settings Panel (Typography, System & Storage)                        │
│   └──▶ Onboarding / Import State (Empty Library CTA)                        │
└─────────────────────────────────────────────────────────────────────────────┘

### 2. Back Behavior:
   * In Library: Secondary screens (Search, Settings) return to Main Dashboard.
   * In Reader View: If the Main Reading Menu or an expanded floating drawer is open, Back closes the open menu/chrome first. A second tap on < Back exits the reader, automatically persisting exact byte/character reading positions to SQLite.
 * Chrome Auto-Hide Timing:
   * Reading Overlays (Header & Scrubber Footer): Auto-hide after 2.5s of tap/drag inactivity.
   * Floating Action Hub (FAB): Dims to 30% opacity after 4.5s of inactivity (remains semi-transparent while reading). Sets to 0% opacity (invisible) instantly whenever the Main Menu is opened.
### 3. Library Screen Dashboard
 * Adaptive Cover Grid:
   * Responsive columns (2–3 columns portrait mobile, 4–5 columns landscape/desktop).
   * Hero Section: Prominent "Continue Reading" card featuring active cover art, author metadata, dual-progress ring, and a dynamic RESUME button.
 * Gestures & Context Menus:
   * Long-press or ··· (Three-Dots) button on cover triggers a Sheet (Mobile) or Popover (Desktop) with actions: Open, Edit Metadata, Move to Shelf, Export Backup, Delete (Confirm).
 * Header Bar: Search input with live filter, View Mode Toggle (Grid/List), Filter/Sort icon, and Profile Avatar. Floating + Import FAB in lower margin.
 * Empty State: High-contrast vector illustration, "Import Your First Book" primary CTA, and supported file tags (.epub, .pdf).
 * Import UX: Non-blocking modal showing real-time file extraction stages (Parsing metadata, pre-rendering cover thumbnails) with cancel/retry states.
### 4. Reader Engine — EPUB Viewport
 * Canvas Geometry: Edge-to-edge reflow canvas with user-configurable margins (12pt–40pt) and 1.5x–1.6x baseline line height.
 * Adaptive Header Navigation:
   * Desktop (>= 600px): Horizontal Header Bar — < Back on top-left, Chapter Title centered, [≡] Main Menu on top-right.
   * Mobile (< 600px): Stacked Left Header — Row 1 has < Back. Row 2 anchors [≡ Nav] directly below it, rendering at 30% idle opacity for thumb-friendly reachability.
 * Tap Zones & Page Turning:
   * Left 20% = Previous Page; Right 20% = Next Page; Center 60% = Toggle Overlay Chrome.
   * Page Curl Physics: Hardware-accelerated 3D conical page curl shader (wgpu/Skia pipeline) executing dynamic displacement during drag gestures.
 * Pinned Floating Action Hub (FAB):
   * Single or multi-grouped floating bubble for user shortcuts (Read Aloud, Quick Note, Search, Bookmark).
   * Background Tasks (TTS): Running tasks keep the hub illuminated with an active accent ring (#D4C1A5). Tapping it expands media sub-controls (Play/Pause, 1.0x–2.0x Speed, Speaker Selector) that auto-collapse after 5s.
 * Aa Typography Sheet:
   * Palette Selection: Circular swatches for Day (Scandinavian Hygge), Focus (Industrial Steel), and Night (Gilded Indulgence) with auto-night activation option.
   * Font Stepper (A- / A+), Line Height slider, Margin controls, and Text Justification toggle.
### 5. Reader Engine — PDF Viewport
 * Rendering & Navigation: Continuous vertical scroll or discrete side-by-side pages with pinch-to-zoom support.
 * Hardware Filter Integration: Real-time fragment shader applying Hygge/Steel/Gilded color matrix filters to PDF canvases without destroying vector crispness.
 * UI Lockout: Automatic chrome hide upon zooming in past 100% scale to maximize viewport focus; dynamic "Page X of Y" floating pill appears transiently during rapid scrolling.
### 6. Selection, Highlights & Annotations (EPUB)
 * Interactive Selection Overlay: Long-press text triggers selection handles and positions a floating contextual bubble directly above text bounds:
   * Highlight: Color Palette Swatches (Yellow, Green, Blue, Pink).
   * Add Note: Triggers inline modal sheet.
   * Lookup / Define: Dictionary popover window.
   * Copy: Text string to system clipboard.
 * Highlight Management:
   * Tap existing highlight: Shows swatch color picker, Note view/edit icon, and Delete button with a 4s Undo snackbar.
 * Annotations Gallery: Centralized screen displaying grouped book quotes, color chips, chapter tags, and instant jump-to-location links.
### 7. Search UX
 * Global Library Search: Full-screen overlay; instant query evaluation across title, author, and book content text indexes in SQLite. Results highlight query terms using standard <mark> accent styling.
 * In-Reader Search: Overlay panel featuring sequential hit lists, step navigation arrows (< / >), and vector highlight overlays drawn behind matching search results on the reader canvas.
### 8. Settings & Customization
Sleek sectioned menu structure applying live adjustments to active state:
 * Reading & Rendering: Default Theme Palette, Primary Font, Tap Zone Layout, Auto-Night schedule settings.
 * Narration (TTS Engine): Default Speech Rate, Voice/Pitch Selection, System Wake-Lock toggle, Auto-play next chapter.
 * Storage & Sync: SQLite Database Backup/Restore, Cover Image Cache Clearing.
 * About: Build Version, Open-Source Licenses, Privacy Statement.
### 9. Accessibility Standard
 * Screen Readers: Full Slint accessibility tree integration providing semantic labels for controls and exposing body text nodes.
 * Font Scaling: UI bounds dynamically respect OS accessibility font scale multipliers (1.0x–2.0x) while keeping book text independently customizable.
 * Color Contrast: All theme tokens guarantee a minimum 4.5:1 contrast ratio for body text.
 * Reduced Motion: Respects system reduced-motion flags by converting 3D page curl animations into instant fade transitions.
### 10. Orientation & Responsive States
 * Library Dashboard: Dynamic re-grid recalculations on window resize (Portrait: 2–3 columns; Landscape: 4–5 columns).
 * Reader Engine: Dynamic text reflow and page recalculations on screen rotation, using exact character/byte offsets to prevent reading position loss.
 * TTS Continuity: Background audio narration threads continue playing seamlessly across rotation events.
### 11. Complete Slint Component Hierarchy (ui/)
AppRoot.slint                      # Root Window, Routing Stack & Theme Provider
├── Theme.slint                    # Global Palette Tokens, Fonts & Style Declarations
├── LibraryScreen.slint            # Main Dashboard View
│   ├── SearchHeader.slint         # Query input, Grid/List Toggles, Profile Avatar
│   ├── ContinueReadingCard.slint  # Hero Progress Widget
│   ├── BookGridContainer.slint    # Responsive Cover Grid
│   │   └── BookCardItem.slint     # Book Cover, Shadow & Progress Bar
│   ├── ActivityGoalsWidget.slint  # Reading Streak & Notes Summary
│   └── GlobalBottomTabBar.slint   # Navigation Bar (Mobile)
│
├── ReaderScreen.slint             # Primary Reading Workspace
│   ├── RenderViewport.slint       # GPU Canvas (wgpu/Skia Curl Shader)
│   ├── AdaptiveHeader.slint       # Desktop Horizontal vs Mobile Stacked Left Header
│   ├── ReaderFooter.slint         # Page Scrubber Slider & Metric Labels
│   ├── FloatingActionHub.slint    # Pinned Tool Group (4.5s Auto-Dimming FAB)
│   │   └── TtsControlDrawer.slint # Media Sub-controls (Speed, Speaker, Play)
│   └── MainReadingMenu.slint      # Full Master Sheet / Drawer
│       ├── TocNavigation.slint    # Chapter Table of Contents
│       ├── AaPanel.slint          # Typography, Size & Palette Settings
│       └── FeaturePinning.slint   # Tool Pinning Matrix (`[ Pin to Hub 📌 ]`)
│
├── SelectionPopover.slint         # Floating Selection Bubble (Highlight, Note, Define)
├── SearchScreen.slint             # Full-Bleed Search Overlay & Result Rows
├── HighlightsScreen.slint         # Annotations & Bookmark Gallery View
├── SettingsScreen.slint           # Configuration Sections
└── Dialogs.slint                  # Confirm, Error, and File Import Progress Modals

### 12 * Naming Conventions: kebab-case.slint files, PascalCase components. All UI styling consumes Theme.slint tokens—no hardcoded hex values in view components.

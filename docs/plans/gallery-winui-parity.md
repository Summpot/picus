# Plan: example_gallery ↔ WinUI Gallery control parity

**Status**: active  
**Baseline**: `example_gallery` **57** pages (post `feat(gallery): expand showcase to 57 pages…`)  
**Reference**: sibling repo `../WinUI-Gallery` (`ControlInfoData.json`, ~**120** sample pages)  
**Owner path**: `examples/gallery/` + `crates/picus_core/src/components/` + `crates/picus` facade

This plan lists **every WinUI Gallery control page**, classifies what Picus can still fill in, and sequences the work so gallery pages track real components (no hollow placeholders as “done”).

---

## 1. Goals

1. Cover **all Picus-appropriate** WinUI Gallery controls with a **one-control-per-page** showcase (WinUI model).
2. When a WinUI control has no 1:1 type, either:
   - **compose** from existing Picus APIs and document the mapping, or
   - **implement the component** (ECS + project + theme + facade + tests), then add the gallery page.
3. Keep hard skips explicit (XAML authoring model, media stack, OS shell) so agents do not re-litigate them.
4. Applications continue to depend on the **`picus` facade only** (`AGENTS.md`).

### Non-goals

- Pixel-perfect WinUI clone or XAML code samples.
- Shipping WebView / video / camera / maps / system toast protocol.
- Restoring long tutorial encyclopedias into `AGENTS.md` (narrative stays in `docs/`).

---

## 2. Status legend

| Tag | Meaning |
|-----|---------|
| **DONE** | Gallery page exists and demos the primary API |
| **POLISH** | Page exists; deepen variants / interaction / edge cases only |
| **GALLERY** | Component already exists (or can be composed); only gallery (+ thin facade export) work |
| **COMPOSE** | No new public type required; document composition of existing pieces + gallery page |
| **COMPONENT** | Need new or extended `Ui*` API in `picus_core`, then gallery |
| **SKIP** | Intentionally out of scope for this plan |

**Effort** (rough): S ≤ 1 day · M 2–4 days · L multi-day / multi-PR · XL product-sized.

---

## 3. Master inventory (WinUI → Picus)

Organized by WinUI Gallery categories. **“Picus target”** is the intended API or composition.

### 3.1 Fundamentals (WinUI) — mostly SKIP

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| XamlResources | SKIP | — | — | XAML resource dictionaries |
| XamlStyles | SKIP | Theme RON / `StyleClass` guides already in docs | — | Not a control page |
| Binding | SKIP | ECS / projection, not `{x:Bind}` | — | |
| Templates | SKIP | `UiComponentTemplate` is framework, not a sample page | — | |
| CustomUserControls | SKIP | Derive `UiComponent` docs | — | |
| CustomXamlConditionals | SKIP | — | — | |
| ScratchPad | SKIP | — | — | |

### 3.2 Design

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| Color | GALLERY | Token / swatch page from theme RON + `ColorPicker` | M | Design guidance page, not a control |
| Geometry | GALLERY | Corner radius / spacing tokens + canvas shapes | S | |
| Iconography | POLISH | `Icons` page + fuller Fluent set browser | M | Expand glyph grid / search |
| Spacing | GALLERY | Spacing scale from theme + layout demos | S | |
| Typography | POLISH | Existing `Typography` / `TextBlock` pages | S | Export `TypographyPreset` on facade if needed |

### 3.3 Accessibility — SKIP as doc-clone pages; track as test contracts later

| WinUI UniqueId | Status | Notes |
|----------------|--------|-------|
| AccessibilityColorContrast | SKIP | Prefer automated contrast checks over a prose page |
| AccessibilityKeyboard | SKIP | Keyboard nav tests under `docs/guide/testing.md` |
| AccessibilityScreenReader | SKIP | Accessible names / roles when a11y surface stabilizes |

### 3.4 Basic input

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| Button | POLISH | `UiButton` appearances / sizes / icons | S | Appearances already partially demoed |
| DropDownButton | COMPONENT | `UiDropDownButton` (button + chevron + `UiDropdownMenu` / menu items) | M | Distinct from `UiComboBox` selection |
| HyperlinkButton | POLISH | `UiLink` | S | Done; add more inline / disabled cases |
| RepeatButton | COMPONENT | `UiRepeatButton` (press-and-hold repeat `Clicked`) | M | |
| ToggleButton | COMPONENT | `UiToggleButton` (checked + appearance) | M | Or checked state on button API |
| SplitButton | COMPONENT | Primary action + secondary menu | L | Depends on DropDownButton plumbing |
| ToggleSplitButton | COMPONENT | Toggle + menu | L | After SplitButton |
| CheckBox | POLISH | `UiCheckbox` tri-state | S | |
| ColorPicker | POLISH | `UiColorPicker` | S | |
| ComboBox | POLISH | `UiComboBox` | S | |
| RadioButton | POLISH | `UiRadioGroup` | S | |
| RatingControl | POLISH | `UiRating` | S | |
| Slider | POLISH | `UiSlider` | S | Optional vertical / tick marks later |
| ToggleSwitch | POLISH | `UiSwitch` | S | |

### 3.5 Text

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| AutoSuggestBox | COMPONENT | `UiSearch` + suggestion overlay list | L | Today: SearchBox without suggestions |
| NumberBox | POLISH | `UiNumericUpDown` | S | Already under Basic Input as NumberBox |
| PasswordBox | POLISH | `UiPasswordInput` | S | |
| RichEditBox | COMPONENT | Editable rich text (subset) | XL | Optional phase; markdown editor not enough |
| RichTextBlock | COMPOSE / COMPONENT | `UiMarkdown` + spans, or lightweight rich static text | L | Start with markdown/static runs |
| TextBlock | POLISH | `UiText` / `UiLabel` + type ramp | S | Export `TypographyPreset` if missing |
| TextBox | POLISH | `UiTextInput` | S | |
| *(Picus)* MultiLineTextBox | DONE | `UiMultilineTextInput` | — | WinUI folds into TextBox |

### 3.6 Date & time

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| DatePicker | POLISH | `UiDatePicker` | S | Overlay calendar only today |
| TimePicker | POLISH | `UiTimePicker` | S | Facade export already done |
| CalendarDatePicker | COMPONENT | Always-visible or hybrid calendar field | L | May share month grid with DatePicker |
| CalendarView | COMPONENT | Multi-date / range calendar surface | L | Extract shared calendar grid module |

### 3.7 Collections

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| ListView | POLISH | `UiListView` | S | Item templates / headers later |
| TreeView | POLISH | `UiTreeNode` | S | |
| GridView | COMPONENT | Wrap/tile collection (`UiGridView` or ListView + responsive grid) | L | |
| FlipView | COMPONENT | `UiFlipView` (paged carousel) | M | |
| ItemsRepeater | COMPONENT / COMPOSE | Virtualized repeater over scroll | XL | After ScrollView virtualization hooks mature |
| ItemsView | SKIP / DEFER | WinUI 3 ItemsView; wait for clearer product need | — | |
| PullToRefresh | COMPONENT | Overscroll refresh affordance | L | Platform gesture dependent |
| *(Picus)* Table / DataTable | DONE | `UiTable` / `UiDataTable` | — | Beyond WinUI set |

### 3.8 Status & info

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| ProgressBar | POLISH | `UiProgressBar` | S | |
| ProgressRing | POLISH | `UiSpinner` | S | Naming: Spinner page = ProgressRing |
| ToolTip | POLISH | `HasTooltip` / `UiTooltip` | S | |
| InfoBadge | COMPONENT | Dot / number / icon badge API on `UiBadge` or `UiInfoBadge` | M | Nav `with_info_badge` should share styles |
| InfoBar | POLISH | `UiMessageBar` | S | Action buttons / non-dismissible variants |
| *(Picus)* Avatar | DONE | `UiAvatar` | — | PersonPicture-class under Media in WinUI |

### 3.9 Dialogs & flyouts

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| ContentDialog | POLISH | `UiDialog` | M | Primary/secondary buttons, content slot |
| Flyout | POLISH | `UiPopover` / anchored flyout | M | Rename map: Popover ≈ Flyout |
| Popup | COMPOSE | Manual overlay (`spawn_manual_overlay_at`) page | S | Already partial on Popover page |
| TeachingTip | COMPONENT | Anchored tip + title/body/close + light-dismiss | M | Compose popover + structured content first |

### 3.10 Menus & toolbars

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| MenuBar | POLISH | `UiMenuBar` | S | |
| MenuFlyout | POLISH | `UiContextMenu` / menu panels | M | Dedicated page: left-click flyout vs context |
| CommandBar | COMPONENT | `UiCommandBar` (primary/secondary/overflow) on `UiToolbar` | L | |
| AppBarButton | COMPOSE | Icon+label button in toolbar | S | After CommandBar or via Toolbar samples |
| AppBarToggleButton | COMPOSE | Toggle in toolbar | S | Needs ToggleButton |
| AppBarSeparator | COMPOSE | `UiDivider::vertical()` | S | |
| CommandBarFlyout | COMPONENT | Selection-scoped command flyout | L | After CommandBar + Flyout |
| SwipeControl | SKIP / DEFER | Touch-first; optional later | L | |
| StandardUICommand | SKIP | XAML command pattern | — | |
| XamlUICommand | SKIP | XAML command pattern | — | |
| *(Picus)* Toolbar | DONE | `UiToolbar` | — | Stepping stone to CommandBar |

### 3.11 Navigation

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| BreadcrumbBar | POLISH | `UiBreadcrumb` | S | Clickable segments / overflow |
| NavigationView | POLISH | `UiNavigationView` | M | Modes, back, compact, badges, settings |
| TabView | COMPONENT | Closable / reorderable tabs on `UiTabBar` | L | |
| Pivot | COMPOSE | TabBar with pivot styling | M | May share TabBar |
| SelectorBar | COMPONENT | Compact segmented control (`UiSelectorBar`) | M | |
| *(Picus)* TabBar | DONE | `UiTabBar` | — | Base for TabView/Pivot |

### 3.12 Scrolling

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| ScrollView | POLISH | `UiScrollView` | M | Nested scroll, programmatic offset |
| ScrollViewer | COMPOSE | Alias / note: same as ScrollView | S | Single page is enough |
| PipsPager | COMPONENT | `UiPipsPager` | M | Pair with FlipView |
| AnnotatedScrollBar | COMPONENT | Labeled scrollbar rail | L | |
| SemanticZoom | COMPONENT | Zoom-out index + zoom-in list | XL | |

### 3.13 Layout

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| StackPanel | POLISH | `UiFlexRow` / `UiFlexColumn` | S | |
| Grid | POLISH | `UiGrid` | S | |
| Canvas | POLISH | `UiCanvas` | S | |
| Expander | POLISH | `UiExpander` | S | Accordion pattern |
| SplitView | POLISH | `UiSplitPane` | S | Pane display modes |
| Border | COMPONENT | `UiBorder` (stroke, radius, padding, bg) | M | Or stylesheet-only demo first |
| RelativePanel | COMPONENT | Constraint layout | XL | Low priority vs flex/grid |
| VariableSizedWrapGrid | COMPOSE | Responsive wrap grid | M | Via `UiResponsiveGrid` / wrap flex |
| Viewbox | COMPONENT | Uniform scale-to-fit container | M | |
| *(Picus)* Responsive / GroupBox / FormRow / Card / Divider | DONE | Existing | — | Keep; document as Picus-native |

### 3.14 Media

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| Image | POLISH | `UiImage` | S | |
| PersonPicture | POLISH | `UiAvatar` (alias page or merge docs) | S | Already Avatar page |
| AnimatedVisualPlayer | SKIP | Lottie / animated visual stack | — | |
| CaptureElementPreview | SKIP | Camera | — | |
| MapControl | SKIP | Maps | — | |
| MediaPlayerElement | SKIP | Video/audio playback | — | |
| Sound | SKIP | System sounds API | — | |
| WebView2 | SKIP | Embedded browser | — | |

### 3.15 Styles / brushes / chrome

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| SystemBackdrops | POLISH | `WindowBackdrop` page | S | Done |
| Acrylic | GALLERY | Acrylic token / material samples | M | Theme + backdrop |
| RadialGradientBrush | POLISH | Canvas radial gradient | S | Brushes page partial |
| Line / Shape | POLISH | Canvas shapes | S | Shapes page |
| IconElement | POLISH | Icons page | S | |
| ThemeShadow | COMPONENT | Elevation / shadow tokens | M | Theme + paint |
| CompactSizing | GALLERY | Density variant (compact stylesheet) | M | Theme variant, not control |
| AnimatedIcon | SKIP / DEFER | Animated glyph | L | Without full AVP |
| SystemBackdropElement | SKIP | WinUI composition host | — | |
| *(Picus)* Theme / Brushes / Markdown / I18n | DONE | Keep | — | |

### 3.16 Motion — SKIP (product motion system separate)

| WinUI UniqueId | Status |
|----------------|--------|
| XamlCompInterop | SKIP |
| ConnectedAnimation | SKIP |
| EasingFunction | SKIP (optional micro-demo later with bevy_tween) |
| ImplicitTransition | SKIP |
| PageTransition | SKIP |
| ThemeTransition | SKIP |
| ParallaxView | SKIP |

*Exception (optional later)*: single **Easing / tween** lab page using existing `bevy_tween` integration — not WinUI parity-critical.

### 3.17 Windowing

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| TitleBar | POLISH | `UiTitleBar` | S | |
| AppWindowTitleBar | POLISH | Extend title bar / drag regions | M | Multi-window guide |
| AppWindow | GALLERY | Window size, state, min size demos | M | Via Bevy window APIs |
| CreateMultipleWindows | GALLERY | Multi-window sample in gallery or dedicated example | L | `docs/guide/multi-window.md` |

### 3.18 System

| WinUI UniqueId | Status | Picus target | Effort | Notes |
|----------------|--------|--------------|--------|-------|
| Clipboard | GALLERY / COMPONENT | Public clipboard helpers on facade + page | M | `picus_core::clipboard` exists |
| StoragePickers | GALLERY | `rfd` file/folder pickers (already on facade) | M | |
| ContentIsland | SKIP | WinUI islands | — | |

### 3.19 Shell — SKIP

| WinUI UniqueId | Status | Notes |
|----------------|--------|-------|
| AppNotification | SKIP | OS toast protocol; in-app `UiToast` already covered |
| BadgeNotificationManager | SKIP | Taskbar badge |
| JumpList | SKIP | |

---

## 4. Fill-in backlog (actionable only)

Everything that is **not SKIP**, grouped by work type. This is the complete “can fill” set.

### 4.A — Gallery / polish only (component exists)

| # | Deliverable | Depends |
|---|-------------|---------|
| A1 | Search shell: filter nav leaves by `UiSearch` | events |
| A2 | NavigationView deep samples (compact / left / top modes, back, badges) | — |
| A3 | MenuFlyout dedicated page (vs ContextMenu) | — |
| A4 | ContentDialog buttons / content slot demos | Dialog API may need small extend |
| A5 | Flyout / Popup variants on Popover page (or rename) | — |
| A6 | ScrollView nested + programmatic scroll | — |
| A7 | Iconography browser (searchable Fluent grid) | — |
| A8 | Design pages: Color tokens, Geometry, Spacing | theme tokens |
| A9 | Acrylic / CompactSizing / ThemeShadow samples | theme |
| A10 | Clipboard page | facade export |
| A11 | StoragePickers page (`rfd`) | — |
| A12 | AppWindow / multi-window gallery samples | multi-window |
| A13 | PersonPicture alias note on Avatar page | — |
| A14 | InfoBar action buttons / sticky variants | MessageBar API |
| A15 | Breadcrumb clickable items + overflow | Breadcrumb API |
| A16 | Button / Slider / Combo / Date / Time polish cards | — |
| A17 | Export missing facade types as needed (`TypographyPreset`, clipboard) | facade |

### 4.B — New or extended components (then gallery page)

| # | WinUI name | Picus work | Gallery page label |
|---|------------|------------|--------------------|
| B1 | ToggleButton | `UiToggleButton` or checked button state | ToggleButton |
| B2 | DropDownButton | Button + menu open API | DropDownButton |
| B3 | RepeatButton | Hold-to-repeat click | RepeatButton |
| B4 | SplitButton | Primary + secondary menu | SplitButton |
| B5 | ToggleSplitButton | B1+B4 | ToggleSplitButton |
| B6 | AutoSuggestBox | Search + suggestion panel | AutoSuggestBox (replace or extend SearchBox) |
| B7 | TeachingTip | Structured anchored tip | TeachingTip |
| B8 | Border | `UiBorder` | Border |
| B9 | Viewbox | Scale-to-fit | Viewbox |
| B10 | SelectorBar | Segmented selector | SelectorBar |
| B11 | PipsPager | Page indicators | PipsPager |
| B12 | FlipView | Carousel | FlipView |
| B13 | GridView | Tile grid collection | GridView |
| B14 | CalendarView | Month multi-select surface | CalendarView |
| B15 | CalendarDatePicker | Field + calendar | CalendarDatePicker |
| B16 | CommandBar | Overflow command strip | CommandBar |
| B17 | CommandBarFlyout | Selection commands | CommandBarFlyout |
| B18 | TabView | Closable tabs | TabView |
| B19 | InfoBadge | Dot/number variants | deepen InfoBadge |
| B20 | AnnotatedScrollBar | Labeled scrollbar | AnnotatedScrollBar |
| B21 | VariableSizedWrapGrid | Wrap layout helper | VariableSizedWrapGrid |
| B22 | RichTextBlock | Static rich runs / markdown bridge | RichTextBlock |
| B23 | RichEditBox | Editable rich (subset) | RichEditBox |
| B24 | PullToRefresh | Gesture refresh | PullToRefresh |
| B25 | ItemsRepeater | Virtualized repeater | ItemsRepeater |
| B26 | RelativePanel | Constraint panel | RelativePanel |
| B27 | SemanticZoom | Two-level zoom list | SemanticZoom |
| B28 | SwipeControl | Item swipe actions | SwipeControl |
| B29 | ThemeShadow | Elevation API / tokens | ThemeShadow |
| B30 | AppBar* / Pivot | Compose after B1/B16/TabBar | AppBarButton, Pivot, … |

### 4.C — Explicit permanent / long-term SKIP (do not schedule as parity)

- All **Fundamentals** XAML pages  
- All **Motion** pages (except optional tween lab)  
- **Media**: WebView2, MediaPlayer, Map, Camera, Sound, AnimatedVisualPlayer  
- **Shell**: JumpList, BadgeNotificationManager, OS AppNotification  
- **ContentIsland**, **SystemBackdropElement**  
- **StandardUICommand** / **XamlUICommand**  
- **Accessibility** gallery prose pages (track tests separately)  
- **ItemsView** until product need is clear  

---

## 5. Phased PR plan

Each phase ends with: `cargo test -p example_gallery`, component unit tests where added, and `GalleryPage::ALL` / category contiguity tests updated.

### Phase 0 — Bookkeeping (this document)

- [x] Baseline 57-page gallery committed  
- [x] Plan file written (`docs/plans/gallery-winui-parity.md`)  
- [ ] Link from `docs/README.md`  
- [ ] Keep `GalleryPage` doc-comment skip list in sync with §4.C  

### Phase 1 — Experience & gallery-only (no new controls)

**Target pages / UX**: A1–A8, A13–A17 polish  

| PR | Scope |
|----|--------|
| **1a** | Nav search filter + NavigationView deep samples |
| **1b** | MenuFlyout / Dialog / Flyout-Popup polish pages |
| **1c** | Design guidance pages (Color, Geometry, Spacing) + Iconography browser |
| **1d** | Facade: clipboard + TypographyPreset; Clipboard + StoragePickers pages |

**Exit**: Gallery feels closer to WinUI shell; no new `Ui*` types required.

### Phase 2 — Basic input completion

| PR | Scope |
|----|--------|
| **2a** | ToggleButton + gallery |
| **2b** | DropDownButton + gallery |
| **2c** | RepeatButton + gallery |
| **2d** | SplitButton + ToggleSplitButton + gallery |

**Exit**: WinUI Basic input category fully representable.

### Phase 3 — Dialogs, badges, teaching

| PR | Scope |
|----|--------|
| **3a** | TeachingTip component + page |
| **3b** | InfoBadge variants + InfoBar actions |
| **3c** | ContentDialog structured actions |

**Exit**: Status + Dialogs categories at parity for non-media apps.

### Phase 4 — Layout containers

| PR | Scope |
|----|--------|
| **4a** | UiBorder + page |
| **4b** | Viewbox + page |
| **4c** | VariableSizedWrapGrid compose/page |
| **4d** | (Optional) RelativePanel — only if customers need |

### Phase 5 — Navigation & command chrome

| PR | Scope |
|----|--------|
| **5a** | SelectorBar + Pivot styling |
| **5b** | CommandBar (+ AppBarButton/Separator compose pages) |
| **5c** | TabView (closable/reorder) |
| **5d** | CommandBarFlyout |

### Phase 6 — Date calendar family

| PR | Scope |
|----|--------|
| **6a** | Shared calendar grid module (refactor DatePicker) |
| **6b** | CalendarView + page |
| **6c** | CalendarDatePicker + page |

### Phase 7 — Collections & scrolling extras

| PR | Scope |
|----|--------|
| **7a** | FlipView + PipsPager |
| **7b** | GridView |
| **7c** | AnnotatedScrollBar |
| **7d** | PullToRefresh (if gesture path ready) |
| **7e** | ItemsRepeater / SemanticZoom (only after virtualization story) |

### Phase 8 — Text richness & system

| PR | Scope |
|----|--------|
| **8a** | AutoSuggestBox (suggestions) |
| **8b** | RichTextBlock subset |
| **8c** | RichEditBox subset (optional / XL) |
| **8d** | Multi-window + AppWindow gallery |

### Phase 9 — Stretch

- ThemeShadow, CompactSizing density theme  
- SwipeControl, AnimatedIcon  
- Optional tween/easing lab (not full Motion)  

---

## 6. Per-item definition of done

For each **COMPONENT** item:

1. Public type(s) on **`picus` facade** (`Default + Clone` authoring where applicable).  
2. `UiComponentTemplate` expand/project + style type aliases.  
3. Fluent theme RON rules (missing rules stay invisible — no framework default brand palette on widgets).  
4. Unit / projection tests as appropriate.  
5. Gallery: `GalleryPage` variant, `ALL`/`CATEGORIES`, `label`/`description`/`icon`, `spawn_*_page`, routing match.  
6. No direct app use of `picus::__macro_support` or `picus_core`.  

For each **GALLERY / POLISH / COMPOSE** item:

1. Real interactive sample (not only `placeholder(...)`).  
2. Note text maps WinUI name → Picus API.  
3. Tests updated if page count / categories change.  

---

## 7. Tracking checklist (fill-in targets)

Use this as the living “can still ship” list. Check when gallery + API meet §6.

### Phase 1
- [ ] A1 Search filters nav  
- [ ] A2 NavigationView modes  
- [ ] A3 MenuFlyout page  
- [ ] A4 ContentDialog polish  
- [ ] A5 Flyout/Popup polish  
- [ ] A6 ScrollView polish  
- [ ] A7 Iconography browser  
- [ ] A8 Color / Geometry / Spacing  
- [ ] A9 Acrylic / CompactSizing / ThemeShadow samples  
- [ ] A10 Clipboard  
- [ ] A11 StoragePickers  
- [ ] A12 Multi-window / AppWindow  
- [ ] A13–A17 remaining polish  

### Phase 2–3
- [ ] ToggleButton  
- [ ] DropDownButton  
- [ ] RepeatButton  
- [ ] SplitButton  
- [ ] ToggleSplitButton  
- [ ] TeachingTip  
- [ ] InfoBadge variants  
- [ ] InfoBar actions  
- [ ] ContentDialog structured actions  

### Phase 4–5
- [ ] Border  
- [ ] Viewbox  
- [ ] VariableSizedWrapGrid  
- [ ] SelectorBar  
- [ ] Pivot (compose)  
- [ ] CommandBar  
- [ ] AppBarButton / AppBarToggleButton / AppBarSeparator pages  
- [ ] TabView  
- [ ] CommandBarFlyout  

### Phase 6–7
- [ ] CalendarView  
- [ ] CalendarDatePicker  
- [ ] FlipView  
- [ ] PipsPager  
- [ ] GridView  
- [ ] AnnotatedScrollBar  
- [ ] PullToRefresh  
- [ ] ItemsRepeater  
- [ ] SemanticZoom  

### Phase 8–9
- [ ] AutoSuggestBox (suggestions)  
- [ ] RichTextBlock  
- [ ] RichEditBox (optional)  
- [ ] ThemeShadow  
- [ ] SwipeControl (optional)  
- [ ] RelativePanel (optional)  

### Never (document only)
- [x] XAML Fundamentals set  
- [x] Media stack (WebView / player / map / camera / sound / AVP)  
- [x] Motion set  
- [x] Shell JumpList / OS badge / OS toast protocol  
- [x] ContentIsland / SystemBackdropElement / XamlUICommand  

---

## 8. Suggested page count trajectory

| Milestone | Approx gallery pages | Notes |
|-----------|----------------------|-------|
| Baseline (now) | **57** | Committed |
| After Phase 1 | ~65–70 | Design + system pages, no big components |
| After Phase 2–3 | ~75–80 | Input + tips complete |
| After Phase 4–5 | ~85–95 | Layout + nav chrome |
| After Phase 6–7 | ~100–110 | Calendar + collections |
| After Phase 8–9 | ~110–115 | Approaches full *applicable* WinUI set |
| Hard ceiling vs WinUI 120 | **~100–115** | ~15–25 permanent skips |

Exact counts depend on whether aliases (ScrollViewer, PersonPicture, ProgressRing naming) get separate pages or notes on existing pages.

---

## 9. Implementation conventions

1. **One control → one `GalleryPage` variant** (existing model in `state.rs`).  
2. Prefer **extend existing components** over parallel types when WinUI is a mode (e.g. TabView on TabBar).  
3. **Compose first** for AppBar* and Pivot if CommandBar / TabBar already ship the behaviors.  
4. Theme: production colors from RON only; gallery may use `gallery.*` classes.  
5. Update this plan’s checkboxes when merging; do not leave completed narrative only in commit messages.  
6. When a phase finishes, optionally add a one-line note under [docs/examples/index.md](../examples/index.md).  

---

## 10. Immediate next action

Start **Phase 1a**:

1. Wire top-bar `UiSearch` to filter `UiNavigationView` leaf items (or hide non-matching categories).  
2. Expand NavigationView gallery page: pane display modes, back button, info badges, settings.  
3. Keep tests green; no new components.

Then proceed **1b → 1d**, then **Phase 2** input components.

---

## 11. References

- WinUI data: `../WinUI-Gallery/WinUIGallery/SampleSupport/Data/ControlInfoData.json`  
- Gallery state: `examples/gallery/src/state.rs`  
- Components: `crates/picus_core/src/components/`  
- Facade: `crates/picus/src/lib.rs`  
- App / theme / events: `docs/guide/app.md`, `styling-themes.md`, `events-messages.md`  
- Multi-window: `docs/guide/multi-window.md`  
- Process: root `AGENTS.md`

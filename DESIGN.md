# picus Workspace Design Document

Date: 2026-03-17

This document describes the **current implementation** of the picus workspace, a Bevy-first UI framework integrating ECS state management with a retained Xilem/Masonry UI runtime.

## 1. Workspace Overview

The picus workspace consists of three main crates:

- **picus_core**: The main UI framework crate providing ECS-driven UI projection, styling, overlay management, and built-in components
- **picus_surface**: External window surface bridge for Vello rendering via wgpu
- **picus_activation**: Cross-platform deep-link/activation handling (single-instance gate, custom URI protocol registration, IPC forwarding)

Additionally, the workspace includes multiple example applications demonstrating various features.

## 2. Core Runtime Architecture

### 2.1 Bevy-First Event Loop Ownership

Bevy owns scheduling and window/input message flow. Masonry is driven as a retained UI runtime resource from Bevy systems. The framework does not run a separate Xilem/Masonry event loop; GUI apps use Bevy's native `App::run()` and `bevy_winit` window lifecycle.

### 2.2 Headless Retained Runtime Resource

`MasonryRuntime` is a Bevy `NonSend` resource that owns:

- Masonry `RenderRoot` (retained widget tree)
- Current synthesized root view
- Xilem `ViewCtx` and `ViewState`
- Pointer state required for manual event injection
- Active Bevy primary-window attachment metrics (logical size + scale factor)
- External surface bridge (`picus_surface::ExternalWindowSurface`)
- Vello renderer state

**Scheduling:**

- `PreUpdate`: Input injection, font asset collection, interaction markers, overlay click handling, scroll view geometry sync, widget actions
- `Update`: Overlay lifecycle, overlay actions, tooltip hovers, auto-dismiss ticking, stylesheet asset events, active variant sync, style dirty marking, style target sync, tween interpolation
- `PostUpdate`: UI synthesis, Masonry rebuild, IME state sync
- `Last`: Explicit Vello paint/present pass

**Initialization invariant:**

`initialize_masonry_runtime_from_primary_window` injects an explicit initial logical resize immediately after first attach so Masonry never starts hit-testing from a `(0, 0)` root size.

### 2.3 Explicit Masonry/Vello Paint Pass (Last)

Because Bevy's renderer plugins are intentionally not required for the retained UI path, `picus_core` performs an explicit Vello paint/present pass in `Last`:

- `RenderRoot::redraw()` produces the current scene
- `picus_surface::ExternalWindowSurface` bridge owns persistent surface/device state bound to the Bevy primary window
- The pass renders to an intermediate texture, blits to the swapchain surface, and presents
- The primary window requests another redraw to keep UI animations and visual updates flowing

This avoids the "window opens but no pixels are drawn" failure mode when only window/input plugins are active.

## 3. Input and Hit Testing

### 3.1 Input Injection Bridge (PreUpdate)

`inject_bevy_input_into_masonry` consumes Bevy messages and translates them to Masonry events injected into `MasonryRuntime.render_root`:

- `CursorMoved` → `PointerEvent::Move`
- `CursorLeft` → `PointerEvent::Leave`
- `MouseButtonInput` → `PointerEvent::Down`/`Up`
- `MouseWheel` → `PointerEvent::Scroll`
- `KeyboardInput` → `TextEvent::Keyboard` (navigation/editing keys) and `TextEvent::Ime::Commit` (committed text)
- `Ime` → `TextEvent::Ime::{Preedit,Commit,Enabled,Disabled}`
- `WindowFocused` → `TextEvent::WindowFocusChange`
- `WindowResized` → `WindowEvent::Resize`
- `WindowScaleFactorChanged` → `WindowEvent::Rescale`

**Pointer bridge invariants:**

- `Window::physical_cursor_position()` from the current `PrimaryWindow` is the source of truth for injected Masonry pointer coordinates
- When physical cursor data is unavailable, pointer interaction injection is skipped (cursor outside window)
- Click-path ordering is enforced by injecting `PointerMove` before each `PointerDown`/`PointerUp` so hot/hovered state is current before activation
- Window resize injection uses logical `Window::width()`/`height()` ensuring DPI-correct dimensions

### 3.2 IME Bridge

IME signals from Masonry are captured via `RenderRoot` callbacks and translated back to Bevy window IME state (`ime_enabled`, `ime_position`) in `sync_masonry_ime_state_to_bevy_window`.

## 4. UI Components and Registration

### 4.1 Component-Centric UI Encapsulation (`UiComponentTemplate`)

Built-in logical UI components are organized under `crates/picus_core/src/components/*.rs`. Each UI component module owns its logical component shape, template-part policy, and the trait contract used for registration.

The unifying trait is `UiComponentTemplate`:

- `expand(world, entity)` for one-time logical→template expansion
- `project(&T, ProjectionCtx) -> UiView` for ECS→Masonry projection

### 4.2 Streamlined Registration API

`AppPicusExt` exposes `.register_ui_component::<T: UiComponentTemplate>()`. One call performs projector registration, `Added<T>` expansion system hookup, and selector type alias registration. Built-in UI components are registered centrally via `PicusBuiltinsPlugin`, so user apps only call this for explicit custom usage.

### 4.3 Built-in Component Coverage

The built-in ECS UI components registered through `components/mod.rs` currently include:

**Interactive controls:** `UiButton`, `UiCheckbox`, `UiSlider`, `UiSwitch`, `UiTextInput`, `UiComboBox` (with `UiDropdownMenu` and `UiDropdownItem`), `UiRadioGroup`, `UiTabBar`, `UiTreeNode`, `UiMenuBar`, `UiMenuBarItem`, `UiMenuItemPanel`, `UiColorPicker` (with `UiColorPickerPanel`), `UiDatePicker` (with `UiDatePickerPanel`), `UiThemePicker` (with `UiThemePickerMenu`), `UiPopover`

**Display and container widgets:** `UiBadge`, `UiProgressBar`, `UiDialog`, `UiScrollView`, `UiTable`, `UiTooltip`, `UiSpinner`, `UiGroupBox`, `UiSplitPane`, `UiToast`

In addition, the core projector layer provides structural ECS markers such as `UiRoot`, `UiOverlayRoot`, `UiFlexColumn`, `UiFlexRow`, and `UiLabel`.

### 4.4 Portal-Based `UiScrollView`

Implemented as a logical ECS UI component projected through a Masonry portal view, with explicit scroll state (`scroll_offset`, `content_size`) and optional external scrollbar parts.

- `PreUpdate` reads back portal geometry from Masonry and synchronizes ECS `viewport_size`/`content_size` each frame
- `viewport_size` acts as an initial logical seed, but the live viewport geometry follows parent layout constraints in Masonry and is synchronized back into ECS every frame
- `scroll_offset` is strictly clamped to physical bounds after drag/wheel/layout-sync updates
- Wheel deltas are routed from deepest hit target outward and consumed by the first ancestor `UiScrollView` that can actually move, preventing boundary desync in nested scroll views

## 5. Event Handling

### 5.1 Zero-Closure ECS Button Path

To remove user-facing closure boilerplate:

- `EcsButtonView` implements `xilem_core::View` on top of a custom wrapper widget
- `EcsButtonWithChildView` provides the same event semantics for composed button content
- On interaction, it emits structural events (`PointerEntered`, `PointerPressed`) and typed ECS actions (`UiEventQueue`)

This keeps built-in interactive controls on a unified ECS event route even in headless-runtime mode.

### 5.2 Typed Action Queue

`UiEventQueue` is a Bevy `Resource` backed by a lock-free `SegQueue`. Widgets push type-erased actions. Bevy systems drain typed actions via `drain_actions::<T>()` non-destructively for multiple consumers.

### 5.3 Pointer Event Bubbling

`UiPointerHitEvent` represents a hit-tested pointer event before ECS bubbling. `UiPointerEvent` is emitted for each ancestor in the hierarchy with `consumed` flag. The `StopUiPointerPropagation` marker component stops bubbling at the tagged entity.

### 5.4 Overlay Pointer Routing

`OverlayPointerRoutingState` tracks suppressed presses/releases to prevent trigger buttons from receiving the corresponding release after an overlay consumes a click. This avoids sticky-pressed visual states.

## 6. Styling Engine

The runtime supports a data-driven style pipeline with four phases:

1. **Inline style overrides:** `InlineStyle` (preferred consolidated override) or legacy split components (`LayoutStyle`, `ColorStyle`, `TextStyle`, `StyleTransition`)
2. **Selector-based stylesheet & cascade:** `StyleSheet` resource mapped from `.ron` files
3. **Pseudo classes:** `InteractionState { hovered, pressed }` synchronized from interaction events (mutated in-place to avoid archetype churn)
4. **Computed-style cache & incremental invalidation:** Resolves final traits via `StyleDirty` / `ComputedStyle`

### 6.1 Smooth Transitions

`TargetColorStyle` + `CurrentColorStyle` are driven by `bevy_tween` time-runner + component-tween state targeting `CurrentColorStyle`, allowing smooth micro-interaction transforms and color transitions without snapping. `ColorStyleLens` implements `Interpolator` for RGBA channels with easing (default `QuadraticInOut`).

### 6.2 Base vs Active Stylesheet Tiers

Runtime styling distinguishes two explicit tiers: `BaseStyleSheet` (embedded Fluent baseline) and `ActiveStyleSheet` (runtime override tier). Active rules cascade over baseline rules by priority.

The active tier can come from a hot-reloaded asset path (`AppPicusExt::load_style_sheet`, tracked by `ActiveStyleSheetAsset`) or be applied directly from embedded RON text (`AppPicusExt::load_style_sheet_ron`). Runtime selectors/tokens owned by the active tier override the baseline tier without permanently mutating the embedded theme bundle.

Baseline Fluent theme includes a global `Type("UiRoot")` preflight rule for app-surface background, and the `UiRoot` projector stretches to full viewport so root background styling consistently covers the entire window.

`PicusPlugin` boots with embedded **Fluent Dark** by default. Built-in Fluent theming is provided as a **single multi-variant bundle** (`fluent_theme.ron`) that contains named variants (`dark`, `light`, `high-contrast`). The styling system parses/registers this bundle into `RegisteredStyleVariants`.

Runtime variant selection is state-driven via `ActiveStyleVariant`. Apps set desired variant by name through `set_active_style_variant_by_name(...)`, and `sync_active_style_variant` automatically applies it to `BaseStyleSheet` + runtime `StyleSheet`. Plugin bootstrap sets the theme file's own default variant as active, and the first `Update` pass applies it automatically.

Theme activation no longer exposes `install_*` APIs. The only public path is active-variant state plus automatic sync.

Variant bundles support top-level shared `rules`/`tokens` plus per-variant overrides. This keeps common selector graphs out of any single variant and lets each variant focus on palette/token deltas.

When an entity has no matched selector rules and no inline style sources, style resolution intentionally uses a transparent text fallback so the UI does not inherit Masonry/Xilem intrinsic default text appearance.

### 6.3 Hit-Testing Invariants

Layout-affecting styles (padding/border/background) are applied directly to the target UI component widget itself, ensuring Masonry's hit-testing matches the structural box model users see, specifically on bounded overlays/dialogs vs global backgrounds.

### 6.4 Selector Model and Token Support

Selectors support: `Type` (component `TypeId`), `TypeName` (string component name), `Class` (style class), `PseudoClass` (`:hover`, `:pressed`), `And` (conjunction), and `Descendant` (ancestor-descendant relationships). `StyleTypeRegistry` resolves selector type names loaded from RON into actual ECS component types.

Style rules support token-aware values via `StyleValue::Var(String)`, allowing stylesheet rules to reference named tokens from the active `StyleSheet`.

### 6.5 Supported Style Properties

**Layout:** `padding`, `gap`, `corner_radius`, `border_width`, `justify_content` (flex main-axis), `align_items` (flex cross-axis), `scale`

**Colors:** `bg`, `text`, `border`, plus pseudo overrides `hover_*` and `pressed_*`

**Text:** `size`, `text_align` (`Start`, `Center`, `End`)

**Font family:** `font_family: Option<Vec<String>>` (font stack)

**Box shadow:** `box_shadow`

**Transitions:** `transition: Option<StyleTransition>` with `duration` in seconds

## 7. Overlay and Modal System

`picus_core` includes a built-in ECS overlay model using floating/portal roots natively stacked through Masonry.

### 7.1 Layering and Positioning

- **Centralized Layering Model:** `OverlayStack` maintains top-most order. `sync_overlay_stack_lifecycle` keeps it pruned.
- **Universal Placement Model:** `OverlayPlacement` handles Center/Top/Bottom/Left/Right and Start/End alignments. `sync_overlay_positions` calculates clamping and auto-flipping against screen edges.
- **Shared anchored popover metadata:** `UiPopover` centralizes anchor/placement/auto-flip configuration for anchored floating surfaces so built-in dropdowns, tooltips, picker panels, and app-level popovers reuse the same placement path.
- **Built-in Floating Widgets:** `UiDialog` (modal, optional fixed width/height hints for overlay placement and projection sizing), `UiComboBox` (anchor), `UiDropdownMenu` (floating list), `UiTooltip` (hover-anchor), `UiToast` (default bottom-end placement, configurable placement/width/close-button), `UiMenuItemPanel`, `UiColorPickerPanel`, `UiDatePickerPanel`, `UiThemePickerMenu`
- **Dialog close contract:** `UiDialog` optionally carries a typed close-action hook. Both the built-in header close control (rendered in the top-right dialog chrome) and outside-click dismissal route through the same overlay helper, which emits the hook through `UiEventQueue` before despawning. Dialogs without the hook keep the existing despawn-only behavior.
- **FOUC prevention invariant:** overlay projectors must render with fully transparent resolved styles while `OverlayComputedPosition.is_positioned == false`, then become visible once synchronized placement is available.
- **Generic temporary lifecycle:** `AutoDismiss { timer }` supports timer-driven teardown for temporary overlays (e.g., toasts).

### 7.2 Layered Dismissal and Blocking Flow

`handle_global_overlay_clicks` dynamically evaluates pointer location against the top-most overlay using `RenderRoot::get_hit_path(physical_pos)` and all widget IDs bound to that overlay entity, with an `OverlayComputedPosition` rectangle fallback. This avoids false outside-click dismissal when interacting with deeply nested portal/menu content.

Clicks outside the opaque overlay root cause dismissals without disrupting interactive siblings. Optional `UiOverlayRoot` dimly renders full-view backgrounds without structurally wrapping modal UI boundaries.

When clicking an overlay anchor to close an anchored overlay, pointer suppression is press-only for the consumed click. This avoids stale suppressed-release state that can otherwise leave trigger buttons in a sticky pressed visual/input state.

### 7.3 Overlay Reparenting

`reparent_overlay_entities` automatically moves overlay entities (dialogs, dropdowns, menus, tooltips, toasts, pickers) under the global `UiOverlayRoot` to keep them outside normal layout clipping hierarchies.

## 8. Iconography

Built-in directional indicators and radio markers are provided through a dedicated `picus_core::icons` module backed by `lucide-icons` icon data/font assets. The plugin registers bundled Lucide font bytes at startup and icon text styling uses the upstream Lucide family name (`"lucide"`) so rendering remains stable across locales and system font configurations.

## 9. Assets and Internationalization

### 9.1 Font Bridge

`XilemFontBridge` manages moving Bevy `Asset<Font>` to Masonry's system. Registers font bytes from `collect_bevy_font_assets` directly to `MasonryRuntime` using `sync_fonts_to_xilem` in the asset queue lifecycle. Supports both asset-server loading and direct byte/path registration via `AppPicusExt::register_xilem_font_bytes/path`.

### 9.2 Synchronous i18n Registry

Centralized in `AppI18n`. Synchronous setup through `.register_i18n_bundle()`. Uses declarative font stacks applied based on locale priorities. `resolve_localized_text` resolves `LocalizeText` component keys through the active bundle, falling back to the key or provided fallback text.

## 10. ECS Data Model & Synthesis Pipeline

### 10.1 Data Model

Core built-ins: `UiRoot`, `UiFlexColumn`, `UiFlexRow`, `UiLabel`, `UiButton`, `LocalizeText`. Node identities for projection context use `entity.to_bits()`.

### 10.2 Synthesis Pipeline

Driven via `UiProjectorRegistry`. `PostUpdate` executes:

1. Gather `UiRoot` (and `UiOverlayRoot`) entities via `gather_ui_roots` (overlays sorted last)
2. Recursive projection (`project()`) through `synthesize_entity`
3. Store `SynthesizedUiViews`
4. Rebuild retained Masonry root in `MasonryRuntime`

When more than one root is present, runtime rebuild composes the synthesized roots into a full-viewport `zstack` aligned to top-left before calling Xilem Core rebuild.

The synthesis stats resource tracks `root_count`, `node_count`, `cycle_count` (cycles detected), `missing_entity_count`, and `unhandled_count`.

## 11. Developer Ergonomics

### 11.1 Two-Level UI Componentization Policy

- **Micro-componentization:** Reusable fragments returned as pure Rust View helpers (`UiView` or `impl View`)
- **Macro-componentization:** UI regions mapped purely to ECS via `register_ui_component::<T>()`

### 11.2 Bevy-Native Run Helpers

`run_app()` and `run_app_with_window_options()` avoid raw setup tasks, bootstrapping native `bevy_winit` safely to Bevy systems for seamless desktop lifecycle apps. They auto-enable Bevy's native window plugins (`AccessibilityPlugin` + `InputPlugin` + `WindowPlugin` + `WinitPlugin`) before `App::run()`.

## 12. picus_surface: External Window Surface Bridge

`picus_surface` provides a Vello rendering surface attached to an externally owned Bevy window. It manages:

- wgpu instance/device/queue lifecycle
- Surface configuration and resizing
- Scene rendering with DPI-aware scaling
- Texture blitting and presentation
- AMD Windows compatibility workaround (premultiplied alpha)

The bridge is created from a `RawHandleWrapper` (Bevy's window handle wrapper) and synchronized with window metrics (physical size, logical size, scale factor).

## 13. picus_activation: Deep Link and Single-Instance Runtime

`picus_activation` handles application activation, single-instance enforcement, and custom URI protocol registration across platforms.

### 13.1 Responsibilities

- **Single-instance gate** (`app-single-instance`): `notify_if_running(app_id)` detects an already-running primary instance. On first instance, `start_primary(app_id, ...)` keeps primary ownership alive.
- **Non-macOS activation forwarding** (`ipc-channel` one-shot rendezvous): Primary continuously rotates `IpcOneShotServer` endpoints and publishes the active server name in a per-app rendezvous file under temp dir. Secondaries read that name, connect through `IpcSender`, forward URI payloads, and wait for explicit ack (`Ack`/`Nack`) over an embedded IPC ack channel before exiting.
- **macOS activation delivery is Apple-Event-native:** `picus_activation` installs an `NSAppleEventManager` `kAEGetURL` handler through `objc2`, receives custom-scheme callbacks in the running app process, and feeds them directly into the activation service queue without the IPC rendezvous path.
- **Custom URI protocol registration is crate-native:** Implements its own protocol registration instead of depending on `sysuri`. Windows uses HKCU registry (`Software\Classes/<scheme>`); Linux writes `~/.local/share/applications/*.desktop` entry and runs `xdg-mime default` plus `update-desktop-database`.
- **Startup URI collection:** Activation scans raw process arguments directly, normalizes quoted values, filters callback URIs by case-insensitive scheme match, and deduplicates before secondary-to-primary IPC forwarding.
- **macOS bundle workflow:** Apps supply their own `Info.plist` through `MacosBundleConfig`. `picus_activation` reads that plist, creates/updates a runnable `.app` bundle around the current executable when needed, registers it with Launch Services (`lsregister`), and then requests the current app bundle become the default URL-scheme handler via `NSWorkspace::setDefaultApplicationAtURL:toOpenURLsWithScheme:completionHandler:` during startup.
- **macOS current-bundle detection:** When the process is already running from an application bundle, activation resolves that bundle through `NSBundle::mainBundle()` instead of inferring solely from `current_exe()`, keeping Launch Services registration and default URL-scheme ownership pinned to the real running app bundle.

### 13.2 Activation Bootstrap Flow

`bootstrap(config)` returns `BootstrapOutcome::Primary(ActivationService)` or `BootstrapOutcome::SecondaryForwarded`.

Primary instance receives:

- `startup_uris` (URIs from command-line arguments at launch)
- `drain_uris()` (subsequent activation URIs delivered via IPC/Apple Events)

Secondary instances forward URIs to primary and exit immediately.

## 14. Examples and Workspace Members

The workspace currently includes these example members from `Cargo.toml`:

- `async_downloader`
- `calculator`
- `chess_game`
- `game_2048`
- `overlay_hit_routing`
- `pixcus`
- `shared_utils`
- `timer`
- `todo_list`
- `ui_showcase`

The `pixcus` example currently exposes authentication through a sidebar-footer login entry that opens a modal overlay dialog for Pixiv OAuth inputs. Once authenticated, the same sidebar footer switches to an avatar-based account trigger with a compact logout popover that reuses the shared anchored popover placement path. Selecting an illustration opens a `UiDialog`-backed artwork detail modal that expands to a near-fullscreen two-column layout sized from current `ViewportMetrics` rather than a fixed `1320x880`: a large artwork hero on the left and a scrollable right rail stacking artwork, author, image, caption, and tag metadata while the built-in header close affordance remains visible in the top-right chrome.

## 15. Plugin System

`PicusPlugin` wires the entire framework:

- Ensures `TaskPoolPlugin`, `AssetPlugin`, and `DefaultTweenPlugins` are present
- Adds `TimePlugin` and `PicusBuiltinsPlugin`
- Registers core resources: `UiProjectorRegistry`, `SynthesizedUiViews`, `UiSynthesisStats`, `UiEventQueue`, `StyleSheet`, `BaseStyleSheet`, `ActiveStyleSheet`, `ActiveStyleSheetAsset`, `ActiveStyleSheetSelectors`, `ActiveStyleSheetTokenNames`, `ActiveStyleVariant`, `AppliedStyleVariant`, `RegisteredStyleVariants`, `StyleAssetEventCursor`, `XilemFontBridge`, `AppI18n`, `OverlayStack`, `OverlayPointerRoutingState`, `MasonryRuntime`
- Adds Bevy message types for window/input events
- Registers systems to `PreUpdate`, `Update`, `PostUpdate`, and `Last` (see section 2.2)
- Registers embedded Fluent theme variants and sets default active variant
- Registers core projectors via `register_core_projectors`

`PicusBuiltinsPlugin` registers all built-in UI components.

## 16. Non-Goals

- Custom render-graph bridging out of scope; sticks to Masonry retained runtime ownership implicitly
- No Bevy render-graph integration; uses explicit Vello surface via `picus_surface`
- No automatic closure-based event handling; ECS queue is the unified path
- No CSS cascade complexity beyond selector-based rules and inline overrides
- No inherited style contexts; styles are per-entity with descendant selector matching

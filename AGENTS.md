# AGENTS.md

Repository guide for automated agents and humans working in the same style. Keep
this file focused on enforceable process rules, public architecture contracts, and
cross-cutting design decisions that code comments cannot express well.

## 1. Operating Rules

1. **Design contract**
   - Keep implementation, examples, tests, and this file aligned.
   - Update this file with changes to public architecture, styling behavior, public
     APIs, config/schema, activation protocol, example UX, or repository workflow.

2. **Verification**
   - Add or update tests for behavior changes.
   - Use Rust 1.95.0 or newer for Bevy 0.19 workspace checks.
   - Run `cargo test` for every change.
   - For Rust changes, run `cargo fmt` and
     `cargo clippy --all-targets --all-features -- -D warnings`.
   - Leave no compiler or Clippy warnings.

3. **Rust dependencies**
   - After adding a Rust dependency, check whether `cargo upgrade` is available.
   - If available, run it and prefer the newest reasonable compatible version.
   - If unavailable, skip version probing and continue.

4. **Runtime commands**
   - Avoid `cargo run` unless an interactive issue or runtime log requires it.
   - Prefer tests, static checks, and targeted diagnostics.

5. **Fluent IDs**
   - Fluent `.ftl` message IDs use hyphen namespacing, such as
     `nav-home-title`.
   - Do not use dots in Fluent message IDs; Fluent reserves dots for attributes.
   - Style class names may use dots, such as `todo.root`.

6. **Autonomy**
   - Execute straightforward tasks directly.
   - Ask the user only for architecture-level choices with meaningful trade-offs.

7. **Third-party dependencies**
   - Core Linebender dependencies track official upstream only. Prefer crates.io
     releases when every needed crate is published; otherwise use official
     `https://github.com/linebender/xilem.git` Git dependencies pinned by commit.
   - Do not depend directly on upstream `masonry` or upstream `xilem`.
     `picus_masonry` and local `xilem_masonry` are Picus-owned compatibility
     crates copied from upstream and retargeted to `masonry_core`.
   - Do not reintroduce `third_party` submodules or user-fork dependencies unless
     a required capability cannot be expressed against official upstream crates.
   - Temporary local Cargo `[patch]` or path overrides are allowed for validation;
     remove them unless they are part of the intended design.

## 2. Workspace

`picus` is a Bevy-first UI framework that combines ECS state management with a
retained Masonry Core UI runtime. `picus_core` depends directly on
`masonry_core` and the Picus-owned local `xilem_masonry` adapter. The
higher-level upstream `masonry` and upstream `xilem` crates are not dependencies
and should not be reintroduced.

Crates:

- `picus_core`: ECS-driven UI projection, styling, overlays, built-in components,
  fonts, icons, and runtime integration.
- `picus_masonry`: Picus-owned Masonry widget/property set copied from upstream
  Masonry and retargeted to `masonry_core`.
- `xilem_masonry`: Picus-owned Xilem-compatible view adapter copied from upstream
  `xilem_masonry` and retargeted to `picus_masonry`.
- `picus_surface`: Vello/wgpu rendering surface for an externally owned Bevy
  window.
- `picus_activation`: single-instance activation, custom URI protocol handling,
  IPC forwarding, and platform registration.

Example applications live under `examples/`: `async_downloader`, `calculator`,
`chess_game`, `game_2048`, `overlay_hit_routing`, `pixcus`, `shared_utils`,
`timer`, `todo_list`, and `gallery`.

## 3. Runtime Architecture

Bevy owns scheduling, windows, and input. Masonry Core runs as a retained runtime
resource driven by Bevy systems; GUI apps use Bevy's native `App::run()` and
`bevy_winit` lifecycle.

`MasonryRuntime` is a `NonSend` resource containing a `masonry_core::app::RenderRoot`,
retained view state, pointer state, primary-window metrics, `picus_surface` state,
and Vello renderer state.

System stages:

- `PreUpdate`: input, font, interaction, overlay-click, scroll-geometry, and widget
  action synchronization.
- `Update`: overlay lifecycle, style/theme synchronization, dirty marking, action
  handling, and transition ticking.
- `PostUpdate`: UI synthesis, retained-tree rebuild, and IME synchronization.
- `Last`: explicit Vello paint/present pass.

Runtime invariants:

- Initial primary-window attachment injects a logical resize before hit testing.
- Retained UI rendering does not depend on Bevy render-graph integration.
- The paint pass redraws Masonry Core, renders through `picus_surface`, blits to the
  swapchain, presents, and requests the next redraw.

## 4. Input, IME, and Hit Testing

`inject_bevy_input_into_masonry` translates Bevy window/input messages into
Masonry Core pointer, text, IME, focus, resize, and rescale events.

Pointer invariants:

- `Window::physical_cursor_position()` on the primary window is the source of
  injected pointer coordinates.
- Pointer injection is skipped when physical cursor data is unavailable.
- Click injection sends a pointer move before down/up events.
- Window resize injection uses logical dimensions.

IME state flows both ways: Bevy events enter Masonry Core, and Masonry Core IME
callbacks update Bevy window `ime_enabled` and `ime_position`.

Layout-affecting styles such as padding, border, background, and corner radius are
applied to the target widget so Masonry Core hit testing matches the visible box model.

## 5. ECS UI Model

Logical UI components live under `crates/picus_core/src/components/*.rs`.
Built-ins are registered through `PicusBuiltinsPlugin`; applications register custom
components with `AppPicusExt::register_ui_component::<T>()`.

`UiComponentTemplate` is the component contract:

- `expand(world, entity)` creates template children once.
- `project(&T, ProjectionCtx) -> UiView` converts ECS state into a Picus retained
  view.

Projection uses `entity.to_bits()` for stable node identities. Core root/container
types include `UiRoot`, `UiOverlayRoot`, `UiFlexColumn`, `UiFlexRow`, `UiGrid`
with MewUI-style `UiGridLength` track intent and `UiGridCell` attached
placement, `UiLabel`, `UiButton`, `UiCanvas`/`UiCanvasCommand` plus
`UiCanvasPosition` child positioning, `UiImage`, `UiTextInput`,
`UiPasswordInput`, `UiMultilineTextInput`, `UiListView`, `UiTable`,
`UiDataTable`, and `LocalizeText`.

Priority built-ins (`UiButton`, `UiBadge`, `UiProgressBar`, `UiSwitch`, and
`UiCheckbox`) own their Picus-composed visual structure instead of exposing raw
compatibility widget appearance.

## 6. Synthesis and Events

UI synthesis is driven by `UiProjectorRegistry` in `PostUpdate`. It gathers
`UiRoot` and `UiOverlayRoot` entities, projects ECS trees recursively, stores
`SynthesizedUiViews`, and rebuilds `MasonryRuntime`. Multiple roots compose into a
full-viewport top-left `zstack`, with overlays sorted last.

Interactive controls use the ECS event route:

- `EcsButtonView` and `EcsButtonWithChildView` emit pointer interaction events and
  typed actions through `UiEventQueue`.
- `UiEventQueue` stores type-erased actions and supports typed non-destructive
  drains through `drain_actions::<T>()`.
- `UiPointerHitEvent` is the hit-tested source event; `UiPointerEvent` bubbles
  through ancestors until `StopUiPointerPropagation`.
- `OverlayPointerRoutingState` suppresses consumed overlay click paths so trigger
  controls do not remain pressed.

## 7. Styling Contract

The styling system is CSS-like, ECS-driven, and centered in
`crates/picus_core/src/styling.rs`.

Style sources:

- `BaseStyleSheet`: embedded Fluent baseline variants.
- `ActiveStyleSheet`: runtime override tier from loaded assets or embedded RON.
- `InlineStyle`: preferred inline component containing layout, color, text, and
  transition intent.
- Legacy split inline components remain supported: `LayoutStyle`, `ColorStyle`,
  `TextStyle`, and `StyleTransition`.

Selectors support component type, registered type name, class, `:hover`, `:pressed`,
conjunction, and descendant matching. Style values may reference named tokens.

Resolution order, low to high:

1. Selector-matched rules from base and active sheets.
2. Inline style components.
3. Compatibility pseudo color overrides from `ColorStyle`.
4. Animated color override from `CurrentColorStyle`.

Runtime styling invariants:

- `InteractionState { hovered, pressed }` is stable component state.
- `StyleDirty`, `ComputedStyle`, and target color state cache resolved style.
- Descendant selector invalidation propagates from changed ancestors.
- Entities with no matched rules and no inline style resolve to transparent text
  fallback.
- `ComputedStyle.font_family` carries resolved font-family data for projectors.
- Color transitions use `bevy_tween`; projectors read resolved plus animated style
  through `resolve_style`.

Built-in Fluent theming is a multi-variant bundle at
`crates/picus_core/src/theme/fluent_theme.ron` with `dark`, `light`, and
`high-contrast` variants. Runtime selection uses `ActiveStyleVariant` and
`set_active_style_variant_by_name(...)`.

Projectors should resolve style through the styling helpers, then apply it with the
widget, label, or text-input style helpers. Use
`resolve_style_for_entity_classes(...)` for pseudo-state-sensitive class styling.

## 8. Scroll Views and Overlays

`UiScrollView` is a logical ECS component projected through a Masonry Core portal view.
It stores scroll offset, viewport/content geometry, and optional external scrollbar
parts.

Scroll invariants:

- Masonry Core portal geometry synchronizes back to ECS each frame.
- Live viewport size follows Masonry Core layout constraints.
- Scroll offsets clamp to physical bounds after wheel, drag, and geometry updates.
- Nested wheel routing starts at the deepest hit target and stops at the first
  scroll view that can move.

The overlay system uses Masonry Core floating/portal roots and `OverlayStack` ordering.
`OverlayPlacement` handles screen placement, clamping, and auto-flip behavior.
`UiPopover` is the shared anchored-placement model for dropdowns, tooltips, picker
panels, popovers, and related floating surfaces.

Overlay invariants:

- Overlay projectors render transparent content until
  `OverlayComputedPosition.is_positioned`.
- Outside-click dismissal checks the top overlay's hit path and bound widget IDs,
  with rectangle fallback.
- Dismissed dialogs emit their typed close hook through `UiEventQueue` before
  despawn when such a hook exists.
- Overlay entities reparent under `UiOverlayRoot` to avoid normal layout clipping.
- `UiToast` uses configurable placement and defaults to bottom-end behavior.

## 9. Assets, Fonts, Icons, and I18n

`picus_core::icons` uses bundled Lucide icon/font data. Icon text styling uses the
upstream Lucide family name, `"lucide"`.

`XilemFontBridge` is the legacy-named font bridge that registers Bevy font assets
with Masonry Core. Fonts can come from the asset server, direct bytes, or direct
paths through `AppPicusExt`.

`AppI18n` is the synchronous i18n registry. `LocalizeText` resolves through the
active bundle and falls back to the key or explicit fallback text.

## 10. Surface and Activation

`picus_surface` owns wgpu instance/device/queue state, surface configuration,
DPI-aware scene rendering, swapchain presentation, and the Windows AMD premultiplied
alpha compatibility path. It attaches through raw window handles and tracks physical
size, logical size, and scale factor.

`picus_activation` provides:

- `bootstrap(config)` returning either `Primary(ActivationService)` or
  `SecondaryForwarded`.
- Single-instance ownership and secondary-to-primary forwarding.
- Custom URI protocol registration on Windows, Linux, and macOS.
- Non-macOS IPC forwarding through rotating one-shot rendezvous endpoints.
- macOS activation through Apple Events and bundle-aware Launch Services
  registration.
- Startup URI collection from process arguments and subsequent URI delivery through
  `ActivationService::drain_uris()`.

## 11. Plugin and App Helpers

`PicusPlugin` installs the framework resources, built-in message types, schedule
systems, `DefaultTweenPlugins`, embedded Fluent variants, and core projectors.
`PicusBuiltinsPlugin` registers built-in UI components.

`run_app()` and `run_app_with_window_options()` bootstrap desktop apps with Bevy
window/input/accessibility/winit plugins and then call `App::run()`.

Use two UI composition layers:

- Rust view helpers for reusable local fragments.
- ECS components registered through `register_ui_component::<T>()` for reusable UI
  regions.

## 12. Example UX Contracts

`pixcus` authentication lives in the sidebar footer. The unauthenticated state opens
a modal OAuth dialog; the authenticated state shows an avatar trigger with a compact
logout popover.

Selecting a Pixiv illustration opens a `UiDialog` artwork detail modal sized from
current `ViewportMetrics`. The modal uses a two-column layout: large artwork on the
left and a dedicated `UiScrollView` metadata rail on the right. Long captions and
tags scroll in that rail, and the built-in Lucide close control remains visible in
the dialog chrome.

## 13. Reference Files

- Styling: `crates/picus_core/src/styling.rs`
- Plugin wiring: `crates/picus_core/src/plugin.rs`
- Built-in components: `crates/picus_core/src/components/`
- ECS button widget: `crates/picus_core/src/widgets/ecs_button_widget.rs`
- ECS button views: `crates/picus_core/src/views/ecs_button_view.rs`
- Theme bundle: `crates/picus_core/src/theme/fluent_theme.ron`
- Surface bridge: `crates/picus_surface/`
- Activation: `crates/picus_activation/`

## 14. Non-Goals

- Retained UI rendering does not use Bevy render-graph integration.
- Built-in interactive controls do not use user-facing closure event handlers.
- Styling does not implement full CSS cascade semantics.
- Inherited style contexts are unsupported; styles are per-entity with selector
  matching.

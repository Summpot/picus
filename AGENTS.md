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
   - Run `cargo check` and `cargo test` once per change set.
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

6. **Committing**
   - Run `cargo fmt` only before committing, as part of the final pre-commit workflow.
   - Run `cargo clippy --all-targets --all-features -- -D warnings` before committing.
   - Formatting-only changes are not semantic modifications. Do not rebuild or retest after a pure formatting change.
   - All commit messages must be written in English.

## 2. Workspace

`picus` is a Bevy-first UI framework that combines ECS state management with a
retained Masonry Core UI runtime. User applications depend on the public `picus`
facade crate. `picus_core` is the implementation crate and depends directly on
`masonry_core` and the Picus-owned local `picus_view` adapter.
`picus_view` consumes `picus_widget` for the underlying widget, property, layer,
and theme runtime. The higher-level upstream `masonry` and upstream `xilem`
crates are not dependencies and should not be reintroduced.

Crates:

- `picus`: application-facing facade with grouped public modules (`app`,
  `components`, `projection`, `styling`, `events`, `overlay`, `runtime`,
  `i18n`, and `scene`) plus transitional root re-exports.
- `picus_core`: implementation crate for ECS-driven UI projection, styling,
  overlays, built-in components, fonts, icons, and runtime integration.
- `picus_widget`: Picus-owned retained widget/property backend and the long-term
  home for incremental widget rewrites on top of `masonry_core`.
- `picus_view`: Picus-owned Xilem-compatible view adapter on top of
  `picus_widget` and `xilem_core`.
- `picus_surface`: Vello/wgpu rendering surface for an externally owned Bevy
  window.

Embedded fork crates under `thirdparty/CodeWhale/crates/*` are picus workspace
members (see `members` in the root `Cargo.toml`) so their `*.workspace = true`
inheritance resolves against the picus root. Their `version`/`license`/`repository`
are pinned per-crate (not inherited) to preserve the fork's identity
(`codewhale-*` v0.8.66). The fork's own `thirdparty/CodeWhale/Cargo.toml`
workspace root is excluded from picus to avoid dual membership; it is kept for
standalone maintenance of the fork. Shared dep versions
(`tokio = "full"`, `chrono` with `serde`, `reqwest` with `stream`+`socks`,
`rusqlite` bundled, etc.) are unified in the picus `[workspace.dependencies]`
table to stay compatible with the fork's own manifest.

Example applications live under `examples/`: `async_downloader`, `calculator`,
`chess_game`, `game_2048`, `gallery`, `overlay_hit_routing`, `picuscode`,
`shared_utils`, `timer`, and `todo_list`.

## 2.1. example_picuscode / CodeWhale integration

`example_picuscode` is a Codex-desktop-style GUI for CodeWhale. It embeds the
CodeWhale runtime in-process via a dedicated bridge thread (see
`examples/picuscode/src/bridge.rs`):

- A background thread owns a tokio `Runtime` plus `codewhale_core::Runtime`,
  `codewhale_config::ConfigStore`, and `codewhale_state::StateStore`.
- Config and state resolve against the same default `~/.codewhale/` paths an
  installed `codewhale` binary uses, so picuscode is fully config- and
  state-compatible with the user's installed CodeWhale.
- The ECS world talks to the bridge through `crossbeam_channel`: it pushes
  `BridgeRequest` values and polls `BridgeEvent` values each `PreUpdate` frame,
  keeping async runtime off the Bevy render thread.
- Model turns stream through the OpenAI-compatible `/chat/completions` SSE
  endpoint using `codewhale_config::resolve_runtime_options` + `ModelRegistry`
  for provider/model/api-key resolution, so the same provider setup an
  installed codewhale uses is honored. Only `WireFormat::ChatCompletions`
  providers are streamed in this phase; Anthropic Messages / Responses API
  support is a follow-up.
- The UI shell: a primary chat window (status-rich title bar, thread sidebar
  with per-thread previews, scrollable streaming `UiStreamingMarkdown`
  transcript, composer, and status bar) plus secondary About and grouped
  Settings windows bound via `UiWindow`. Settings edits are staged in
  `PicusState::config_edits` and persisted through the bridge only when the
  user presses Save.
- `spawn_bridge_with_config_path(Option<PathBuf>)` exists for tests so they
  never touch the user's real `~/.codewhale/` config.

Phase 1 covers thread lifecycle (create/list/read/rename/archive), config
get/set/list/reload, and streaming chat. Full TUI feature parity (diff/patch
viewing, tool-call approval, MCP management, hooks, execpolicy, context/fleet/
skills) is tracked as follow-up work; the bridge already exposes the
`codewhale-protocol` `EventFrame` surface needed to render those frames as UI.

## 3. Runtime Architecture

Bevy owns scheduling, windows, and input. Masonry Core runs as a retained runtime
resource driven by Bevy systems; GUI apps use Bevy's native `App::run()` and
`bevy_winit` lifecycle.

`MasonryRuntime` is a `NonSend` resource keyed by Bevy window entity. It holds one
`WindowRuntime` per attached window, each owning a `masonry_core::app::RenderRoot`,
retained view state, pointer/keyboard state, IME channel, `picus_surface` state, and
Vello renderer state. The primary window (Bevy `PrimaryWindow`) is auto-attached; other
windows attach as secondary runtimes. Access the primary window via `primary()` /
`primary_mut()`, or a specific window via `window(entity)` / `window_mut(entity)`. Use
`ensure_window(entity, is_primary)` to create a runtime for a window on demand. A
`UiWindow(Entity)` binding component on a `UiRoot` directs synthesis into a specific
window; roots without it bind to the primary window.

System stages:

- `PreUpdate`: input, font, interaction, overlay-click, scroll-geometry, view
  message routing, and widget action synchronization.
- `Update`: overlay lifecycle, style/theme synchronization, dirty marking, action
  handling, and transition ticking.
- `PostUpdate`: UI synthesis, retained-tree rebuild, and IME synchronization.
- `Last`: explicit Vello paint/present pass.

Runtime invariants:

- Initial primary-window attachment injects a logical resize before hit testing.
- Secondary windows are auto-attached as they appear; headless contexts (tests
  without a winit handle) create a fallback 1024×768 runtime so synthesis and
  hit-testing still work.
- Window runtimes are pruned when their Bevy window is closing or has lost its
  `Window` component, dropping retained surfaces before the native window is
  destroyed.
- Retained UI rendering does not depend on Bevy render-graph integration.
- The paint pass redraws Masonry Core only when the retained runtime has pending
  paint/animation work or has not produced its first frame, renders through
  `picus_surface`, blits to the swapchain, presents, and forwards Masonry redraw
  requests through Bevy `RequestRedraw`.
- Font registration broadcasts to all attached window runtimes, is retained for
  future windows, and is replayed into each new `WindowRuntime` when it attaches.

## 4. Input, IME, and Hit Testing

`inject_bevy_input_into_masonry` translates Bevy window/input messages into
Masonry Core pointer, text, IME, focus, resize, and rescale events. Events are
routed to the per-window runtime identified by their `window` field; events for
windows without an attached runtime are ignored.

Pointer invariants:

- `Window::physical_cursor_position()` on the event's window is the source of
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

Application components implement this trait directly so projector signatures
remain constrained by the trait contract.

Projection uses `entity.to_bits()` for stable node identities. Core root/container
types include `UiRoot`, `UiOverlayRoot`, `UiFlexColumn`, `UiFlexRow`, `UiGrid`
with `UiGridLength` track intent and `UiGridCell` attached
placement, `UiLabel`, `UiButton`, `UiCanvas`/`UiCanvasCommand` plus
`UiCanvasPosition` child positioning (with `right`/`bottom` anchoring against
`UiCanvas::size`), `UiImage`, `UiSearch`, `UiTextInput`, `UiPasswordInput`,
`UiMultilineTextInput`, `UiListView`, `UiTable`, `UiDataTable` with `UiDataCell`
text/image cell templates, `UiNavigationView` with ECS-backed `UiNavigationItem`
sidebar template entities, `UiNumericUpDown`, `UiMarkdown`, `UiStreamingMarkdown`,
and `LocalizeText`.

Icon-capable public authoring values store `IconGlyph` (glyph plus font fallback
stack) instead of a bare `char`. `PicusIcon` remains the bundled Lucide-backed
compatibility set; `FluentIcon` is the Fluent Design / WinUI symbol set. Use
`Into<IconGlyph>` builder parameters for public icon APIs so both sets remain
usable.

Priority built-ins (`UiButton`, `UiBadge`, `UiProgressBar`, `UiSwitch`, and
`UiCheckbox`) own their Picus-composed visual structure instead of exposing raw
compatibility widget appearance. `UiButton.disabled` renders a non-interactive
styled container that never emits click actions. `UiCheckbox.indeterminate` adds
a tri-state dash appearance; clicking an indeterminate checkbox transitions to
checked.

## 6. BSN UI Authoring and Migration

Picus supports Bevy Scene Notation as the preferred Rust-embedded description
language for static or mostly static ECS UI trees. `PicusPlugin` installs
Bevy's `ScenePlugin`, and `picus::prelude::*` re-exports `bsn!`,
`bsn_list!`, `Scene`, `SceneList`, and scene spawning extension traits.

Use BSN to describe entity hierarchy and component bundles. Do not treat `.bsn`
files as the default workflow; Picus currently prefers `bsn!`/`bsn_list!` in
Rust so UI descriptions can use local helper functions, typed constructors,
and normal Rust expressions.

Migration rules from old spawn code:

1. Replace a root spawn plus child `ChildOf(root)` calls with one
   `commands.spawn_scene(bsn! { ... Children [ ... ] })` block.
2. Components on the same entity are whitespace-separated in BSN. Sibling
   entities inside `Children [...]` are comma-separated.
3. Move `ChildOf(parent)` structure into nested `Children [...]`; do not keep
   explicit parent entity plumbing unless later systems need the entity ID.
4. Prefer field patch syntax for template-ready components, such as
   `UiButton { label: { "Save".to_string() } }`. Field patch syntax requires
   Bevy `FromTemplate`; for ordinary Picus UI authoring types this means
   `Default + Clone`.
5. All public Picus UI authoring components and their nested authoring values
   must remain `Default + Clone` unless they are documented runtime-only
   exceptions. This is a public BSN contract, not a convenience.
6. For components that do not or should not expose a default template, wrap the
   existing constructor with `template_value(...)`, for example
   `template_value(MyWidget::new(args))`, or insert the component from an ECS
   system. Type-erased runtime hooks such as `UiDialogCloseAction` are examples
   of this exception path.
7. Extract repeated fragments into Rust functions returning `impl Scene` or
   `impl SceneList`; keep dynamic data flow and event handling in ordinary ECS
   systems.
8. `UiComponentTemplate::expand` remains authoritative for Picus-owned template
   parts. BSN creates the logical ECS tree; Picus still expands logical controls
   and projects them into the retained Masonry runtime.
9. When adding a new Picus UI authoring component, derive or implement
   `Default + Clone` and update the
   `public_ui_authoring_types_are_bsn_template_ready` compile-time test. Use
   Bevy `FromTemplate` directly only when fields need spawn-time context such
   as named entity references or asset handle templates.
10. Components with `Entity` fields may use `Entity::PLACEHOLDER` as the
    default only to support BSN patching. Real scenes must patch those fields
    with a meaningful entity reference or let the relevant ECS system populate
    them.

## 7. Synthesis and Events

UI synthesis is driven by `UiProjectorRegistry` in `PostUpdate`. It gathers
`UiRoot` and `UiOverlayRoot` entities, projects ECS trees recursively, stores
`SynthesizedUiViews`, and rebuilds only dirty `MasonryRuntime` windows. Multiple
roots compose into a full-viewport top-left `zstack`, with overlays sorted last.

Projection invalidation is retained/incremental:

- Component projectors registered through `register_component` and
  `register_ui_component` automatically register their component type as a
  projection dependency. Raw projectors are conservatively treated as untracked
  and force synthesis; prefer component registration or an explicit
  `UiProjectionInvalidation` request for new code.
- `UiProjectionInvalidation` is the public escape hatch for projection inputs
  that do not appear as tracked components, such as external resources or
  bespoke runtime state. Use `request_all`, `request_window`, or `request_root`
  instead of writing a stable resource/component every frame to trigger rebuilds.
- `SynthesizedUiViews.dirty_windows` is the only handoff that should cause
  `rebuild_masonry_runtime` to replace retained root views. The paint pass then
  follows Masonry Core's own paint/animation invalidation.
- Systems feeding projection must avoid no-op mutable writes. Compare before
  assigning projection-visible resources/components such as window size, overlay
  geometry, scroll geometry, and style-derived state so Bevy change detection
  remains meaningful.

Interactive controls use the ECS event route:

- `ButtonView` and `ButtonWithChildView` emit pointer interaction events and
  typed actions through `UiEventQueue`.
- Public action helpers (`button`, `button_with_child`, `text_input`, `slider`,
  `switch`, and `checkbox`) are imported from `picus::components`,
  `picus::projection`, or `picus::prelude`; do not expose a public
  `picus::views` compatibility module.
- `picus_core::retained_bridge` is an internal ECS-to-retained adapter layer.
  It may bind entities, retained widget messages, and `UiEventQueue`, but it is
  not an application API and must not be made public as a module.
- Raw retained widgets remain private implementation details. Projection
  internals that need low-level widgets import them directly from
  `picus_view::view`.
- Search, text input, slider, switch, and checkbox helpers map retained widget
  actions into `UiEventQueue`. Do not expose the old Xilem app-state callback
  model in Picus-facing view APIs.
- `UiSearch` owns its current `value`; retained edits update the component and
  emit `UiSearchChanged`.
- Widget actions not consumed by ancestor `on_action` handlers are emitted as
  `RenderRootSignal::Action` and captured per window in `WindowRuntime`. The
  `route_masonry_view_messages` PreUpdate system (run after input injection,
  before `handle_widget_actions`) dispatches each captured action to its source
  view's `View::message` handler via the `ViewCtx` widget map, so
  callback-based views (`text_input`, `UiSearch`, `slider`, `switch`,
  `checkbox`) fire their `on_changed`/`on_enter` callbacks into `UiEventQueue`
  in the same frame. Button widgets push to `UiEventQueue` directly from
  `on_pointer_event` and do not rely on this routing path.
- `UiEventQueue` stores type-erased actions and supports typed non-destructive
  drains through `drain_actions::<T>()`.
- `UiPointerHitEvent` is the hit-tested source event; `UiPointerEvent` bubbles
  through ancestors until `StopUiPointerPropagation`.
- `OverlayPointerRoutingState` suppresses consumed overlay click paths so trigger
  controls do not remain pressed.

## 8. Styling Contract

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
- Entities with no matched rules and no inline style resolve to an otherwise
  empty `ResolvedStyle`, except for a configured stylesheet default
  `font_family`; projectors must not inject visible no-theme fallback colors,
  borders, padding, or shadows.
- Picus' Masonry runtime and Picus-owned text views make retained text defaults
  transparent so missing theme data does not fall through to backend static
  black text.
- Active stylesheet RON may declare `default_variant: "<name>"`; loading it
  applies that registered variant only when no active variant has already been
  selected.
- Stylesheet RON may declare a top-level `font_family` style value, usually
  `(Var: "font-family-base")`, as the default font stack for resolved styles.
  `FontFamily(...)` tokens and rule-level `font_family` fields accept either a
  CSS `font-family` string or a string list; CSS platform aliases such as
  `-apple-system` and `BlinkMacSystemFont` normalize to native `system-ui`.
  Use the map-style `(Var: "...")` spelling for font-family token references so
  one-item literal font stacks remain unambiguous.
- `ComputedStyle.font_family` carries resolved font-family data for projectors.
- Color transitions use `bevy_tween`; projectors read resolved plus animated style
  through `resolve_style`.

Built-in Fluent theming is a multi-variant bundle at
`crates/picus_core/src/theme/fluent_theme.ron` with `dark`, `light`, and
`high-contrast` variants. `PicusPlugin` registers these variants but does not
select one automatically; applications must explicitly call
`set_active_style_variant_by_name(...)` or load a stylesheet/theme that declares
`default_variant`.
The bundle provides default interactive `:hover`, `:pressed`, and focused-state
rules for Fluent-like built-ins and shared menu/list/table/navigation item classes,
plus NavigationView sidebar/content container classes.
Picus-only helpers that do not correspond to Fluent UI components, such as
`UiGroupBox`, must not receive default box styling from this built-in Fluent
bundle; examples or applications that want a visible group box provide their own
class or inline style.

Projectors should resolve style through the styling helpers, then apply it with the
widget, label, or text-input style helpers. Use
`resolve_style_for_entity_classes(...)` for pseudo-state-sensitive class styling.

## 9. Scroll Views and Overlays

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
panels, popovers, and related floating surfaces. `spawn_manual_overlay_at` places a
floating panel at an explicit `(x, y)` pixel coordinate, bypassing anchor-based
placement.

Overlay invariants:

- Overlay projectors render transparent content until
  `OverlayComputedPosition.is_positioned`.
- Outside-click dismissal checks the top overlay's hit path and bound widget IDs,
  with rectangle fallback.
- Dismissed dialogs emit their typed close hook through `UiEventQueue` before
  despawn when such a hook exists.
- Overlay entities reparent under `UiOverlayRoot` to avoid normal layout clipping.
- `UiToast` uses configurable placement and defaults to bottom-end behavior.

## 9.1. Markdown and Streaming Text

`UiMarkdown` renders a Markdown source string as a vertical stack of styled blocks
(headings, paragraphs, lists, block quotes, fenced code blocks, thematic breaks).
Inline emphasis (bold/italic/code/strikethrough/links) is flattened into per-run
styled labels because picus labels carry one style per label; mixed-emphasis
paragraphs lay out consecutive same-style runs in a wrapping flex row.

- Parsing uses `pulldown-cmark` with CommonMark + GFM tables/strikethrough/task lists.
- Fenced code blocks are syntax-highlighted with `syntect` (base16-ocean.dark theme)
  when a language fence matches a loaded grammar; otherwise plain monospace text.
- The highlight state (`SyntaxSet`/`Theme`) is lazily initialized once and reused.

`UiStreamingMarkdown` is the append-only streaming variant optimized for LLM output:

- Tokens are appended via `append`/`append_str` into an in-progress tail.
- `flush_completed` promotes the tail into a cached completed prefix.
- `finish` flushes any remaining tail and blocks further appends.
- `StreamingMarkdownParseCache` (`Update` system `update_streaming_markdown_cache`)
  caches parsed completed-prefix blocks keyed by entity + completed-source hash, so
  only the in-progress tail is re-parsed each frame.
- `evict_streaming_markdown_cache` removes cache entries for despawned entities.

## 10. Assets, Fonts, Icons, and I18n

`picus_core::icons` provides `IconGlyph`, bundled Lucide `PicusIcon` glyphs, and
Fluent Design / WinUI `FluentIcon` glyphs. Lucide glyphs use the bundled
`lucide` font bytes. Fluent glyphs use the font fallback stack
`Segoe Fluent Icons`, `Segoe MDL2 Assets`, `FabricMDL2Icons`, then
`Segoe UI Symbol`; examples that want Fluent Design icons should pass
`FluentIcon`/`IconGlyph` instead of styling a raw label character.

`XilemFontBridge` is the legacy-named font bridge that registers Bevy font assets
with Masonry Core. Fonts can come from the asset server, direct bytes, or direct
paths through `AppPicusExt`. The bridge deduplicates registered bytes, keeps a
registered-font history for future windows, and drains pending font bytes only
after an attached runtime exists.

`AppI18n` is the synchronous i18n registry. `LocalizeText` resolves through the
active bundle and falls back to the key or explicit fallback text.

## 11. Surface

`picus_surface` owns wgpu instance/device/queue state, surface configuration,
DPI-aware scene rendering, and swapchain presentation. It prefers opaque swapchain
alpha for externally owned Bevy windows, keeps the Windows AMD premultiplied-alpha
compatibility blit path as a fallback, and never blocks the Bevy thread waiting for
GPU completion after `present()`. It attaches through raw window handles and tracks
physical size, logical size, and scale factor.

## 12. Plugin and App Helpers

`PicusPlugin` installs the framework resources, built-in message types, schedule
systems, Bevy `ScenePlugin`, `DefaultTweenPlugins`, embedded Fluent variants, and core projectors.
`PicusBuiltinsPlugin` registers built-in UI components.

`run_app()` and `run_app_with_window_options()` bootstrap desktop apps with Bevy
window/input/accessibility/winit plugins and then call `App::run()`. Unless an app
has already installed `WinitSettings`, these helpers use bounded reactive updates
(120 Hz focused, 30 Hz unfocused) instead of Bevy's game-style continuous loop.

Use two UI composition layers:

- Rust view helpers for reusable local fragments.
- ECS components registered through `register_ui_component::<T>()` for reusable UI
  regions.

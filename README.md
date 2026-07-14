# picus

A Bevy-first UI framework that connects ECS state management with a retained Masonry Core runtime.

---

## What is picus?

**picus** is a workspace for building desktop user interfaces with Rust. It combines Bevy's ECS architecture with a retained Masonry Core widget tree model, giving you:

- Declarative UI defined through ECS components
- Explicit, typed event handling
- A powerful styling system with CSS-like cascades
- Built-in internationalization support
- Cross-platform window management

The workspace currently contains these crates:

- **picus** — the public application-facing facade (this is the crate you depend on)
- **picus_core** — implementation crate for projection, styling, overlays, runtime integration, and built-ins
- **picus_widget** — Picus-owned retained widget/property backend
- **picus_view** — Picus-owned Xilem-compatible retained view adapter
- **picus_surface** — Vello rendering bridge for window surfaces

This README covers the `picus` crate, which provides the complete UI framework experience through grouped public modules such as `picus::app`, `picus::components`, `picus::projection`, `picus::styling`, and `picus::overlay`. The companion crates provide the retained runtime, rendering, and platform integration.

---

## Installation

Add `picus` to your `Cargo.toml`:

```toml
[dependencies]
picus = "0.1"
```

If you're working with this workspace directly, use path dependencies from the repository root.

---

## Quick start

Here's a minimal counter app that demonstrates the core pattern:

```rust,no_run
use std::sync::Arc;

use picus::{
    AppPicusExt, PicusPlugin, ProjectionCtx, UiComponentTemplate, UiEventQueue, UiRoot,
    UiView,
    bevy_app::{App, PreUpdate, Startup},
    bevy_ecs::prelude::*,
    button, run_app_with_window_options,
    xilem::winit::{dpi::LogicalSize, error::EventLoopError},
};

#[derive(Component, Debug, Clone, Copy)]
struct CounterRoot;

#[derive(Resource, Debug, Default)]
struct Counter(i32);

#[derive(Debug, Clone, Copy)]
enum CounterEvent {
    Increment,
}

impl UiComponentTemplate for CounterRoot {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        Arc::new(button(ctx.entity, CounterEvent::Increment, "Increment"))
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((UiRoot, CounterRoot));
}

fn drain_events(world: &mut World) {
    let events = world
        .resource::<UiEventQueue>()
        .drain_actions::<CounterEvent>();

    if events.is_empty() {
        return;
    }

    let mut counter = world.resource_mut::<Counter>();
    for _ in events {
        counter.0 += 1;
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .insert_resource(Counter::default())
        .register_ui_component::<CounterRoot>()
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, drain_events);
    app
}

fn main() -> Result<(), EventLoopError> {
    run_app_with_window_options(build_app(), "Counter", |options| {
        options.with_initial_inner_size(LogicalSize::new(360.0, 220.0))
    })
}
```

The pattern is straightforward:

1. Define a component type that implements `UiComponentTemplate`
2. In `project()`, return a Picus view built from the entity
3. Spawn the component with `UiRoot` to attach it to the UI tree
4. Handle typed events from `UiEventQueue` in your systems
5. Run with `run_app_with_window_options` or `run_app`

---

## BSN UI description

Picus supports Bevy Scene Notation as a Rust-embedded UI description language.
`PicusPlugin` installs Bevy's `ScenePlugin`, and `picus::prelude::*`
re-exports `bsn!`, `bsn_list!`, `Scene`, `SceneList`, and the scene spawning
extension traits.

Use BSN when the shape of a UI tree is mostly static and you want to avoid
manual `commands.spawn((..., ChildOf(parent)))` wiring:

```rust,no_run
use picus::{
    bevy_ecs::prelude::*,
    prelude::*,
};

fn setup(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        UiRoot
        UiFlexColumn
        StyleClass(vec!["counter.root".to_string()])
        Children [
            UiThemePicker,
            UiLabel {
                text: { "Counter".to_string() },
            },
            (
                UiButton {
                    label: { "Increment".to_string() },
                }
                StyleClass(vec!["counter.primary".to_string()])
            ),
        ]
    });
}
```

The spawned entities are ordinary Picus ECS components. `UiComponentTemplate`
expansion, style resolution, event routing, synthesis, and retained Masonry
projection all run through the same pipeline as hand-written spawns. Picus treats
BSN as an in-code UI DSL; external `.bsn` files are not the recommended workflow.

BSN field-patch syntax, such as `UiButton { label: { "Save".to_string() } }`,
requires the patched type to be template-ready. Picus maintains its public UI
authoring components and their nested authoring values as `Default + Clone`, which
uses Bevy's blanket `FromTemplate` implementation. For application components that
you want to patch in `bsn!`, derive both:

```rust
#[derive(Component, Debug, Clone, Default)]
struct LoginPanel {
    title: String,
}
```

If a component intentionally carries runtime-only or type-erased behavior, do not
make callers guess. Use `template_value(MyComponent::new(...))` or spawn that
component from an ECS system. Picus documents this as the exception path for
event-hook components such as `UiDialogCloseAction`. Components with `Entity`
fields may use `Entity::PLACEHOLDER` only as a patching default; replace it with a
real entity reference when the value matters at runtime.

---

## Features

- **Bevy-native scheduling** — runs entirely within Bevy's update loop, no separate event loop
- **ECS-driven projection** — map components to widget views via `UiComponentTemplate`
- **BSN authoring** — describe static Picus UI trees with Rust-embedded Bevy Scene Notation
- **Typed action queue** — `UiEventQueue` provides type-safe event handling without closures
- **Explicit rendering pass** — Vello paint in `Last` stage, no Bevy render graph needed
- **Built-in components** — buttons, checkboxes, sliders, text inputs, dialogs, scroll views, and more
- **Styling engine** — CSS-like cascade with class selectors, inline overrides, and smooth transitions
- **Internationalization** — synchronous `AppI18n` with `LocalizeText` component
- **Overlay system** — dialogs, tooltips, dropdowns, toasts with automatic placement
- **Helper utilities** — `run_app()` auto-configures window plugins for desktop apps

---

## Workspace crates

### picus (public facade)

The main application-facing crate. It provides grouped modules for clearer imports:

- `picus::app` for plugins, runners, and Bevy re-exports
- `picus::components` for ECS authoring components and common action helpers
- `picus::projection` for low-level custom projector helpers
- `picus::styling` for style resolution and theme APIs
- `picus::events` for `UiEventQueue`, typed events, and widget actions
- `picus::overlay`, `picus::runtime`, `picus::i18n`, and `picus::scene` for focused subsystems

The root also re-exports the established `picus_core` API during migration, but new code should prefer the grouped modules or `picus::prelude::*`.

### picus_core

The implementation crate. It provides:

- The `PicusPlugin` that wires all core systems
- UI component library and registration API
- Styling system with selector-based rules
- Overlay and modal management
- Font and i18n bridges
- Run helpers for desktop applications

Most applications should depend on `picus` instead of `picus_core`.

### picus_surface

A low-level bridge that attaches a Vello renderer to an external Bevy window. Picus uses this internally for the `Last` paint pass. You typically won't interact with this crate directly unless you're customizing the rendering pipeline.

### picus_widget and picus_view

`picus_widget` is the Picus-owned retained backend crate. It owns widgets, properties, and layers on top of `masonry_core`. Widgets are lookless: production colours come from stylesheet RON via `picus_core`. Test harness skins live in `picus_theme_test`.

`picus_view` is the Picus-owned Xilem-compatible view adapter. It builds on `picus_widget` and `xilem_core` without depending on upstream `masonry` or upstream `xilem`.

---

## Examples

The workspace includes several example applications:

| App | Cargo package | Description |
|-----|---------------|-------------|
| `gallery` | `example_gallery` | Component gallery with Picus controls |
| `chess_game` | `example_chess_game` | Full chess UI with embedded engine |
| `async_downloader` | `example_async_downloader` | Async operations with progress UI |
| `calculator` | `example_calculator` | Standard calculator interface |
| `timer` | `example_timer` | Countdown timer with start/stop controls |
| `todo_list` | `example_todo_list` | Task management with add/remove |
| `game_2048` | `example_game_2048` | Classic 2048 game implementation |
| `overlay_hit_routing` | `example_overlay_hit_routing` | Overlay interaction patterns |

Run any example from the repository root:

```bash
cargo run -p example_gallery
```

---

## Styling system

Picus includes a complete styling pipeline inspired by CSS:

- Define rules in a `StyleSheet` resource (loaded from RON files or set directly)
- Attach classes to entities with `StyleClass`
- Resolve styles in projectors using helper functions
- Support for hover/pressed states and smooth color transitions

See [AGENTS.md](./AGENTS.md#8-styling-contract) for the full guide on selectors, cascade rules, and transition configuration.

---

## API conventions

Application code depends on `picus`, not `picus_core`. Prefer grouped imports when you only need part of the framework:

```rust
use picus::{
    app::{AppPicusExt, PicusPlugin, run_app},
    components::{UiComponentTemplate, UiRoot, UiView, button},
    events::UiEventQueue,
    runtime::ProjectionCtx,
};
```

Helper views such as `button`, `checkbox`, `slider`, `switch`, and `text_input` are Picus-native helpers that emit typed actions into `UiEventQueue`. Import them from `picus::components` or `picus::prelude` with the rest of the authoring surface. Raw retained widgets are internal implementation details imported from `picus_view::view` by projectors when needed.

---

## Event handling model

The framework follows a clear pipeline each frame:

1. UI components emit typed actions into `UiEventQueue`
2. Your systems drain those actions in `PreUpdate`
3. You mutate ECS state/resources based on events
4. Picus synthesizes the widget tree in `PostUpdate`
5. The retained Masonry scene is painted and presented in `Last`

This keeps interaction handling explicit and fully ECS-compatible.

---

## License

Dual-licensed under MIT OR Apache-2.0.

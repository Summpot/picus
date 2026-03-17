# picus

A Bevy-first UI framework that connects ECS state management with a retained Xilem/Masonry runtime.

---

## What is picus?

**picus** is a workspace for building desktop user interfaces with Rust. It combines Bevy's ECS architecture with Xilem/Masonry's retained widget tree model, giving you:

- Declarative UI defined through ECS components
- Explicit, typed event handling
- A powerful styling system with CSS-like cascades
- Built-in internationalization support
- Cross-platform window management

The workspace currently contains three crates:

- **picus_core** — the main UI framework (this is the crate you depend on)
- **picus_surface** — Vello rendering bridge for window surfaces
- **picus_activation** — deep linking and single-instance support

This README covers the `picus_core` crate, which provides the complete UI framework experience. The companion crates handle specialized platform integration.

---

## Installation

Add `picus_core` to your `Cargo.toml`:

```toml
[dependencies]
picus_core = "0.1"
```

If you're working with this workspace directly, use path dependencies from the repository root.

---

## Quick start

Here's a minimal counter app that demonstrates the core pattern:

```rust,no_run
use std::sync::Arc;

use picus_core::{
    AppPicusExt, PicusPlugin, ProjectionCtx, UiComponentTemplate, UiEventQueue, UiRoot,
    UiView,
    bevy_app::{App, PreUpdate, Startup},
    bevy_ecs::prelude::*,
    run_app_with_window_options, text_button,
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
        Arc::new(text_button(ctx.entity, CounterEvent::Increment, "Increment"))
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
2. In `project()`, return a Xilem/Masonry view built from the entity
3. Spawn the component with `UiRoot` to attach it to the UI tree
4. Handle typed events from `UiEventQueue` in your systems
5. Run with `run_app_with_window_options` or `run_app`

---

## Features

- **Bevy-native scheduling** — runs entirely within Bevy's update loop, no separate event loop
- **ECS-driven projection** — map components to widget views via `UiComponentTemplate`
- **Typed action queue** — `UiEventQueue` provides type-safe event handling without closures
- **Explicit rendering pass** — Vello paint in `Last` stage, no Bevy render graph needed
- **Built-in components** — buttons, checkboxes, sliders, text inputs, dialogs, scroll views, and more
- **Styling engine** — CSS-like cascade with class selectors, inline overrides, and smooth transitions
- **Internationalization** — synchronous `AppI18n` with `LocalizeText` component
- **Overlay system** — dialogs, tooltips, dropdowns, toasts with automatic placement
- **Helper utilities** — `run_app()` auto-configures window plugins for desktop apps

---

## Workspace crates

### picus_core (primary)

The main framework crate. It provides:

- The `PicusPlugin` that wires all core systems
- UI component library and registration API
- Styling system with selector-based rules
- Overlay and modal management
- Font and i18n bridges
- Run helpers for desktop applications

**This is the crate most users depend on.**

### picus_surface

A low-level bridge that attaches a Vello renderer to an external Bevy window. `picus_core` uses this internally for the `Last` paint pass. You typically won't interact with this crate directly unless you're customizing the rendering pipeline.

### picus_activation

Handles deep linking, single-instance enforcement, and custom URI protocol registration. Useful for applications that need to respond to custom URL schemes or ensure only one instance runs at a time. Like `picus_surface`, this is an implementation detail for most users.

---

## Examples

The workspace includes several example applications:

| App | Cargo package | Description |
|-----|---------------|-------------|
| `ui_showcase` | `example_ui_showcase` | Component gallery with theming and localization |
| `chess_game` | `example_chess_game` | Full chess UI with embedded engine |
| `async_downloader` | `example_async_downloader` | Async operations with progress UI |
| `calculator` | `example_calculator` | Standard calculator interface |
| `timer` | `example_timer` | Countdown timer with start/stop controls |
| `todo_list` | `example_todo_list` | Task management with add/remove |
| `game_2048` | `example_game_2048` | Classic 2048 game implementation |
| `overlay_hit_routing` | `example_overlay_hit_routing` | Overlay interaction patterns |
| `pixiv_client` | `example_pixiv_client` | Image browsing application |

Run any example from the repository root:

```bash
cargo run -p example_ui_showcase
```

---

## Styling system

`picus_core` includes a complete styling pipeline inspired by CSS:

- Define rules in a `StyleSheet` resource (loaded from RON files or set directly)
- Attach classes to entities with `StyleClass`
- Resolve styles in projectors using helper functions
- Support for hover/pressed states and smooth color transitions

See [STYLING.md](./STYLING.md) for the full guide on selectors, cascade rules, and transition configuration.

---

## API conventions

The crate exports two families of UI components:

- **ECS adapters** (recommended) — `button`, `checkbox`, `slider`, `switch`, `text_button`, `text_input` — these emit typed actions directly into `UiEventQueue`
- **Raw Xilem widgets** — `xilem_button`, `xilem_checkbox`, etc. — for cases where you need the original widget without ECS integration

Legacy `ecs_*` names remain for backward compatibility.

---

## Event handling model

The framework follows a clear pipeline each frame:

1. UI components emit typed actions into `UiEventQueue`
2. Your systems drain those actions in `PreUpdate`
3. You mutate ECS state/resources based on events
4. `picus_core` synthesizes the widget tree in `PostUpdate`
5. The retained Masonry scene is painted and presented in `Last`

This keeps interaction handling explicit and fully ECS-compatible.

---

## License

Dual-licensed under MIT OR Apache-2.0.

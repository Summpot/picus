# Events and messages

## Architecture

```text
Retained widgets / projection callbacks
        │  push type-erased payload
        ▼
InternalUiEventQueue  (app-owned, not public)
        │  sole consumer: dispatch_ui_actions
        ▼
UiActionRegistry (TypeId → handlers)
        │
        ├─ built-in handlers (widget/overlay mutations)
        └─ application handlers → Messages<UiAction<T>>
                                      │
                                      ▼
                         MessageReader<UiAction<T>>
```

## Application API

| Type / API | Role |
|------------|------|
| `UiAction<T> { source, action }` | Bevy `Message` apps read |
| `add_ui_action::<T>()` | Registers messages + `UiActionSender<T>` + dispatcher |
| `UiActionSender<T>` | Cloneable write handle for deferred emits (async tasks, custom widgets) |
| `UiEmit` | Non-generic ECS component binding a button to `T` |
| `ProjectionCtx::button` / `button_with_child` | Action-aware retained buttons for custom projection |
| `ProjectionCtx::action_sender` | Capture a sender while projecting |
| `PicusUiSet` | PreUpdate chain: Input → RetainedRouting → DispatchActions |

Built-in widget/overlay interactions (`WidgetUiAction`, `OverlayUiAction`) are applied
by the **same** PreUpdate dispatcher via registry handlers; applications never drain
those types. High-level `*Changed` payloads are already registered as `UiAction`
messages by `PicusPlugin`.

There is **no** public global `emit_ui_action`. Prefer capturing `UiActionSender<T>` from
`ProjectionCtx` or `Res<UiActionSender<T>>` in systems.

## Scheduling

- Input-driven actions become `UiAction` messages **before** ordinary `Update`
  systems in the same frame.
- Emissions from `Update` (or later) via `UiActionSender` are **next-frame** visible.
- Unregistered payloads: panic in debug/test; log-once and drop in release.

## What is not a Message

Pointer hits, hover/press interaction markers, and other high-frequency internal
events stay on internal paths and are not automatically elevated to application
messages.

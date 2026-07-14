# picus_core — agent rules

Implementation crate for Picus. Applications must depend on the **`picus` facade**,
not this crate. See root [`AGENTS.md`](../../AGENTS.md) and
[`docs/architecture/crates.md`](../../docs/architecture/crates.md).

## Hard invariants

### Actions / queue

- The internal action queue is **app-owned** (`InternalUiEventQueue`).
- **Single consumer**: only `dispatch_ui_actions` drains the queue in production
  schedules. Built-in `WidgetUiAction` / `OverlayUiAction` mutate ECS via
  registry handlers (`apply_widget_ui_action` / `apply_overlay_ui_action`).
- Do not re-export queue/drain/`emit_ui_action` on the `picus` facade.
- Unregistered payloads: panic in debug/test; log-once and drop in release.
- FIFO: handler-enqueued actions process after already-queued entries; respect
  `UI_ACTION_DISPATCH_LIMIT`.

### Projection

- Resource dependencies must be registered (`register_projection_resource` or
  `UiComponentTemplate::register_projection_dependencies` / derive `resources`).
- Avoid no-op mutable writes on projection-visible state.

### Overlay / scroll

- Overlay projectors stay transparent until positioned.
- Outside-click dismissal checks the top overlay hit path / bound widget IDs.
- Nested wheel routing starts at the deepest hit target.

### Markdown / streaming

- Prefer incremental cache updates; do not wholesale rebuild parse caches on
  every append when streaming markdown content is unchanged in prefix.

### BSN authoring

- Public UI authoring components: `Default + Clone` unless documented
  runtime-only.

## When changing this crate

- Keep docs under `docs/guide/` and root AGENTS in sync for any public-contract
  change that surfaces through the facade.
- Prefer extending dispatcher handlers over new typed-drain systems.

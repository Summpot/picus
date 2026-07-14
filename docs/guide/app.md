# Application guide

## Recommended path

1. Create a Bevy `App` and add `PicusPlugin`.
2. **Explicitly** load a stylesheet (`load_style_sheet` / `load_style_sheet_ron`) and/or
   select a variant (`style_variant`). Picus never auto-picks dark/light.
3. Register business actions with `add_ui_action::<T>()`.
4. Implement `UiComponentTemplate` for custom regions; derive `UiComponent` and
   register them once with `register_ui_components!(app, ...)`.
5. Handle interactions with `MessageReader<UiAction<T>>` (not an internal queue).
6. Run with `app.run_picus(title, BevyWindowOptions::default()...)`.

```rust,ignore
#[derive(Clone, Debug)]
enum AppAction { Inc, Dec }

#[derive(Resource, Default)]
struct Count(i32);

#[derive(Component, Clone, Default, UiComponent)]
#[ui_component(resources(Count))]
struct CountLabel;

impl UiComponentTemplate for CountLabel {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let n = ctx.world.resource::<Count>().0;
        Arc::new(label(format!("{n}")))
    }
}

fn on_app_action(
    mut reader: MessageReader<UiAction<AppAction>>,
    mut count: ResMut<Count>,
) {
    for UiAction { action, .. } in reader.read() {
        match action {
            AppAction::Inc => count.0 += 1,
            AppAction::Dec => count.0 -= 1,
        }
    }
}

fn main() -> Result<(), EventLoopError> {
    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .load_style_sheet_ron(include_str!("../assets/themes/app.ron"))
        .add_ui_action::<AppAction>()
        .add_systems(Startup, setup)
        .add_systems(Update, on_app_action);
    register_ui_components!(app, CountLabel);
    app.run_picus("Counter", BevyWindowOptions::default())
}
```

## Theme contract

- **No theme / no variant** → no framework-provided visible fill or text colour.
- **Partial themes are valid**: missing rules stay transparent/empty; only structural
  RON/token errors fail loading.
- Prefer application RON that sets `default_variant`, or call `style_variant("dark")`.

## Buttons and `UiEmit`

- BSN: attach `template_value(UiEmit::new(AppAction::Inc))` on a `UiButton` entity.
- Without `UiEmit`, enabled buttons emit `BuiltinUiAction::Clicked`.
- Disabled buttons emit nothing.
- Custom projection: `ctx.button(action, label)` / `ctx.button_with_child(...)` /
  `ctx.action_sender::<T>()` (do not use free `button(entity, ...)` helpers).
- Deferred / async emits: clone `UiActionSender<T>` and call `send(source, action)`.
- Style helpers: `styled(view, &resolved)` / `ctx.styled(view)`; layout:
  `ctx.flex_col([...])` / `ctx.flex_row([...])`.
- Inline overrides: `InlineStyle::new().padding(8.0).bg(color)`.

## When not to split a Component

Prefer a single container component that maps children or builds a small view tree
when the piece is not reused, has no independent style type, and does not need its
own projection resources. Split into a `UiComponent` when the subtree is reused,
has distinct styles/classes, or registers its own resource dependencies.

## Fine-grained components vs container map (todo pattern)

Two common authoring styles:

| Style | When | Shape |
|-------|------|--------|
| **Fine-grained** | Each row has independent interaction, style classes, or resources | Spawn one ECS entity per item (`TodoItem` component + `UiEmit` / child buttons) |
| **Container map** | List is pure projection of a resource; items are not independently styled | One `TodoList` component reads `Res<Todos>` and builds labels/buttons in `project` via `ctx.flex_col` |

Guidelines:

- Prefer **container map** for short-lived or purely derived lists (less registration noise).
- Prefer **fine-grained entities** when items need hit testing identity, per-row
  `UiEmit`, focus, or stylesheet type/class selectors.
- See `examples/todo_list` for a hybrid: list state in a resource, rows as projected
  structure with typed actions.

Helpers that reduce dual writing without new components:

- `ctx.flex_col` / `ctx.flex_row`
- `ctx.button` / `ctx.button_with_child` / `ctx.styled`
- `classes!("…")` + RON rules instead of one-off styled wrappers

Composite layout components (e.g. form rows) remain optional convenience; they are
not required for the default app path.

## When to use exclusive systems

Prefer ordinary `MessageReader` systems. For world-exclusive mutation, use
`picus::drain_ui_actions::<T>(world)` which reads only newly arrived messages.

## Related guides

| Topic | Doc |
|-------|-----|
| Actions / schedule | [events-messages.md](events-messages.md) |
| Macros | [macros.md](macros.md) |
| Themes | [styling-themes.md](styling-themes.md) |
| i18n / fonts | [i18n-fonts-icons.md](i18n-fonts-icons.md) |
| Multi-window | [multi-window.md](multi-window.md) |
| Overlays / scroll | [overlays-scroll.md](overlays-scroll.md) |

Rustdoc on the `picus` crate points at these guide names; long tutorials live only here.

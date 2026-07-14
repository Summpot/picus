# Multi-window

## Defaults

- Each window has its own `WindowRuntime` / Masonry tree.
- The **primary** window auto-attaches; additional windows follow the same
  plugin lifecycle when spawned with Bevy window components.
- Action sinks are **app-owned**: all windows of one `App` share one internal
  queue; separate `App` instances never share sinks.

## Pointer and paint

- Pointer coordinates come from the **event window’s** physical cursor position.
- Paint/present errors are captured per surface; only successful present marks
  painted (see `picus_surface` nested AGENTS).

## Theme and fonts

- Stylesheets and variants are app-level resources (same theme across windows
  unless you introduce your own per-window state).
- Font registration broadcasts to every attached window and replays on attach.

## Testing

- Prefer multi-window isolation tests in `picus_core` when changing runtime
  attachment; do not assume a single global render root.

## Related

- Runtime overview: [../architecture/overview.md](../architecture/overview.md)
- Actions: [events-messages.md](events-messages.md)

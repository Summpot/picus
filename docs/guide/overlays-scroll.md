# Overlays and scroll

## Overlays

- Overlay projectors stay **transparent until positioned**.
- Outside-click dismissal uses the **top overlay hit path** and bound widget IDs.
- Built-in overlay interactions use internal `OverlayUiAction` payloads applied
  by the PreUpdate dispatcher (not a separate public drain API).

## Scroll

- Nested wheel routing starts at the **deepest** hit target.
- ECS `UiScrollView` state stays authoritative; portal/retained views reflect
  `scroll_offset`.

## Application path

Prefer ordinary `MessageReader<UiAction<T>>` for business reactions to overlay
results (combo selection, dialog dismiss, etc.). Do not depend on internal
queue types.

## Related

- Events: [events-messages.md](events-messages.md)
- Core hard rules: [`crates/picus_core/AGENTS.md`](../../crates/picus_core/AGENTS.md)

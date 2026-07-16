# Paint isolation (anim entries)

Continuous high-frequency visual animation must **not** dirty the full-window
base present path on every tick. Widgets declare a **painter slot** via
`PaintIsolation` (in `picus_widget`; not a global top-most layer).

Architecture detail: [architecture/runtime.md](../architecture/runtime.md).
Plan: [plans/frame-pipeline.md](../plans/frame-pipeline.md) (Phase 3 delivered).
Facade note: [reference/public-modules.md](../reference/public-modules.md)
(`PaintIsolation` is **not** on `picus::prelude`).

## API

```text
PaintIsolation::Inline     // default — base / cached scene segment
PaintIsolation::AnimEntry  // External painter-order slot → Picus anim host
                           // (when host discovers the widget)
```

| Concept | Meaning |
|---------|---------|
| **Painter slot** | Where pixels land in Masonry painter order (inline segment vs External placeholder filled by host) |
| **Not** | Always-on-top Z boost, gallery hardcode, or “whole window is anim” |

`AnimEntry` maps to Masonry `PaintLayerMode::External` **every paint** (mode is
not sticky) via `PaintIsolation::apply`. That only reserves the Masonry slot.

## Promotion vs discovery (read carefully)

| Step | What happens | Openness today |
|------|----------------|----------------|
| **Apply** | Widget paint calls `isolation.apply(ctx)` → External placeholder | Any widget can call `apply` |
| **Discover** | Host reads isolation for a live widget id | **Closed allowlist**: `Spinner`, indeterminate `ProgressBar` via `paint_isolation()` |
| **Promote** | If discovered isolation is `AnimEntry` → Anim entry + host scene | Isolation-**keyed** once discovered |
| **Paint host scene** | Host builds window-space scene | Type-dispatched (arms / segment) |

So: promotion is isolation-keyed, but **isolation discovery is still type-dispatched**.
Wording “isolation-driven” means the enum decides promote vs not after discovery —
not that any third-party type becomes self-describing without a host allowlist entry.

## When `AnimEntry` is required

Use **`AnimEntry`** when the control’s **visual** changes continuously at
display-rate (or similar), for example:

- Indefinite loading spinners
- Indeterminate progress “candy bar” motion
- Any future **host-known** widget that would otherwise `request_paint` every
  frame into the base scene and force full-window rewrite + encode

Stay on **`Inline`** when:

- Paint is event/state driven (clicks, theme, layout, discrete progress value)
- Animation is short, one-shot, or already covered by property transitions that
  do not need a permanent 60 Hz present loop on the base path

**Hard rule (AGENTS):** continuous ~60 Hz visual animation must not default to
dirtying the full-window base present path.

## Built-in defaults

| Control | Isolation |
|---------|-----------|
| `UiSpinner` / retained `Spinner` | Always `AnimEntry` |
| `UiProgressBar` indeterminate (`progress == None`) | `AnimEntry` |
| `UiProgressBar` determinate (`Some`) | `Inline` |
| Other stock widgets | `Inline` |

No gallery or entity hardcodes. Host **scene paint** for Spinner / ProgressBar
remains type-dispatched (arms / indeterminate segment).

## Authoring notes

- Application code normally uses `UiSpinner` / `UiProgressBar` through the
  `picus` facade; isolation is already correct for those.
- Determinate progress must **not** keep a permanent anim tick; switching
  indeterminate → determinate drops the host slot and returns to Inline.

## Known limitation: custom `AnimEntry` widgets

A custom retained widget that **only** calls `PaintIsolation::AnimEntry.apply(ctx)`:

1. Gets an External placeholder in the visual plan (good).
2. Is **not** discovered by the host allowlist → stays a **transparent External**
   forever (no host scene, no G2 anim path). Never an empty Anim entry.

**Required today for real anim isolation:** framework-known type with both
`paint_isolation()` discovery and a host scene painter (stock: Spinner /
indeterminate ProgressBar).

**Path forward (not P3):** open discovery without inventory/linkme, e.g.:

- explicit TypeId-keyed host painter registration on app/runtime setup, or
- a sealed/public trait queried through a registration table the host owns

Until that lands, third-party continuous anim controls should use stock
`UiSpinner` / indeterminate `UiProgressBar`, or land in-tree with host
allowlist + painter updates (same place as Spinner/ProgressBar).

## Selective path (G2)

When dirty is only anim paint and the plan already has Anim entries, the runtime
can skip full-tree redraw and encode **only** anim entries (base cached segments
stay clean). Isolation-keyed promotion after discovery is what makes that path
possible for Spinner / indeterminate ProgressBar.

Anim entries use a tight physical-pixel target derived from the External slot's
window-space bounds. `picus_surface` composites that texture back into its exact
painter-order viewport. If only Anim textures changed, the persistent ordered
intermediate is rebuilt only inside the union of those target rectangles; the
final swapchain blit remains full-window. The compatibility full-window target
remains available internally. A full content redraw compares retained Scene
recordings per static segment, so a sidebar hover does not invalidate unrelated
cached segments.

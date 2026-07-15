# Paint isolation (anim entries)

Continuous high-frequency visual animation must **not** dirty the full-window
base present path on every tick. Widgets declare a **painter slot** via
`PaintIsolation` (in `picus_widget`; not a global top-most layer).

Architecture detail: [architecture/runtime.md](../architecture/runtime.md).
Plan: [plans/frame-pipeline.md](../plans/frame-pipeline.md) Phase 3.

## API

```text
PaintIsolation::Inline     // default — base / cached scene segment
PaintIsolation::AnimEntry  // External painter-order slot → Picus anim host
```

| Concept | Meaning |
|---------|---------|
| **Painter slot** | Where pixels land in Masonry painter order (inline segment vs External placeholder filled by host) |
| **Not** | Always-on-top Z boost, gallery hardcode, or “whole window is anim” |

`AnimEntry` maps to Masonry `PaintLayerMode::External` **every paint** (mode is
not sticky). Picus promotes that External slot to a stable anim compositor
entry (`AnimLayerId` / `LayerId`) when the live widget reports `AnimEntry`.

## When `AnimEntry` is required

Use **`AnimEntry`** when the control’s **visual** changes continuously at
display-rate (or similar), for example:

- Indefinite loading spinners
- Indeterminate progress “candy bar” motion
- Any future widget that would otherwise `request_paint` every frame into the
  base scene and force full-window rewrite + encode

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

No gallery or entity hardcodes: isolation is declared on the widget; host
registration reads it. Host **scene paint** for Spinner / ProgressBar remains
type-dispatched (arms / indeterminate segment).

## Authoring notes

- Application code normally uses `UiSpinner` / `UiProgressBar` through the
  `picus` facade; isolation is already correct for those.
- Custom retained widgets that need continuous isolation must:
  1. Report `PaintIsolation::AnimEntry` (and apply it every paint).
  2. Be known to the Picus anim host painter path (today: framework-registered
     types only — bare External without host content stays a transparent
     placeholder, never an empty Anim entry).
- Determinate progress must **not** keep a permanent anim tick; switching
  indeterminate → determinate drops the host slot and returns to Inline.

## Selective path (G2)

When dirty is only anim paint and the plan already has Anim entries, the runtime
can skip full-tree redraw and encode **only** anim entries (base cached segments
stay clean). Isolation + host promotion is what makes that path possible for
Spinner / indeterminate ProgressBar.

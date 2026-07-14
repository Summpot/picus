# Architecture overview

Picus is a **Bevy-first** UI framework: Bevy owns scheduling, windows, and input;
Masonry Core runs as a retained runtime resource driven by Bevy systems.

```text
Application (depends on `picus` facade only)
    │
    ▼
picus  ──facade──►  picus_core  ──►  picus_view / picus_widget::masonry_core
                         │              └── xilem::core / xilem::winit
                         └──► picus_surface ──► picus_imaging (desktop Vello/wgpu)
```


## Frame stages (summary)

| Stage | Work |
|-------|------|
| PreUpdate | Input injection, retained message routing, **action dispatch** (`PicusUiSet`) |
| Update | Overlay lifecycle, style/theme, transitions |
| PostUpdate | UI synthesis, retained rebuild, IME sync |
| Last | Vello paint/present |

## Key contracts

- Projection invalidation tracks components/resources registered as dependencies.
- Application business actions use Bevy `Message` (`UiAction<T>`), not a public queue.
- Missing style data draws nothing visible; no default brand palette in widgets.

See [crates.md](crates.md) and the root `AGENTS.md` for enforceable rules.

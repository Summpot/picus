# Crate boundaries

| Crate | Role |
|-------|------|
| `picus` | **Only** application dependency. Grouped modules + macros. |
| `picus_macros` | Proc-macros (`UiComponent`). Re-exported by `picus`. |
| `picus_core` | Implementation: projection, styling, overlays, plugin, runner. |
| `picus_widget` | Lookless retained widgets/properties; owns `masonry_core` re-export module. |
| `picus_view` | Xilem-compatible view adapter on `picus_widget` (`xilem::core`). |
| `picus_surface` | wgpu/Vello surface for Bevy windows. |
| `picus_imaging` | Desktop imaging adapters (paint → WGPU texture). No wasm. |
| `picus_theme_test` | Test-only dark property sets; not for apps. |

## Upstream dependency

- **`xilem`** facade (git-pinned today; switch to crates.io when a release includes
  imaging/layout).
- Surfaces used by Picus:
  - `picus_widget::masonry_core` ← `xilem::masonry` (+ core properties subset, `debug_panic`)
  - `xilem::core` (former `xilem_core`)
  - `xilem::winit` / `EventLoop*` / `WindowId` (former `masonry_winit` subset)
- Imaging: **`picus_imaging`** on crates.io `imaging*` (desktop only).

## Forbidden

- Depend on `picus_core` from application code (use `picus`).
- Ship production colour palettes from `picus_widget`.

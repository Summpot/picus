# Picus documentation

This directory is the human-readable source of truth for architecture, application
guides, and subsystem deep-dives. Hard process rules for agents live in
repository `AGENTS.md` files (root and nested where needed).

## Map

| Path | Topic |
|------|--------|
| [architecture/overview.md](architecture/overview.md) | Bevy + Masonry + projection overview |
| [architecture/crates.md](architecture/crates.md) | Crate boundaries |
| [architecture/runtime.md](architecture/runtime.md) | Per-window runtime, frame stages, layer model, fonts, presentation |
| [architecture/input-ime-hit.md](architecture/input-ime-hit.md) | Input coordinates, ordering, IME, and hit testing |
| [architecture/projection.md](architecture/projection.md) | Projector registration, dependencies, and invalidation |
| [guide/app.md](guide/app.md) | How to write a Picus app (`AppPicusExt`, theme, `UiAction`, BSN) |
| [guide/events-messages.md](guide/events-messages.md) | `UiAction`, `UiActionSender`, `UiEmit`, scheduling |
| [guide/macros.md](guide/macros.md) | `#[derive(UiComponent)]`, `#[ui_view]`, `register_ui_components!`, `classes!` |
| [guide/components-bsn.md](guide/components-bsn.md) | Authoring component contracts and BSN patterns |
| [guide/markdown-streaming.md](guide/markdown-streaming.md) | Incremental markdown projection and streaming tests |
| [guide/testing.md](guide/testing.md) | Headless action, projection, and workspace verification |
| [guide/styling-themes.md](guide/styling-themes.md) | No-theme contract, RON, variants, backdrop |
| [guide/overlays-scroll.md](guide/overlays-scroll.md) | Overlay hit path, scroll wheel routing |
| [guide/i18n-fonts-icons.md](guide/i18n-fonts-icons.md) | Localization, fonts, icons |
| [guide/multi-window.md](guide/multi-window.md) | Multi-window runtime and sinks |
| [guide/paint-isolation.md](guide/paint-isolation.md) | `PaintIsolation::{Inline, AnimEntry}` for continuous anim |
| [examples/index.md](examples/index.md) | What each example teaches |
| [plans/gallery-winui-parity.md](plans/gallery-winui-parity.md) | Gallery ↔ WinUI Gallery control fill-in plan |
| [contributing/codewhale-submodule.md](contributing/codewhale-submodule.md) | CodeWhale fork sync |
| [reference/public-modules.md](reference/public-modules.md) | Public facade modules and advanced boundary |
| [reference/style-tokens.md](reference/style-tokens.md) | Theme, token, and missing-rule contract |
| [perf/frame-pipeline-baseline.md](perf/frame-pipeline-baseline.md) | Frame pipeline baseline protocol, PresentMon/ETW, result tables |

**Rustdoc strategy**: crate/module docs on `picus` are one-liners plus a pointer to
the matching guide file above. Long tutorials and architecture narrative stay in
`docs/` only; API contracts on public types stay in rustdoc.

Start with [guide/app.md](guide/app.md) and the `timer` or `calculator` example.

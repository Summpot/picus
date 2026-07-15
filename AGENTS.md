# AGENTS.md

Enforceable process rules and cross-cutting contracts for humans and automated
agents. This file is **not** a tutorial. Architecture, walkthroughs, and API
guides live under [`docs/`](docs/README.md).

## 1. Role and scope

- Prefer short, executable rules here; put explanations in `docs/`.
- Nested `AGENTS.md` files may add subsystem-local hard rules without modifying
  third-party submodule contents.
- When a contract changes, update **both** this file (or the nested AGENTS) and
  the corresponding docs guide.

## 2. Dependency and crate boundaries

- Applications depend on the **`picus` facade only**, not `picus_core`.
- Upstream GUI stack is the **`xilem`** facade (currently git-pinned until a
  crates.io release includes the imaging/layout APIs Picus uses). Masonry Core
  surfaces are inlined as `picus_widget::masonry_core`; reactive core and winit
  come from `xilem::core` / `xilem::winit`. Paint adapters: `picus_imaging`
  (desktop only; no wasm).
- `picus_widget` is lookless: no production brand colour palettes; test skins
  belong in `picus_theme_test`.
- Macro expansions may only touch `picus::__macro_support` (doc-hidden).

See [`docs/architecture/crates.md`](docs/architecture/crates.md).

## 3. Default application path

Applications must:

1. Install `PicusPlugin` on a Bevy `App`.
2. Load theme RON and/or select a variant **explicitly** (`AppPicusExt`).
3. Register business payloads with `add_ui_action::<T>()`.
4. Derive `UiComponent` (or `#[ui_view]` for zero-state projected regions)
   and register custom components via one `register_ui_components!(app, ...)`
   list (not ad-hoc hidden APIs).
5. Consume interactions with `MessageReader<UiAction<T>>`; exclusive systems
   should receive app-owned pending state collected by a normal message reader.
6. Start with `AppPicusExt::run_picus` (not removed runners).

Guide: [`docs/guide/app.md`](docs/guide/app.md). Entry examples: `timer`,
`calculator`.

## 4. Must-follow contracts

### BSN / authoring

- Public UI authoring components and nested authoring values are
  `Default + Clone` unless documented as runtime-only (e.g. `UiEmit`, hooks).
- Prefer `bsn!` / `bsn_list!` for static trees; `UiComponentTemplate::expand`
  remains authoritative for Picus-owned template parts.

### Projection

- Projection dependencies (components/resources) must be registered (via
  `#[ui_component(resources(...))]` / derive metadata or advanced APIs).
- Avoid no-op mutable writes on projection-visible state so change detection
  stays meaningful.

### Actions / messages

- Applications **do not** drain or hold the internal action queue.
- Public surface: `UiAction<T>`, `UiActionSender<T>`, `UiEmit`,
  `add_ui_action`, `PicusUiSet`.
- PreUpdate order: `Input → RetainedRouting → DispatchActions`.
- Input-driven actions are visible to same-frame `Update` readers; Update-time
  sender emissions are next-frame.

See [`docs/guide/events-messages.md`](docs/guide/events-messages.md).

### Styling / theme

- **Missing theme or missing rules ⇒ no visible framework defaults** (not an error).
- Framework **never** auto-selects dark/light.
- Partial themes are legal; only structural RON/token errors fail.
- Production colours come from stylesheet RON, not widget defaults.

See [`docs/guide/styling-themes.md`](docs/guide/styling-themes.md).

### Runtime / input (cross-module)

- Per-window `MasonryRuntime` / `WindowRuntime`; primary window auto-attaches.
- Pointer coordinates from the event window’s physical cursor position.
- Click injection sends move before down/up; resize uses logical dimensions.
- Paint/present errors are captured; only successful `present()` marks painted.
- Font registration broadcasts to all windows and replays on attach.
- Continuous ~60Hz visual animation must not default to dirtying the full-window
  base present path — use `PaintIsolation::AnimEntry` (painter slot, not global
  top layer). See [`docs/guide/paint-isolation.md`](docs/guide/paint-isolation.md).

### Overlays / scroll

- Overlay projectors stay transparent until positioned.
- Outside-click dismissal checks the top overlay hit path / bound widget IDs.
- Nested wheel routing starts at the deepest hit target.

### picuscode / CodeWhale

- Integration example only; tests must not touch the user’s real `~/.codewhale/`.
- Full sync procedure: [`docs/contributing/codewhale-submodule.md`](docs/contributing/codewhale-submodule.md).
  Submodule-local hard steps may live under `thirdparty/` AGENTS without
  editing submodule files.

## 5. Forbidden

- Public `UiEventQueue` / typed drain / process-global app action API.
- Framework default dark theme on empty configuration.
- Root-level `pub use picus_core::*` dump on the facade.
- inventory / linkme for UI component registration.
- Closure-on-Component application APIs.
- Application code calling `picus::__macro_support` directly.

## 6. Documentation map

| Need | Where |
|------|--------|
| Doc index | [`docs/README.md`](docs/README.md) |
| App guide | [`docs/guide/app.md`](docs/guide/app.md) |
| Events | [`docs/guide/events-messages.md`](docs/guide/events-messages.md) |
| Macros | [`docs/guide/macros.md`](docs/guide/macros.md) |
| Themes | [`docs/guide/styling-themes.md`](docs/guide/styling-themes.md) |
| Overlays / scroll | [`docs/guide/overlays-scroll.md`](docs/guide/overlays-scroll.md) |
| i18n / fonts | [`docs/guide/i18n-fonts-icons.md`](docs/guide/i18n-fonts-icons.md) |
| Multi-window | [`docs/guide/multi-window.md`](docs/guide/multi-window.md) |
| Paint isolation | [`docs/guide/paint-isolation.md`](docs/guide/paint-isolation.md) |
| Architecture | [`docs/architecture/overview.md`](docs/architecture/overview.md) |
| Runtime | [`docs/architecture/runtime.md`](docs/architecture/runtime.md) |
| Input / IME / hit testing | [`docs/architecture/input-ime-hit.md`](docs/architecture/input-ime-hit.md) |
| Projection | [`docs/architecture/projection.md`](docs/architecture/projection.md) |
| Components / BSN | [`docs/guide/components-bsn.md`](docs/guide/components-bsn.md) |
| Markdown streaming | [`docs/guide/markdown-streaming.md`](docs/guide/markdown-streaming.md) |
| Testing | [`docs/guide/testing.md`](docs/guide/testing.md) |
| Public modules | [`docs/reference/public-modules.md`](docs/reference/public-modules.md) |
| Style tokens | [`docs/reference/style-tokens.md`](docs/reference/style-tokens.md) |
| Examples | [`docs/examples/index.md`](docs/examples/index.md) |
| Nested rules | `crates/picus_core/AGENTS.md`, `crates/picus_surface/AGENTS.md`, `examples/picuscode/AGENTS.md`, `thirdparty/AGENTS.md` |

## 7. Editing this file

- Keep hard constraints self-sufficient for the current scope.
- Move long narrative to `docs/` and leave a one-line rule + link.
- Do not restore tutorial encyclopedias into AGENTS.

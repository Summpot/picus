# picuscode example — agent rules

Integration example for CodeWhale + Picus. Not a product binary for end users.

## Hard rules

- **Do not** read, write, or delete the developer's real `~/.codewhale/` (or
  equivalent user config home) from tests or example default paths.
- Prefer fixture directories under the example or temp dirs for agent/config state.
- CodeWhale bridge and streaming Markdown are intentional advanced surfaces;
  keep application entry on the standard DX path:
  `PicusPlugin` + `add_ui_action` + `register_ui_components!` + `run_picus`.

## Sync

Full CodeWhale submodule procedure:
[`docs/contributing/codewhale-submodule.md`](../../docs/contributing/codewhale-submodule.md).
Do not edit files inside the submodule to “fix” Picus; use nested AGENTS under
`thirdparty/` for hard steps that must stay outside the submodule tree.

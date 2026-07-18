# Examples index

Prefer real examples over a separate “minimal” crate. Start with **timer** or
**calculator**.

| Example | Teaches | Advanced / retained pieces |
|---------|---------|----------------------------|
| `timer` | Full DX path: `UiAction`, macros, `run_picus`, explicit theme | Canvas dial, async tick task + `UiActionSender` |
| `calculator` | Keypad BSN composition + `UiAction` | Engine resource projection |
| `todo_list` | Dynamic entities, filters, text input | Virtual scroll list |
| `overlay_hit_routing` | Builtin click vs overlay hit order | Manual overlay spawn |
| `async_downloader` | Async tasks → `UiActionSender` / messages | Dialogs, IoTaskPool |
| `game_2048` | Keyboard + button actions | Custom hotkey widget |
| `chess_game` | Multi-resource projection, engine thread | Board grid projection |
| `gallery` | Full Fluent control surface | NavigationView shell, backdrop picker; **Spinner** / indeterminate **ProgressBar** use `PaintIsolation::AnimEntry` (anim host path — not full-window base encode on pure anim ticks). See [paint-isolation](../guide/paint-isolation.md). WinUI parity backlog: [gallery-winui-parity](../plans/gallery-winui-parity.md). |
| `picuscode` | Multi-window, streaming markdown | CodeWhale bridge (do not touch `~/.codewhale/` in tests) |

Every example loads a theme explicitly and uses `run_picus`.

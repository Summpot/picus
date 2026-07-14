# Styling and themes

## Contract (non-negotiable)

1. **No theme / no selected variant** → controls show **no** framework default
   visible fill or text colour (transparent / empty). This is not an error.
2. The framework **never** auto-selects Fluent dark or light.
3. **Partial themes are legal**: implement only the components you use. Missing
   component or property rules stay empty.
4. Errors are for **structure** only (bad RON, wrong value type, invalid token).

## Loading themes via `AppPicusExt`

| Method | Purpose |
|--------|---------|
| `load_style_sheet(path)` | Asset-path RON, hot-reload |
| `load_style_sheet_ron(text)` | Embedded RON |
| `style_variant(name)` | Select registered variant (`"dark"`, `"light"`, …) |
| `theme_backdrop(material)` | Override window backdrop |
| `clear_theme_backdrop_override()` | Clear backdrop override |

Priority when configured:

1. Explicit `style_variant` / already active variant  
2. Stylesheet `default_variant`  
3. **No fallback**

## Style layers (documentation)

| Layer | Use |
|-------|-----|
| 0 | No theme = no visible defaults |
| 1 | Load Fluent bundle / app RON + variant |
| 2 | Inline / builder styles (`InlineStyle`, `styled`) |
| 3 | Class + app RON override |
| 4 | Full multi-brand stylesheet |

### Helpers

- `styled(view, &resolved)` — apply a `ResolvedStyle` to any widget view
- `ProjectionCtx::styled(view)` — resolve this entity's style and apply it
- `InlineStyle::new().padding(8.0).bg(color).text_size(14.0)` — common inline builder

Production colours come from stylesheet RON, not widget defaults.

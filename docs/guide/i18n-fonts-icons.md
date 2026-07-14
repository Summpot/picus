# i18n, fonts, and icons

## i18n

- Register Fluent bundles with `AppPicusExt::register_i18n_bundle`.
- Resolve display strings through `resolve_localized_text` / `LocalizeText` during
  projection; do not hard-code locale branches in every widget.
- Missing keys should fall back to the authoring string without failing the frame.

## Fonts

- Register fonts with `register_xilem_font` / path / bytes helpers on `AppPicusExt`.
- Font registration **broadcasts to all windows** and **replays on attach** so late
  windows see the same font set (runtime hard rule; see root `AGENTS.md`).
- Prefer explicit font stacks on localized text when shaping must match a locale.

## Icons

- Use `FluentIcon` / `PicusIcon` / `icon_glyph` helpers from the facade.
- Icon colour and size follow resolved style when projected inside buttons and
  chrome; theme RON still owns production colours.

## Related

- Application entry: [app.md](app.md)
- Styling: [styling-themes.md](styling-themes.md)

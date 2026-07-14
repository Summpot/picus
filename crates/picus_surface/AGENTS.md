# picus_surface — agent rules

Window/surface integration for Picus. See root [`AGENTS.md`](../../AGENTS.md).

## Hard invariants

- Paint/present errors are **captured**; only a successful `present()` marks the
  surface as painted for this frame.
- Resize uses **logical** dimensions when configuring the window surface.
- Do not silently swallow GPU/device-lost failures without recording them on the
  runtime/error path used by diagnostics.

## Scope

- Keep platform hooks (e.g. Win32 create-window) local to this crate.
- Application code must not depend on `picus_surface` directly; go through the
  `picus` facade / runner.

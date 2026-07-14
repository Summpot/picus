# thirdparty — agent rules

## CodeWhale submodule

- Treat `thirdparty/CodeWhale` as an **upstream submodule**. Do not modify its
  tracked files to satisfy Picus-only needs.
- Sync procedure (full steps):
  [`docs/contributing/codewhale-submodule.md`](../docs/contributing/codewhale-submodule.md).
- Picus-side integration and test isolation rules live in
  [`examples/picuscode/AGENTS.md`](../examples/picuscode/AGENTS.md) and root
  `AGENTS.md` (do not touch the user's real `~/.codewhale/`).

## General

- Prefer vendoring or submodule pins over copying large third-party trees into
  `crates/`.
- When adding a new third-party dependency that ships its own agent rules, add a
  nested note here rather than editing the vendor tree.

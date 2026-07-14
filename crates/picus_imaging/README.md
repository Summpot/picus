# picus_imaging

Vendored Masonry imaging backend adapters for Picus.

This crate is adapted from Linebender's [`masonry_imaging`](https://github.com/linebender/xilem)
(Apache-2.0). It bridges Masonry retained paint output (`imaging` scenes) to concrete backends
(Vello / hybrid / CPU / Skia) and host-neutral WGPU texture rendering.

Picus depends on this path crate instead of the unpublished `masonry_imaging` package.
Upstream types live on crates.io (`imaging`, `imaging_vello`, `imaging_wgpu`, …).

**Desktop only** — wasm is not supported.

## License

Apache-2.0 (upstream Linebender copyright retained in source files).
Additional Picus packaging is MIT OR Apache-2.0 per the workspace.

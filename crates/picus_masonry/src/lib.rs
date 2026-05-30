// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Local Masonry widget set used by Picus.
//!
//! This crate vendors the upstream Masonry widgets/properties used by Picus so
//! `picus_core` can build on `masonry_core` directly without depending on the
//! upstream aggregate `masonry` crate.

#![forbid(unsafe_code)]
#![allow(
    clippy::all,
    missing_docs,
    reason = "Vendored upstream widget code is kept close to the source while Picus integration tests cover its behavior."
)]

pub mod layers;
pub mod properties;
pub mod theme;
pub mod widgets;

pub use accesskit;
pub use masonry_core::imaging;
pub use masonry_core::palette;
pub use masonry_core::{app, core, dpi, kurbo, layout, parley, peniko, ui_events, util};
pub use parley::{Alignment as TextAlign, AlignmentOptions as TextAlignOptions};

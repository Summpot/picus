// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Picus-owned retained UI backend.
//!
//! This crate is the long-term home for Picus' retained widget runtime. It
//! builds Picus widgets, properties, layers, and theme defaults directly on
//! Masonry Core while keeping Xilem view concerns in `picus_view`.

#![forbid(unsafe_code)]
#![allow(
    clippy::all,
    missing_docs,
    reason = "The retained backend still hosts migrated transitional code while Picus rewrites widgets in place."
)]

pub mod layers;
pub mod properties;
pub mod theme;
pub mod widgets;

pub use accesskit;
pub use masonry_core;
pub use masonry_core::imaging;
pub use masonry_core::palette;
pub use masonry_core::{app, core, dpi, kurbo, layout, parley, peniko, ui_events, util};
pub use parley::{Alignment as TextAlign, AlignmentOptions as TextAlignOptions};

/// Transitional namespace for the retained widget/property runtime.
pub mod retained {
    pub use super::accesskit;
    pub use super::imaging;
    pub use super::palette;
    pub use super::{
        TextAlign, TextAlignOptions, app, core, dpi, kurbo, layers, layout, parley, peniko,
        properties, theme, ui_events, util, widgets,
    };
}

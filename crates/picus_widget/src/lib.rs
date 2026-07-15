// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Picus-owned retained UI backend.
//!
//! This crate is the long-term home for Picus' retained widget runtime. It
//! builds Picus widgets, properties, and layers on Masonry Core while keeping
//! Xilem view concerns in `picus_view`.
//!
//! Widgets are lookless: paint uses property values only. Production apps get
//! colours and control skins from stylesheet RON via `picus_core`. Unbranded
//! geometry metrics live in [`theme`]; test harness skins live in the separate
//! `picus_theme_test` crate.

#![forbid(unsafe_code)]
#![allow(
    clippy::all,
    missing_docs,
    reason = "The retained backend still hosts migrated transitional code while Picus rewrites widgets in place."
)]

pub mod layers;
pub mod masonry_core;
pub mod paint_isolation;
pub mod properties;
pub mod theme;
pub mod widgets;
mod text_rendering;

pub use accesskit;
pub use masonry_core::imaging;
pub use masonry_core::palette;
pub use masonry_core::{app, core, dpi, kurbo, layout, parley, peniko, ui_events, util};
pub use paint_isolation::PaintIsolation;
pub use parley::{Alignment as TextAlign, AlignmentOptions as TextAlignOptions};

/// Panic in debug and `tracing::error` in release mode.
///
/// Historical path was `masonry_core::debug_panic`; the `masonry` facade does not
/// re-export that macro, so Picus defines it here.
#[macro_export]
macro_rules! debug_panic {
    ($msg:expr$(,)?) => {
        if cfg!(debug_assertions) {
            panic!($msg);
        } else {
            tracing::error!($msg);
        }
    };
    ($fmt:expr, $($arg:tt)+) => {
        if cfg!(debug_assertions) {
            panic!($fmt, $($arg)*);
        } else {
            tracing::error!($fmt, $($arg)*);
        }
    };
}

/// Transitional namespace for the retained widget/property runtime.
pub mod retained {
    pub use super::accesskit;
    pub use super::imaging;
    pub use super::palette;
    pub use super::{
        PaintIsolation, TextAlign, TextAlignOptions, app, core, dpi, kurbo, layers, layout,
        paint_isolation, parley, peniko, properties, theme, ui_events, util, widgets,
    };
}

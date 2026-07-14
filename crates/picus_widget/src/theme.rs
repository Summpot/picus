// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Neutral layout metrics and typography defaults for retained widgets.
//!
//! # Theming contract
//!
//! Production visual appearance comes from stylesheet RON resolved by
//! `picus_core` and written into widget properties. This module intentionally
//! does **not** ship a brand colour palette or a full control skin.
//!
//! - Missing paint properties → widgets draw nothing visible.
//! - Geometry metrics here are unbranded hit-target / type-scale constants.
//! - Test harness skins live in the separate `picus_theme_test` crate.

#![allow(missing_docs, reason = "Names are self-explanatory.")]

use crate::core::{StyleProperty, StyleSet};
use crate::layout::Length;
use crate::parley::{GenericFamily, LineHeight};

// ── Stroke / type metrics ───────────────────────────────────────────

/// Default border width for controls (`1px`).
pub const BORDER_WIDTH: Length = Length::const_px(1.);

/// Normal text size (14px body).
pub const TEXT_SIZE_NORMAL: f32 = 14.0;
pub const TEXT_SIZE_SMALL: f32 = 12.0;
pub const TEXT_SIZE_LARGE: f32 = 16.0;

// ── Control sizing (unbranded hit targets) ──────────────────────────

/// Base height for single-line controls (content box before padding).
pub const BASIC_WIDGET_HEIGHT: Length = Length::const_px(18.0);
/// Padding used inside control components like checkbox/radio indicator.
pub const WIDGET_CONTROL_COMPONENT_PADDING: Length = Length::const_px(4.0);
/// Minimum vertical hit target for horizontal sliders.
pub const SLIDER_HORIZONTAL_HEIGHT: f64 = 32.0;

// ── Scrollbar geometry (colours come from properties) ───────────────

pub const SCROLLBAR_WIDTH: f64 = 8.;
pub const SCROLLBAR_PAD: f64 = 2.;
pub const SCROLLBAR_MIN_SIZE: f64 = 45.;
pub const SCROLLBAR_RADIUS: f64 = 5.;
pub const SCROLLBAR_EDGE_WIDTH: f64 = 1.;

// ── Layout spacing ──────────────────────────────────────────────────

/// Default gap between flex children.
pub const DEFAULT_GAP: Length = Length::const_px(8.0);
/// Suggested spacer length for flex spacers.
pub const DEFAULT_SPACER_LEN: Length = Length::const_px(10.0);

// ── Corner radii (geometry scale; not a colour theme) ───────────────

pub const RADIUS_NONE: f64 = 0.;
pub const RADIUS_XS: f64 = 2.;
pub const RADIUS_SM: f64 = 4.;
pub const RADIUS_MD: f64 = 6.;
pub const RADIUS_LG: f64 = 8.;
pub const RADIUS_XL: f64 = 12.;
pub const RADIUS_PILL: f64 = 999.;

/// Applies neutral text styles for Masonry into `styles`.
///
/// Does not set a text colour; use [`crate::properties::ContentColor`]
/// (transparent by default) so missing theme data does not paint.
pub fn default_text_styles(styles: &mut StyleSet) {
    styles.insert(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
    styles.insert(GenericFamily::SystemUi.into());
}

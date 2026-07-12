// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Border brush property supporting both solid colors and gradients.
//!
//! This is the single source of truth for border paint, mirroring
//! [`Background`]'s `Color | Gradient` model. Widgets should paint borders
//! through [`paint_border_brush`] / [`pre_paint_brush`] rather than reading
//! [`BorderColor`] directly.
//!
//! [`Background`]: crate::properties::Background
//! [`BorderColor`]: crate::properties::BorderColor

use crate::core::{
    PaintCtx, PrePaintProps, PropertiesRef, Property, PropertyCache, paint_background,
    paint_box_shadow,
};
use crate::imaging::Painter;
use crate::kurbo::{Join, Rect, Stroke};
use crate::peniko::color::{AlphaColor, Srgb};
use crate::properties::types::Gradient;
use crate::properties::{BorderColor, BorderWidth, CornerRadius};

/// The brush used to paint a widget's border.
///
/// Mirrors [`Background`]: solid color or gradient. Prefer this property over
/// the legacy solid-only [`BorderColor`].
///
/// [`Background`]: crate::properties::Background
/// [`BorderColor`]: crate::properties::BorderColor
#[derive(Clone, Debug, PartialEq)]
pub enum BorderBrush {
    /// Solid color border.
    Color(AlphaColor<Srgb>),
    /// Gradient border.
    Gradient(Gradient),
}

impl Default for BorderBrush {
    fn default() -> Self {
        Self::Color(AlphaColor::TRANSPARENT)
    }
}

impl Property for BorderBrush {
    fn static_default() -> &'static Self {
        static DEFAULT: BorderBrush = BorderBrush::Color(AlphaColor::TRANSPARENT);
        &DEFAULT
    }
}

impl BorderBrush {
    /// Creates a solid-color border brush.
    #[must_use]
    pub const fn color(color: AlphaColor<Srgb>) -> Self {
        Self::Color(color)
    }

    /// Creates a gradient border brush.
    #[must_use]
    pub fn gradient(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }

    /// Returns a peniko brush suitable for stroking a border in `rect`.
    pub fn get_peniko_brush_for_rect(&self, rect: Rect) -> crate::peniko::Brush {
        match self {
            Self::Color(color) => (*color).into(),
            Self::Gradient(gradient) => gradient.get_peniko_gradient_for_rect(rect).into(),
        }
    }

    /// Returns the solid color when this brush is a solid color.
    #[must_use]
    pub const fn as_solid_color(&self) -> Option<AlphaColor<Srgb>> {
        match self {
            Self::Color(color) => Some(*color),
            Self::Gradient(_) => None,
        }
    }

    /// Returns `false` if the brush can be safely treated as non-existent.
    ///
    /// May have false positives (e.g. fully transparent gradient stops).
    pub const fn is_visible(&self) -> bool {
        match self {
            Self::Color(color) => {
                let alpha = color.components[3];
                alpha != 0.0
            }
            Self::Gradient(_) => true,
        }
    }
}

impl From<AlphaColor<Srgb>> for BorderBrush {
    fn from(color: AlphaColor<Srgb>) -> Self {
        Self::Color(color)
    }
}

impl From<Gradient> for BorderBrush {
    fn from(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }
}

impl From<BorderColor> for BorderBrush {
    fn from(border: BorderColor) -> Self {
        Self::Color(border.color)
    }
}

// ---------------------------------------------------------------------------
// Paint helpers
// ---------------------------------------------------------------------------

/// Resolves the effective border brush for a widget.
///
/// Prefer the explicit [`BorderBrush`] property when it is visible; otherwise
/// fall back to the legacy [`BorderColor`] property so theme defaults that
/// still insert only `BorderColor` keep working during migration.
pub fn resolve_border_brush(
    props: &PropertiesRef<'_>,
    cache: &mut PropertyCache,
) -> BorderBrush {
    let brush = props.get::<BorderBrush>(cache).clone();
    if brush.is_visible() {
        brush
    } else {
        BorderBrush::from(*props.get::<BorderColor>(cache))
    }
}

/// Paints a widget's border using a [`BorderBrush`] (solid color or gradient).
pub fn paint_border_brush(
    painter: &mut Painter<'_>,
    border_box: Rect,
    border_brush: &BorderBrush,
    border_width: &BorderWidth,
    corner_radius: &CornerRadius,
) {
    let border_width_value = border_width.width.get();
    if border_width_value == 0. || !border_brush.is_visible() {
        return;
    }
    let border_rect = border_width.border_rect(border_box, corner_radius);
    let border_style = Stroke {
        width: border_width_value,
        join: Join::Miter,
        ..Default::default()
    };
    let brush = border_brush.get_peniko_brush_for_rect(border_box);
    painter.stroke(border_rect, &border_style, &brush).draw();
}

/// Paints box shadow, background, and border with unified [`BorderBrush`] support.
///
/// This is the picus equivalent of masonry_core's `pre_paint`, using a single
/// border paint path (solid or gradient) resolved via [`resolve_border_brush`].
pub fn pre_paint_brush(
    ctx: &mut PaintCtx<'_>,
    props: &PropertiesRef<'_>,
    painter: &mut Painter<'_>,
) {
    let bbox = ctx.border_box();
    let cache = ctx.property_cache();
    let p = PrePaintProps::fetch(props, cache);

    paint_box_shadow(painter, bbox, p.box_shadow, p.corner_radius);
    paint_background(painter, bbox, p.background, p.border_width, p.corner_radius);

    let border_brush = resolve_border_brush(props, cache);
    paint_border_brush(painter, bbox, &border_brush, p.border_width, p.corner_radius);
}

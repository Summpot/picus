// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Border brush property supporting solid colors and gradients.
//!
//! This is the single source of truth for border paint. Widgets should paint
//! borders through [`paint_border_brush`] / [`pre_paint_brush`] rather than
//! reading [`BorderColor`] directly.
//!
//! WinUI elevation borders (`ControlElevationBorderBrush`) are modeled as
//! [`BorderBrush::AbsoluteLinear`]: `MappingMode=Absolute`, a short vertical
//! ramp (typically 3 DIP), optional vertical flip (`ScaleY=-1 CenterY=0.5`),
//! and pad extend so the rest of the control stays on the last stop.
//!
//! [`BorderColor`]: crate::properties::BorderColor

use crate::core::{
    PaintCtx, PrePaintProps, PropertiesRef, Property, PropertyCache, paint_background,
    paint_box_shadow,
};
use crate::imaging::Painter;
use crate::kurbo::{Join, Point, Rect, Stroke};
use crate::peniko::color::{AlphaColor, ColorSpaceTag, HueDirection, Srgb};
use crate::peniko::{
    ColorStop, ColorStops, Extend, InterpolationAlphaSpace, LinearGradientPosition,
};
use crate::properties::types::Gradient;
use crate::properties::{BorderColor, BorderWidth, CornerRadius};

/// Absolute linear gradient in device-independent pixels relative to the paint
/// target's top-left corner (WinUI `LinearGradientBrush` with
/// `MappingMode="Absolute"`).
///
/// `ControlElevationBorderBrush` is typically:
/// - `start = (0, 0)`, `end = (0, 3)`
/// - stops at 0.33 (Secondary) and 1.0 (Default)
/// - `flip_y = true` on Light / Accent (RelativeTransform `ScaleY=-1`)
#[derive(Clone, Debug, PartialEq)]
pub struct AbsoluteLinearGradient {
    /// Start point in DIP relative to the target's top-left.
    pub start: (f64, f64),
    /// End point in DIP relative to the target's top-left.
    pub end: (f64, f64),
    /// Apply WinUI `RelativeTransform` ScaleY=-1 CenterY=0.5 (reflect about
    /// the horizontal mid-line of the paint target).
    pub flip_y: bool,
    /// Color stops along the gradient line (`offset` in 0..=1).
    pub stops: Vec<(f32, AlphaColor<Srgb>)>,
}

impl AbsoluteLinearGradient {
    /// WinUI `ControlElevationBorderBrush` / `AccentControlElevationBorderBrush`
    /// shape: Absolute `(0,0)→(0,extent)`, two stops at 0.33 and 1.0.
    #[must_use]
    pub fn control_elevation(
        secondary: AlphaColor<Srgb>,
        default: AlphaColor<Srgb>,
        flip_y: bool,
    ) -> Self {
        Self {
            start: (0.0, 0.0),
            end: (0.0, 3.0),
            flip_y,
            stops: vec![(0.33, secondary), (1.0, default)],
        }
    }

    /// Resolves absolute start/end points in the paint target's coordinate space.
    #[must_use]
    pub fn resolve_points(&self, rect: Rect) -> (Point, Point) {
        let mut p0 = Point::new(rect.x0 + self.start.0, rect.y0 + self.start.1);
        let mut p1 = Point::new(rect.x0 + self.end.0, rect.y0 + self.end.1);
        if self.flip_y {
            // RelativeTransform ScaleY=-1 CenterY=0.5 → y' = 2*cy - y
            let cy = (rect.y0 + rect.y1) * 0.5;
            p0.y = 2.0 * cy - p0.y;
            p1.y = 2.0 * cy - p1.y;
        }
        (p0, p1)
    }

    /// Builds a peniko gradient covering `rect`.
    pub fn get_peniko_gradient_for_rect(&self, rect: Rect) -> crate::peniko::Gradient {
        let (start, end) = self.resolve_points(rect);
        let mut stops = ColorStops::default();
        for &stop in &self.stops {
            stops.push(ColorStop::from(stop));
        }
        crate::peniko::Gradient {
            kind: LinearGradientPosition { start, end }.into(),
            extend: Extend::Pad,
            interpolation_cs: ColorSpaceTag::Srgb,
            hue_direction: HueDirection::default(),
            stops,
            interpolation_alpha_space: InterpolationAlphaSpace::default(),
        }
    }
}

/// The brush used to paint a widget's border.
///
/// Prefer this property over the legacy solid-only [`BorderColor`].
///
/// [`BorderColor`]: crate::properties::BorderColor
#[derive(Clone, Debug, PartialEq)]
pub enum BorderBrush {
    /// Solid color border.
    Color(AlphaColor<Srgb>),
    /// CSS-style relative gradient (masonry [`Gradient`], spans the paint rect).
    Gradient(Gradient),
    /// WinUI-style absolute linear gradient (short DIP ramp, optional flip).
    AbsoluteLinear(AbsoluteLinearGradient),
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

    /// Creates a CSS-relative gradient border brush.
    #[must_use]
    pub fn gradient(gradient: Gradient) -> Self {
        Self::Gradient(gradient)
    }

    /// Creates a WinUI absolute linear border brush.
    #[must_use]
    pub fn absolute_linear(gradient: AbsoluteLinearGradient) -> Self {
        Self::AbsoluteLinear(gradient)
    }

    /// WinUI `ControlElevationBorderBrush` helper.
    #[must_use]
    pub fn control_elevation(
        secondary: AlphaColor<Srgb>,
        default: AlphaColor<Srgb>,
        flip_y: bool,
    ) -> Self {
        Self::AbsoluteLinear(AbsoluteLinearGradient::control_elevation(
            secondary, default, flip_y,
        ))
    }

    /// Returns a peniko brush suitable for stroking a border in `rect`.
    pub fn get_peniko_brush_for_rect(&self, rect: Rect) -> crate::peniko::Brush {
        match self {
            Self::Color(color) => (*color).into(),
            Self::Gradient(gradient) => gradient.get_peniko_gradient_for_rect(rect).into(),
            Self::AbsoluteLinear(gradient) => gradient.get_peniko_gradient_for_rect(rect).into(),
        }
    }

    /// Returns the solid color when this brush is a solid color.
    #[must_use]
    pub const fn as_solid_color(&self) -> Option<AlphaColor<Srgb>> {
        match self {
            Self::Color(color) => Some(*color),
            Self::Gradient(_) | Self::AbsoluteLinear(_) => None,
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
            Self::Gradient(_) | Self::AbsoluteLinear(_) => true,
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

impl From<AbsoluteLinearGradient> for BorderBrush {
    fn from(gradient: AbsoluteLinearGradient) -> Self {
        Self::AbsoluteLinear(gradient)
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

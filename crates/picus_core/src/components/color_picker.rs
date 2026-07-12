use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// An inline color picker that opens an overlay panel for color selection.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiColorPicker {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Whether the color picker overlay panel is currently open.
    pub is_open: bool,
}

impl UiColorPicker {
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            is_open: false,
        }
    }
}

// ---------------------------------------------------------------------------
// RGB ↔ HSV conversions (f32, hue in degrees 0..360).
// ---------------------------------------------------------------------------

/// Convert sRGB (0..255) to HSV with hue in degrees (0..360), S/V in 0..1.
#[must_use]
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max <= 0.0 { 0.0 } else { delta / max };

    let h = if delta <= 0.0 {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        // max is red
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < f32::EPSILON {
        // max is green
        60.0 * ((b - r) / delta + 2.0)
    } else {
        // max is blue
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, v)
}

/// Convert HSV (hue in degrees 0..360, S/V in 0..1) to sRGB (0..255).
#[must_use]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let to_u8 = |c: f32| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

/// Convert a hue (degrees 0..360) to sRGB at full saturation/value.
#[must_use]
pub fn hue_to_rgb(h: f32) -> (u8, u8, u8) {
    hsv_to_rgb(h, 1.0, 1.0)
}

/// Floating color picker panel (rendered in the overlay layer).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiColorPickerPanel {
    /// The [`UiColorPicker`] anchor entity this panel belongs to.
    pub anchor: Entity,
}

impl Default for UiColorPickerPanel {
    fn default() -> Self {
        Self {
            anchor: Entity::PLACEHOLDER,
        }
    }
}

/// Emitted when the selected color changes in a [`UiColorPicker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiColorPickerChanged {
    pub picker: Entity,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl UiComponentTemplate for UiColorPicker {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_color_picker(component, ctx)
    }
}

impl UiComponentTemplate for UiColorPickerPanel {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_color_picker_panel(component, ctx)
    }
}

// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Test-only theme fixtures for Picus retained widgets.
//!
//! Production apps must not depend on this crate. Visual appearance in
//! applications comes from stylesheet RON loaded by `picus_core`. This crate
//! supplies a self-contained dark Fluent-like [`DefaultProperties`] set so
//! widget unit tests and screenshot harnesses can paint without the ECS theme
//! stack.
//!
//! ## Contract
//!
//! - `picus_widget` stays lookless: missing properties draw nothing.
//! - Tests that need a visible control chrome call [`test_property_set`] or
//!   [`default_property_set`].

#![allow(missing_docs, reason = "Fixture token names are self-explanatory.")]

use picus_widget::core::{
    DefaultProperties, PropertySet, PropertyStack, Selector,
};
use picus_widget::layout::AsUnit;
use picus_widget::peniko::Color;
use picus_widget::properties::*;
use picus_widget::theme::{
    BORDER_WIDTH, DEFAULT_GAP, RADIUS_PILL, RADIUS_SM, RADIUS_XS,
};
use picus_widget::widgets::*;

// ──────────────────────────────────────────────
//  Fixture colour tokens (Fluent-like dark)
// ──────────────────────────────────────────────

pub const BACKGROUND_COLOR: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);

pub const GREY_10: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A);
pub const GREY_14: Color = Color::from_rgb8(0x24, 0x24, 0x24);
pub const GREY_16: Color = Color::from_rgb8(0x29, 0x29, 0x29);
pub const GREY_18: Color = Color::from_rgb8(0x2E, 0x2E, 0x2E);
pub const GREY_20: Color = Color::from_rgb8(0x33, 0x33, 0x33);
pub const GREY_22: Color = Color::from_rgb8(0x38, 0x38, 0x38);
pub const GREY_24: Color = Color::from_rgb8(0x3D, 0x3D, 0x3D);
pub const GREY_26: Color = Color::from_rgb8(0x42, 0x42, 0x42);
pub const GREY_28: Color = Color::from_rgb8(0x47, 0x47, 0x47);
pub const GREY_30: Color = Color::from_rgb8(0x4D, 0x4D, 0x4D);
pub const GREY_32: Color = Color::from_rgb8(0x52, 0x52, 0x52);
pub const GREY_34: Color = Color::from_rgb8(0x57, 0x57, 0x57);
pub const GREY_36: Color = Color::from_rgb8(0x5C, 0x5C, 0x5C);
pub const GREY_38: Color = Color::from_rgb8(0x61, 0x61, 0x61);
pub const GREY_40: Color = Color::from_rgb8(0x66, 0x66, 0x66);
pub const GREY_50: Color = Color::from_rgb8(0x80, 0x80, 0x80);
pub const GREY_60: Color = Color::from_rgb8(0x99, 0x99, 0x99);
pub const GREY_68: Color = Color::from_rgb8(0xAD, 0xAD, 0xAD);
pub const GREY_80: Color = Color::from_rgb8(0xCC, 0xCC, 0xCC);
pub const GREY_84: Color = Color::from_rgb8(0xD6, 0xD6, 0xD6);
pub const GREY_94: Color = Color::from_rgb8(0xF0, 0xF0, 0xF0);
pub const GREY_98: Color = Color::from_rgb8(0xFA, 0xFA, 0xFA);

pub const BRAND_COLOR: Color = Color::from_rgb8(0x00, 0x78, 0xD4);
pub const BRAND_COLOR_HOVER: Color = Color::from_rgb8(0x10, 0x6C, 0xBE);
pub const BRAND_COLOR_PRESSED: Color = Color::from_rgb8(0x00, 0x6C, 0xBE);
pub const ACCENT_COLOR: Color = BRAND_COLOR;

pub const TEXT_COLOR: Color = Color::from_rgb8(0xF3, 0xF3, 0xF3);
pub const TEXT_COLOR_SECONDARY: Color = GREY_84;
pub const TEXT_COLOR_TERTIARY: Color = GREY_68;
pub const DISABLED_TEXT_COLOR: Color = GREY_36;
pub const PLACEHOLDER_COLOR: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x8F);
pub const TEXT_BACKGROUND_COLOR: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A);
pub const FOCUS_COLOR: Color = Color::from_rgb8(0xFF, 0xFF, 0xFF);
pub const FOCUS_OUTER_COLOR: Color = Color::from_rgb8(0x00, 0x78, 0xD4);

pub const SURFACE_SUBTLE: Color = Color::from_rgb8(0x27, 0x27, 0x27);
pub const SURFACE_SUBTLE_HOVER: Color = Color::from_rgb8(0x31, 0x31, 0x31);
pub const SURFACE_SUBTLE_PRESSED: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);
pub const SURFACE_ELEVATED: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);
pub const SURFACE_PANEL: Color = Color::from_rgb8(0x24, 0x24, 0x24);
pub const SURFACE_CARD: Color = Color::from_rgb8(0x2D, 0x2D, 0x2D);
pub const SURFACE_INPUT: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A);
pub const SURFACE_DISABLED: Color = Color::from_rgb8(0x1C, 0x1C, 0x1C);
pub const SURFACE_ACCENT: Color = BRAND_COLOR;
pub const SURFACE_ACCENT_HOVER: Color = BRAND_COLOR_HOVER;

pub const BORDER_DEFAULT: Color = Color::from_rgb8(0x3F, 0x3F, 0x3F);
pub const BORDER_MUTED: Color = Color::from_rgb8(0x33, 0x33, 0x33);
pub const BORDER_SUBTLE: Color = Color::from_rgb8(0x2B, 0x2B, 0x2B);
pub const BORDER_DISABLED: Color = DISABLED_TEXT_COLOR;

pub const SCROLLBAR_TRACK: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x14);
pub const SCROLLBAR_THUMB: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x5C);
pub const SCROLLBAR_THUMB_HOVER: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x8C);
pub const SCROLLBAR_THUMB_PRESSED: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xB8);
pub const SCROLLBAR_COLOR: Color = SCROLLBAR_THUMB;
pub const SCROLLBAR_BORDER_COLOR: Color = Color::TRANSPARENT;

pub const CONTROL_STRONG_FILL: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x8B);
pub const SLIDER_OUTER_THUMB_BORDER: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x0F);

pub const STATUS_INFO_BG: Color = Color::from_rgb8(0x17, 0x32, 0x4D);
pub const STATUS_INFO_BORDER: Color = Color::from_rgb8(0x4C, 0xA0, 0xFF);
pub const STATUS_SUCCESS_BG: Color = Color::from_rgb8(0x17, 0x3A, 0x2A);
pub const STATUS_SUCCESS_BORDER: Color = Color::from_rgb8(0x6C, 0xCB, 0x5F);
pub const STATUS_WARNING_BG: Color = Color::from_rgb8(0x4B, 0x3B, 0x1A);
pub const STATUS_WARNING_BORDER: Color = Color::from_rgb8(0xF7, 0xC9, 0x4B);
pub const STATUS_ERROR_BG: Color = Color::from_rgb8(0x4B, 0x24, 0x24);
pub const STATUS_ERROR_BORDER: Color = Color::from_rgb8(0xFF, 0x99, 0xA4);

/// Full dark fixture property set for widget tests and screenshots.
pub fn default_property_set() -> DefaultProperties {
    let mut properties = DefaultProperties::new();

    // ── Badge ───────────────────────────────────────────────────────
    properties.insert::<Badge, _>(Padding::from_vh(3.px(), 5.px()));
    properties.insert::<Badge, _>(CornerRadius {
        radius: RADIUS_PILL.px(),
    });
    properties.insert::<Badge, _>(BorderWidth { width: 0.px() });
    properties.insert::<Badge, _>(Background::Color(SURFACE_ACCENT));
    properties.insert::<Badge, _>(BorderColor {
        color: SURFACE_SUBTLE,
    });
    properties.insert::<Badge, _>(ContentColor::new(Color::WHITE));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            Background::Color(SURFACE_DISABLED),
        );
        properties.insert_stack::<Badge>(stack);
    }

    // ── Button ──────────────────────────────────────────────────────
    properties.insert::<Button, _>(Padding::from_vh(5.px(), 14.px()));
    properties.insert::<Button, _>(CornerRadius {
        radius: RADIUS_SM.px(),
    });
    properties.insert::<Button, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<Button, _>(Background::Color(SURFACE_SUBTLE));
    properties.insert::<Button, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<Button, _>(ContentColor::new(TEXT_COLOR));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_hovered(true),
            (
                BorderColor {
                    color: BORDER_DEFAULT,
                },
                Background::Color(SURFACE_SUBTLE_HOVER),
            ),
        );
        stack.push(
            Selector::new().with_focused(true),
            (BorderColor {
                color: Color::TRANSPARENT,
            },),
        );
        stack.push(
            Selector::new().with_active(true),
            Background::Color(SURFACE_SUBTLE_PRESSED),
        );
        stack.push(
            Selector::new().with_disabled(true),
            (
                Background::Color(SURFACE_DISABLED),
                ContentColor::new(DISABLED_TEXT_COLOR),
            ),
        );
        properties.insert_stack::<Button>(stack);
    }

    // ── Checkbox ────────────────────────────────────────────────────
    properties.insert::<Checkbox, _>(CornerRadius {
        radius: RADIUS_XS.px(),
    });
    properties.insert::<Checkbox, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<Checkbox, _>(Background::Color(SURFACE_INPUT));
    properties.insert::<Checkbox, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<Checkbox, _>(CheckmarkStrokeWidth { width: 2.0 });
    properties.insert::<Checkbox, _>(CheckmarkColor { color: TEXT_COLOR });
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_hovered(true),
            BorderColor {
                color: BORDER_DEFAULT,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        stack.push(
            Selector::new().with_active(true),
            Background::Color(SURFACE_SUBTLE_PRESSED),
        );
        stack.push(
            Selector::new().with_disabled(true),
            PropertySet::new()
                .with(Background::Color(SURFACE_DISABLED))
                .with(CheckmarkColor {
                    color: DISABLED_TEXT_COLOR,
                }),
        );
        properties.insert_stack::<Checkbox>(stack);
    }

    // ── Divider ─────────────────────────────────────────────────────
    properties.insert::<Divider, _>(ContentColor::new(BORDER_DEFAULT));

    // ── Switch ──────────────────────────────────────────────────────
    properties.insert::<Switch, _>(CornerRadius {
        radius: RADIUS_PILL.px(),
    });
    properties.insert::<Switch, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<Switch, _>(Background::Color(SURFACE_SUBTLE));
    properties.insert::<Switch, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<Switch, _>(ThumbColor(Color::WHITE));
    properties.insert::<Switch, _>(ThumbRadius(RADIUS_PILL.px()));
    properties.insert::<Switch, _>(TrackThickness(20.px()));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_hovered(true),
            BorderColor {
                color: BORDER_DEFAULT,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        stack.push(
            Selector::classes(&["#toggled"]),
            (
                Background::Color(SURFACE_ACCENT),
                BorderColor {
                    color: SURFACE_ACCENT,
                },
            ),
        );
        stack.push(
            Selector::new().with_active(true),
            Background::Color(SURFACE_SUBTLE_PRESSED),
        );
        stack.push(
            Selector::new().with_disabled(true),
            Background::Color(SURFACE_DISABLED),
        );
        properties.insert_stack::<Switch>(stack);
    }

    // ── Flex / Grid ─────────────────────────────────────────────────
    properties.insert::<Flex, _>(Gap::new(DEFAULT_GAP));
    properties.insert::<Grid, _>(Gap::ZERO);

    // ── TextInput ───────────────────────────────────────────────────
    properties.insert::<TextInput, _>(Padding::from_vh(5.px(), 10.px()));
    properties.insert::<TextInput, _>(CornerRadius {
        radius: RADIUS_SM.px(),
    });
    properties.insert::<TextInput, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<TextInput, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<TextInput, _>(PlaceholderColor::new(PLACEHOLDER_COLOR));
    properties.insert::<TextInput, _>(CaretColor { color: TEXT_COLOR });
    properties.insert::<TextInput, _>(SelectionColor { color: BRAND_COLOR });
    properties.insert::<TextInput, _>(Background::Color(SURFACE_INPUT));
    properties.insert::<TextInput, _>(ContentColor::new(TEXT_COLOR));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::classes(&["#unfocused"]),
            SelectionColor {
                color: DISABLED_TEXT_COLOR,
            },
        );
        stack.push(
            Selector::new().with_hovered(true),
            BorderColor {
                color: BORDER_DEFAULT,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        stack.push(
            Selector::new().with_disabled(true),
            (
                Background::Color(SURFACE_DISABLED),
                ContentColor::new(DISABLED_TEXT_COLOR),
            ),
        );
        properties.insert_stack::<TextInput>(stack);
    }

    // ── TextArea ────────────────────────────────────────────────────
    properties.insert::<TextArea<false>, _>(ContentColor::new(TEXT_COLOR));
    properties.insert::<TextArea<false>, _>(CaretColor { color: TEXT_COLOR });
    properties.insert::<TextArea<false>, _>(SelectionColor { color: BRAND_COLOR });
    properties.insert::<TextArea<false>, _>(Background::Color(SURFACE_INPUT));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            ContentColor::new(DISABLED_TEXT_COLOR),
        );
        properties.insert_stack::<TextArea<false>>(stack);
    }
    properties.insert::<TextArea<true>, _>(ContentColor::new(TEXT_COLOR));
    properties.insert::<TextArea<true>, _>(CaretColor { color: TEXT_COLOR });
    properties.insert::<TextArea<true>, _>(SelectionColor { color: BRAND_COLOR });
    properties.insert::<TextArea<true>, _>(Background::Color(SURFACE_INPUT));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            ContentColor::new(DISABLED_TEXT_COLOR),
        );
        properties.insert_stack::<TextArea<true>>(stack);
    }

    // ── Label ───────────────────────────────────────────────────────
    properties.insert::<Label, _>(ContentColor::new(TEXT_COLOR));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            ContentColor::new(DISABLED_TEXT_COLOR),
        );
        properties.insert_stack::<Label>(stack);
    }

    // ── ProgressBar ─────────────────────────────────────────────────
    properties.insert::<ProgressBar, _>(CornerRadius {
        radius: RADIUS_XS.px(),
    });
    properties.insert::<ProgressBar, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<ProgressBar, _>(Background::Color(GREY_14));
    properties.insert::<ProgressBar, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<ProgressBar, _>(BarColor(BRAND_COLOR));

    // ── RadioButton ─────────────────────────────────────────────────
    properties.insert::<RadioButton, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<RadioButton, _>(Background::Color(SURFACE_INPUT));
    properties.insert::<RadioButton, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    properties.insert::<RadioButton, _>(CheckmarkColor { color: TEXT_COLOR });
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_active(true),
            Background::Color(SURFACE_SUBTLE_PRESSED),
        );
        stack.push(
            Selector::new().with_hovered(true),
            BorderColor {
                color: BORDER_DEFAULT,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        stack.push(
            Selector::new().with_disabled(true),
            (
                CheckmarkColor::new(DISABLED_TEXT_COLOR),
                Background::Color(SURFACE_DISABLED),
            ),
        );
        properties.insert_stack::<RadioButton>(stack);
    }

    // ── Slider ──────────────────────────────────────────────────────
    properties.insert::<Slider, _>(TrackThickness(4.px()));
    properties.insert::<Slider, _>(TrackColor {
        active: BRAND_COLOR,
        inactive: CONTROL_STRONG_FILL,
    });
    properties.insert::<Slider, _>(ThumbColor(Color::WHITE));
    properties.insert::<Slider, _>(ThumbRadius(9.px()));
    properties.insert::<Slider, _>(Background::Color(Color::TRANSPARENT));
    properties.insert::<Slider, _>(BorderWidth { width: 0.px() });
    properties.insert::<Slider, _>(BorderColor {
        color: SLIDER_OUTER_THUMB_BORDER,
    });
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_hovered(true),
            TrackColor {
                active: BRAND_COLOR_HOVER,
                inactive: CONTROL_STRONG_FILL,
            },
        );
        stack.push(
            Selector::new().with_active(true),
            TrackColor {
                active: BRAND_COLOR_PRESSED,
                inactive: CONTROL_STRONG_FILL,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        properties.insert_stack::<Slider>(stack);
    }

    // ── Spinner ─────────────────────────────────────────────────────
    properties.insert::<Spinner, _>(ContentColor::new(BRAND_COLOR));

    // ── ScrollBar ───────────────────────────────────────────────────
    properties.insert::<ScrollBar, _>(ContentColor::new(SCROLLBAR_COLOR));
    properties.insert::<ScrollBar, _>(BorderColor {
        color: SCROLLBAR_BORDER_COLOR,
    });

    // ── StepInput ───────────────────────────────────────────────────
    default_step_input_style::<u8>(&mut properties);
    default_step_input_style::<i8>(&mut properties);
    default_step_input_style::<u16>(&mut properties);
    default_step_input_style::<i16>(&mut properties);
    default_step_input_style::<u32>(&mut properties);
    default_step_input_style::<i32>(&mut properties);
    default_step_input_style::<u64>(&mut properties);
    default_step_input_style::<i64>(&mut properties);
    default_step_input_style::<usize>(&mut properties);
    default_step_input_style::<isize>(&mut properties);
    default_step_input_style::<f32>(&mut properties);
    default_step_input_style::<f64>(&mut properties);

    properties
}

/// Alias used by widget unit tests (same as [`default_property_set`] for now).
pub fn test_property_set() -> DefaultProperties {
    default_property_set()
}

fn default_step_input_style<T: Steppable>(properties: &mut DefaultProperties) {
    properties.insert::<StepInput<T>, _>(Padding::from_vh(5.px(), 0.px()));
    properties.insert::<StepInput<T>, _>(CornerRadius {
        radius: RADIUS_SM.px(),
    });
    properties.insert::<StepInput<T>, _>(BorderWidth {
        width: BORDER_WIDTH,
    });
    properties.insert::<StepInput<T>, _>(ContentColor::new(TEXT_COLOR));
    properties.insert::<StepInput<T>, _>(Background::Color(SURFACE_INPUT));
    properties.insert::<StepInput<T>, _>(BorderColor {
        color: BORDER_DEFAULT,
    });
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            (
                ContentColor::new(DISABLED_TEXT_COLOR),
                Background::Color(SURFACE_DISABLED),
            ),
        );
        stack.push(
            Selector::new().with_hovered(true),
            BorderColor {
                color: BORDER_DEFAULT,
            },
        );
        stack.push(
            Selector::new().with_focused(true),
            BorderColor {
                color: FOCUS_OUTER_COLOR,
            },
        );
        properties.insert_stack::<StepInput<T>>(stack);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_scrollbar_thumb_is_translucent() {
        assert_eq!(SCROLLBAR_BORDER_COLOR.to_rgba8().a, 0);
        assert!(SCROLLBAR_COLOR.to_rgba8().a < u8::MAX);
    }
}

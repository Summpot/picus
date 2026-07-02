// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Fluent Design v9 compatible default values used by various widgets in their paint methods.
//!
//! This module defines the dark-theme default tokens that align with the
//! Fluent 2 design system used by Microsoft's fluentui (web-components v9).
//! Light-theme and high-contrast variants are handled at the picus_core level
//! via `fluent_theme.ron`; these constants serve as the retained-widget defaults
//! when no per-instance or per-variant style override is provided.

#![allow(missing_docs, reason = "Names are self-explanatory.")]

use crate::core::{
    DefaultProperties, PropertySet, PropertyStack, Selector, StyleProperty, StyleSet,
};
use crate::layout::{AsUnit, Length};
use crate::parley::{GenericFamily, LineHeight};
use crate::peniko::Color;

// We use glob imports here to avoid frequent merge conflicts.
use crate::properties::*;
use crate::widgets::*;

// ──────────────────────────────────────────────
//  Fluent 2 Dark Theme — Base Colour Tokens
// ──────────────────────────────────────────────

/// Default background colour for the app surface.
///
/// Maps to `colorNeutralBackground1` / `surface-bg`.
pub const BACKGROUND_COLOR: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);

/// Default border width for controls (Fluent `strokeWidthThin`).
pub const BORDER_WIDTH: Length = Length::const_px(1.);

// ── Grey / neutral palette (Fluent v9 grey scale) ────────────────────
// Grey scale matching Fluent v9 grey ramp (rounded to nearest hex).
pub const GREY_10: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A); // grey[10]
pub const GREY_14: Color = Color::from_rgb8(0x24, 0x24, 0x24); // grey[14]
pub const GREY_16: Color = Color::from_rgb8(0x29, 0x29, 0x29); // grey[16]
pub const GREY_18: Color = Color::from_rgb8(0x2E, 0x2E, 0x2E); // grey[18]
pub const GREY_20: Color = Color::from_rgb8(0x33, 0x33, 0x33); // grey[20]
pub const GREY_22: Color = Color::from_rgb8(0x38, 0x38, 0x38); // grey[22]
pub const GREY_24: Color = Color::from_rgb8(0x3D, 0x3D, 0x3D); // grey[24]
pub const GREY_26: Color = Color::from_rgb8(0x42, 0x42, 0x42); // grey[26]
pub const GREY_28: Color = Color::from_rgb8(0x47, 0x47, 0x47); // grey[28]
pub const GREY_30: Color = Color::from_rgb8(0x4D, 0x4D, 0x4D); // grey[30]
pub const GREY_32: Color = Color::from_rgb8(0x52, 0x52, 0x52); // grey[32]
pub const GREY_34: Color = Color::from_rgb8(0x57, 0x57, 0x57); // grey[34]
pub const GREY_36: Color = Color::from_rgb8(0x5C, 0x5C, 0x5C); // grey[36]
pub const GREY_38: Color = Color::from_rgb8(0x61, 0x61, 0x61); // grey[38]
pub const GREY_40: Color = Color::from_rgb8(0x66, 0x66, 0x66); // grey[40]
pub const GREY_50: Color = Color::from_rgb8(0x80, 0x80, 0x80); // grey[50]
pub const GREY_60: Color = Color::from_rgb8(0x99, 0x99, 0x99); // grey[60]
pub const GREY_68: Color = Color::from_rgb8(0xAD, 0xAD, 0xAD); // grey[68]
pub const GREY_80: Color = Color::from_rgb8(0xCC, 0xCC, 0xCC); // grey[80]
pub const GREY_84: Color = Color::from_rgb8(0xD6, 0xD6, 0xD6); // grey[84]
pub const GREY_94: Color = Color::from_rgb8(0xF0, 0xF0, 0xF0); // grey[94]
pub const GREY_98: Color = Color::from_rgb8(0xFA, 0xFA, 0xFA); // grey[98]

// ── Brand / Accent ──────────────────────────────────────────────────
// Fluent v9 brandWeb, corresponding to `accent-primary` / `colorBrandBackground`.
pub const BRAND_COLOR: Color = Color::from_rgb8(0x00, 0x78, 0xD4); // brand[80]
pub const BRAND_COLOR_HOVER: Color = Color::from_rgb8(0x10, 0x6C, 0xBE); // brand[80] hover (slightly different)
pub const BRAND_COLOR_PRESSED: Color = Color::from_rgb8(0x00, 0x6C, 0xBE); // brand[80] pressed

/// Alias: Fluent brand accent (same as `BRAND_COLOR`).
pub const ACCENT_COLOR: Color = BRAND_COLOR;

// ── Foreground / text ───────────────────────────────────────────────
/// Primary text colour (`colorNeutralForeground1`).
pub const TEXT_COLOR: Color = Color::from_rgb8(0xF3, 0xF3, 0xF3); // #f3f3f3
/// Secondary text colour (`colorNeutralForeground2`).
pub const TEXT_COLOR_SECONDARY: Color = GREY_84;
/// Tertiary text colour (`colorNeutralForeground3`).
pub const TEXT_COLOR_TERTIARY: Color = GREY_68;
/// Disabled text colour (`colorNeutralForegroundDisabled`).
pub const DISABLED_TEXT_COLOR: Color = GREY_36;
/// Placeholder text colour (matches `colorNeutralForeground4` region).
pub const PLACEHOLDER_COLOR: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x8F);
/// Text background / surface-input (`colorNeutralBackground1` variant).
pub const TEXT_BACKGROUND_COLOR: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A);
/// Focus indicator colour (`colorStrokeFocus2`).
pub const FOCUS_COLOR: Color = Color::from_rgb8(0xFF, 0xFF, 0xFF);
/// Focus outline colour (`colorStrokeFocus2` typically white in dark).
pub const FOCUS_OUTER_COLOR: Color = Color::from_rgb8(0x00, 0x78, 0xD4);

// ── Surfaces ────────────────────────────────────────────────────────
pub const SURFACE_SUBTLE: Color = Color::from_rgb8(0x27, 0x27, 0x27); // #272727
pub const SURFACE_SUBTLE_HOVER: Color = Color::from_rgb8(0x31, 0x31, 0x31); // #313131
pub const SURFACE_SUBTLE_PRESSED: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F); // #1f1f1f
pub const SURFACE_ELEVATED: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F); // #1f1f1f
pub const SURFACE_PANEL: Color = Color::from_rgb8(0x24, 0x24, 0x24); // #242424
pub const SURFACE_CARD: Color = Color::from_rgb8(0x2D, 0x2D, 0x2D); // #2d2d2d
pub const SURFACE_INPUT: Color = Color::from_rgb8(0x1A, 0x1A, 0x1A); // #1a1a1a
pub const SURFACE_DISABLED: Color = Color::from_rgb8(0x1C, 0x1C, 0x1C); // #1c1c1c
pub const SURFACE_ACCENT: Color = BRAND_COLOR;
pub const SURFACE_ACCENT_HOVER: Color = BRAND_COLOR_HOVER;

// ── Borders ─────────────────────────────────────────────────────────
pub const BORDER_DEFAULT: Color = Color::from_rgb8(0x3F, 0x3F, 0x3F); // #3f3f3f
pub const BORDER_MUTED: Color = Color::from_rgb8(0x33, 0x33, 0x33); // #333333
pub const BORDER_SUBTLE: Color = Color::from_rgb8(0x2B, 0x2B, 0x2B); // #2b2b2b
pub const BORDER_DISABLED: Color = DISABLED_TEXT_COLOR;

// ── Misc ────────────────────────────────────────────────────────────
pub const SCROLLBAR_TRACK: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x14);
pub const SCROLLBAR_THUMB: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x5C);
pub const SCROLLBAR_THUMB_HOVER: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x8C);
pub const SCROLLBAR_THUMB_PRESSED: Color = Color::from_rgba8(0xFF, 0xFF, 0xFF, 0xB8);

// ── Typography ──────────────────────────────────────────────────────
/// Normal text size (Fluent `fontSizeBase300` = 14px).
pub const TEXT_SIZE_NORMAL: f32 = 14.0;
pub const TEXT_SIZE_SMALL: f32 = 12.0;
pub const TEXT_SIZE_LARGE: f32 = 16.0;

// ── Sizing ──────────────────────────────────────────────────────────
/// Base height for single-line controls (Fluent default 32px ≈ 18px content + padding).
pub const BASIC_WIDGET_HEIGHT: Length = Length::const_px(18.0);
/// Padding used inside control components like checkbox/radio indicator.
pub const WIDGET_CONTROL_COMPONENT_PADDING: Length = Length::const_px(4.0);

// ── Scrollbar defaults ──────────────────────────────────────────────
pub const SCROLLBAR_COLOR: Color = SCROLLBAR_THUMB;
pub const SCROLLBAR_BORDER_COLOR: Color = Color::from_rgb8(0x77, 0x77, 0x77);
pub const SCROLLBAR_WIDTH: f64 = 8.;
pub const SCROLLBAR_PAD: f64 = 2.;
pub const SCROLLBAR_MIN_SIZE: f64 = 45.;
pub const SCROLLBAR_RADIUS: f64 = 5.;
pub const SCROLLBAR_EDGE_WIDTH: f64 = 1.;

// ── Layout spacing ──────────────────────────────────────────────────
/// Default gap between flex children (Fluent `spacingHorizontalS` = 8px).
pub const DEFAULT_GAP: Length = Length::const_px(8.0);
pub const DEFAULT_SPACER_LEN: Length = Length::const_px(10.0);

// ── Border radii (Fluent v9) ────────────────────────────────────────
pub const RADIUS_NONE: f64 = 0.;
pub const RADIUS_XS: f64 = 2.;
pub const RADIUS_SM: f64 = 4.;
pub const RADIUS_MD: f64 = 6.;
pub const RADIUS_LG: f64 = 8.;
pub const RADIUS_XL: f64 = 12.;
pub const RADIUS_PILL: f64 = 999.;

// ── Status colours (Fluent v9 status) ───────────────────────────────
pub const STATUS_INFO_BG: Color = Color::from_rgb8(0x17, 0x32, 0x4D);
pub const STATUS_INFO_BORDER: Color = Color::from_rgb8(0x4C, 0xA0, 0xFF);
pub const STATUS_SUCCESS_BG: Color = Color::from_rgb8(0x17, 0x3A, 0x2A);
pub const STATUS_SUCCESS_BORDER: Color = Color::from_rgb8(0x6C, 0xCB, 0x5F);
pub const STATUS_WARNING_BG: Color = Color::from_rgb8(0x4B, 0x3B, 0x1A);
pub const STATUS_WARNING_BORDER: Color = Color::from_rgb8(0xF7, 0xC9, 0x4B);
pub const STATUS_ERROR_BG: Color = Color::from_rgb8(0x4B, 0x24, 0x24);
pub const STATUS_ERROR_BORDER: Color = Color::from_rgb8(0xFF, 0x99, 0xA4);

// ──────────────────────────────────────────────────────────────
//  Default property set (retained-widget defaults)
// ──────────────────────────────────────────────────────────────

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

    // ── Button (Fluent v9 — `colorNeutralBackground1` base) ─────────
    // Default appearance: neutral / outline-like (matches Fluent default)
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

    // ── Checkbox (Fluent v9) ───────────────────────────────────────
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

    // ── Switch (Fluent v9) ──────────────────────────────────────────
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

    // ── Flex ────────────────────────────────────────────────────────
    properties.insert::<Flex, _>(Gap::new(DEFAULT_GAP));

    // ── Grid ────────────────────────────────────────────────────────
    properties.insert::<Grid, _>(Gap::ZERO);

    // ── TextInput (Fluent v9) ───────────────────────────────────────
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

    // ── TextArea (Fluent v9) ────────────────────────────────────────
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

    // ── Label (Fluent v9) ───────────────────────────────────────────
    properties.insert::<Label, _>(ContentColor::new(TEXT_COLOR));
    {
        let mut stack = PropertyStack::new();
        stack.push(
            Selector::new().with_disabled(true),
            ContentColor::new(DISABLED_TEXT_COLOR),
        );
        properties.insert_stack::<Label>(stack);
    }

    // ── ProgressBar (Fluent v9) ─────────────────────────────────────
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

    // ── RadioButton (Fluent v9) ─────────────────────────────────────
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

    // ── Slider (Fluent v9) ──────────────────────────────────────────
    properties.insert::<Slider, _>(TrackThickness(4.px()));
    properties.insert::<Slider, _>(TrackColor {
        active: BRAND_COLOR,
        inactive: SURFACE_SUBTLE,
    });
    properties.insert::<Slider, _>(ThumbColor(TEXT_COLOR));
    properties.insert::<Slider, _>(ThumbRadius(7.px()));
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
        properties.insert_stack::<Slider>(stack);
    }

    // ── Spinner (Fluent v9) ─────────────────────────────────────────
    properties.insert::<Spinner, _>(ContentColor::new(BRAND_COLOR));

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

/// Applies the default text styles for Masonry into `styles`.
pub fn default_text_styles(styles: &mut StyleSet) {
    styles.insert(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
    styles.insert(GenericFamily::SystemUi.into());
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

/// Set of default properties used in unit tests.
///
/// This lets us change default properties without having to reset all screenshots every time.
/// This should still be kept relatively close to `default_property_set()` so that screenshots look like end user apps.
#[cfg(any())]
pub(crate) fn test_property_set() -> DefaultProperties {
    #[allow(unused_mut, reason = "Sometimes we don't have anything to change")]
    let mut properties = default_property_set();

    properties
}

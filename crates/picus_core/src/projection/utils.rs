use std::sync::Arc;

use super::core::UiView;
use crate::{
    ecs::LocalizeText,
    i18n::AppI18n,
    icons::{IconGlyph, PicusIcon},
    styling::{ResolvedStyle, apply_label_style, theme_default_font_family},
};
use bevy_ecs::prelude::*;
use crate::masonry_core::layout::Length;
use picus_view::style::Style as _;
use picus_view::view::{label, sized_box};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VectorIcon {
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronUp,
    ChevronRight,
    Clock,
    Info,
    Minus,
    RadioOff,
    RadioOn,
    SunMoon,
    X,
}

pub(crate) fn vector_icon(icon: VectorIcon, size_px: f64, color: crate::xilem::Color) -> UiView {
    let icon: IconGlyph = match icon {
        VectorIcon::Check => PicusIcon::Check,
        VectorIcon::ChevronDown => PicusIcon::ChevronDown,
        VectorIcon::ChevronLeft => PicusIcon::ChevronLeft,
        VectorIcon::ChevronUp => PicusIcon::ChevronUp,
        VectorIcon::ChevronRight => PicusIcon::ChevronRight,
        VectorIcon::Clock => PicusIcon::Clock,
        VectorIcon::Info => PicusIcon::Info,
        VectorIcon::Minus => PicusIcon::Minus,
        VectorIcon::RadioOff => PicusIcon::Circle,
        VectorIcon::RadioOn => PicusIcon::CircleDot,
        VectorIcon::SunMoon => PicusIcon::SunMoon,
        VectorIcon::X => PicusIcon::X,
    }
    .into();

    let mut icon_style = ResolvedStyle::default();
    icon_style.colors.text = Some(color);
    icon_style.text.size = (size_px * 0.90) as f32;
    icon_style.font_family = Some(icon.font_family_vec());

    Arc::new(
        sized_box(apply_label_style(
            label(icon.glyph().to_string()),
            &icon_style,
        ))
        .dims((Length::px(size_px), Length::px(size_px))),
    )
}

pub(crate) fn translate_text(world: &World, key: Option<&str>, fallback: &str) -> String {
    match key {
        Some(key) => world.get_resource::<AppI18n>().map_or_else(
            || {
                if fallback.is_empty() {
                    key.to_string()
                } else {
                    fallback.to_string()
                }
            },
            |i18n| i18n.translate(key),
        ),
        None => fallback.to_string(),
    }
}

pub(crate) fn transparentize(color: crate::xilem::Color) -> crate::xilem::Color {
    let rgba = color.to_rgba8();
    crate::xilem::Color::from_rgba8(rgba.r, rgba.g, rgba.b, 0)
}

pub(crate) fn hide_style_without_collapsing_layout(style: &mut ResolvedStyle) {
    style.colors.bg = Some(
        style
            .colors
            .bg
            .map_or(crate::xilem::Color::TRANSPARENT, transparentize),
    );
    style.colors.border = Some(
        style
            .colors
            .border
            .map_or(crate::xilem::Color::TRANSPARENT, transparentize),
    );
    style.colors.text = Some(
        style
            .colors
            .text
            .map_or(crate::xilem::Color::TRANSPARENT, transparentize),
    );
    style.box_shadow = None;
}

pub(crate) fn estimate_text_width_px(text: &str, font_size: f32) -> f64 {
    let units = text
        .chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                0.34
            } else if ch.is_ascii() {
                0.56
            } else {
                1.0
            }
        })
        .sum::<f64>();

    (units * font_size as f64).max(font_size as f64 * 2.0)
}

pub(crate) fn estimate_wrapped_lines(text: &str, font_size: f32, max_line_width: f64) -> usize {
    let max_line_width = max_line_width.max(font_size as f64 * 2.0);
    let mut total = 0_usize;

    for raw_line in text.lines() {
        let logical_line = if raw_line.is_empty() { " " } else { raw_line };
        let width = estimate_text_width_px(logical_line, font_size);
        let wrapped = (width / max_line_width).ceil() as usize;
        total += wrapped.max(1);
    }

    total.max(1)
}

pub(crate) fn app_i18n_font_stack(world: &World) -> Option<Vec<String>> {
    world
        .get_resource::<AppI18n>()
        .map(AppI18n::get_font_stack)
        .filter(|stack| !stack.is_empty())
}

pub(crate) fn apply_app_i18n_font_stack_for_text(style: &mut ResolvedStyle, world: &World) {
    let Some(stack) = app_i18n_font_stack(world) else {
        return;
    };

    if style.font_family.is_none()
        || style.font_family.as_ref() == theme_default_font_family(world).as_ref()
    {
        style.font_family = Some(stack);
    }
}

pub(crate) fn localized_font_stack(world: &World, entity: Entity) -> Option<Vec<String>> {
    world.get::<LocalizeText>(entity)?;

    app_i18n_font_stack(world)
}

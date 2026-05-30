use super::{
    core::{BuiltinUiAction, ProjectionCtx, UiView},
    utils::{VectorIcon, localized_font_stack, translate_text, vector_icon},
};
use crate::{
    ecs::{
        LocalizeText, UiBadge, UiButton, UiCheckbox, UiImage, UiLabel, UiMultilineTextInput,
        UiPasswordInput, UiProgressBar, UiSlider, UiSwitch, UiTextInput,
    },
    i18n::resolve_localized_text,
    styling::{
        apply_direct_widget_style, apply_label_style, apply_widget_style, font_stack_from_style,
        resolve_style, resolve_style_for_entity_classes,
    },
    views::{ecs_button_with_child, ecs_slider, ecs_text_input},
    widget_actions::WidgetUiAction,
};
use bevy_ecs::prelude::*;
use masonry_core::{
    layout::{Dim, Length, UnitPoint},
    properties::Padding,
};
use std::sync::Arc;
use tracing::trace;
use xilem_masonry::masonry::widgets::InsertNewline;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt as _, flex_row, image as xilem_image, label, sized_box,
    transformed, zstack,
};

const CHECKBOX_BOX_SIZE: f64 = 18.0;
const CHECKBOX_MARK_SIZE: f64 = 14.0;
const SWITCH_TRACK_WIDTH: f64 = 42.0;
const SWITCH_TRACK_HEIGHT: f64 = 22.0;
const SWITCH_THUMB_SIZE: f64 = 18.0;
const PROGRESS_BAR_WIDTH: f64 = 240.0;
const PROGRESS_BAR_HEIGHT: f64 = 8.0;
const PROGRESS_INDETERMINATE_WIDTH: f64 = 80.0;

fn placeholder_color_from_style(style: &crate::styling::ResolvedStyle) -> crate::xilem::Color {
    style
        .colors
        .text
        .unwrap_or(crate::xilem::Color::WHITE)
        .with_alpha(0.72)
}

fn map_text_alignment_for_input(
    text_align: crate::styling::TextAlign,
) -> masonry_core::parley::Alignment {
    match text_align {
        crate::styling::TextAlign::Start => masonry_core::parley::Alignment::Start,
        crate::styling::TextAlign::Center => masonry_core::parley::Alignment::Center,
        crate::styling::TextAlign::End => masonry_core::parley::Alignment::End,
    }
}

fn style_padding(value: f64) -> Padding {
    Padding::all(Length::px(value))
}

fn masked_text(value: &str, mask: char) -> String {
    value.chars().map(|_| mask).collect()
}

pub(crate) fn project_label(label_component: &UiLabel, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let text = resolve_localized_text(ctx.world, ctx.entity, &label_component.text);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }
    let localization_key = ctx
        .world
        .get::<LocalizeText>(ctx.entity)
        .map(|localize| localize.key.as_str());
    trace!(
        entity = ?ctx.entity,
        localization_key = ?localization_key,
        fallback_text = %label_component.text,
        resolved_text = %text,
        "projected UiLabel text"
    );
    Arc::new(apply_label_style(label(text), &style))
}

pub(crate) fn project_button(button_component: &UiButton, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let button_label_text = resolve_localized_text(ctx.world, ctx.entity, &button_component.label);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }
    let localization_key = ctx
        .world
        .get::<LocalizeText>(ctx.entity)
        .map(|localize| localize.key.as_str());
    trace!(
        entity = ?ctx.entity,
        localization_key = ?localization_key,
        fallback_text = %button_component.label,
        resolved_text = %button_label_text,
        "projected UiButton label"
    );

    let label_child = apply_label_style(label(button_label_text), &style);

    Arc::new(apply_direct_widget_style(
        ecs_button_with_child(ctx.entity, BuiltinUiAction::Clicked, label_child),
        &style,
    ))
}

pub(crate) fn project_badge(badge_component: &UiBadge, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let text = translate_text(
        ctx.world,
        badge_component.text_key.as_deref(),
        &badge_component.text,
    );

    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    Arc::new(apply_widget_style(
        apply_label_style(label(text), &style),
        &style,
    ))
}

pub(crate) fn project_checkbox(checkbox: &UiCheckbox, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let label_text = resolve_localized_text(ctx.world, ctx.entity, &checkbox.label);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let box_style = if checkbox.checked {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            ["template.checkbox.box", "template.checkbox.box.checked"],
        )
    } else {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.checkbox.box"])
    };
    let mark_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.checkbox.mark"]);
    let mark_color = mark_style
        .colors
        .text
        .or(style.colors.text)
        .unwrap_or(crate::xilem::Color::WHITE);
    let mark_size = (mark_style.text.size as f64).clamp(10.0, CHECKBOX_MARK_SIZE);

    let box_layer: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(CHECKBOX_BOX_SIZE)))
            .height(Dim::Fixed(Length::px(CHECKBOX_BOX_SIZE))),
        &box_style,
    ));
    let mut indicator_layers = vec![box_layer];
    if checkbox.checked {
        indicator_layers.push(vector_icon(VectorIcon::Check, mark_size, mark_color));
    }
    let indicator = zstack(indicator_layers).alignment(UnitPoint::CENTER);
    let label_child = apply_label_style(label(label_text), &style);

    let content = flex_row(vec![indicator.into_any_flex(), label_child.into_any_flex()])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(style.layout.gap.max(8.0)));

    Arc::new(apply_direct_widget_style(
        ecs_button_with_child(
            ctx.entity,
            WidgetUiAction::ToggleCheckbox {
                checkbox: ctx.entity,
            },
            content,
        ),
        &style,
    ))
}

pub(crate) fn project_slider(slider: &UiSlider, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    Arc::new(apply_widget_style(
        ecs_slider(
            ctx.entity,
            slider.min,
            slider.max,
            slider.value,
            move |value| WidgetUiAction::SetSliderValue {
                slider: ctx.entity,
                value,
            },
        ),
        &style,
    ))
}

pub(crate) fn project_switch(switch_component: &UiSwitch, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let track_style = if switch_component.on {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            ["template.switch.track", "template.switch.track.on"],
        )
    } else {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.switch.track"])
    };
    let thumb_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.switch.thumb"]);
    let thumb_x = if switch_component.on {
        SWITCH_TRACK_WIDTH - SWITCH_THUMB_SIZE - 2.0
    } else {
        2.0
    };

    let track: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(SWITCH_TRACK_WIDTH)))
            .height(Dim::Fixed(Length::px(SWITCH_TRACK_HEIGHT))),
        &track_style,
    ));
    let thumb: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(SWITCH_THUMB_SIZE)))
            .height(Dim::Fixed(Length::px(SWITCH_THUMB_SIZE))),
        &thumb_style,
    ));
    let switch_visual: UiView = Arc::new(
        zstack(vec![
            track,
            Arc::new(transformed(thumb).translate((thumb_x, 2.0))),
        ])
        .alignment(UnitPoint::TOP_LEFT),
    );

    let mut items = vec![switch_visual.into_any_flex()];
    if let Some(label_text) = switch_component
        .label
        .as_ref()
        .filter(|label| !label.is_empty())
        .map(|label| resolve_localized_text(ctx.world, ctx.entity, label))
    {
        items.push(apply_label_style(label(label_text), &style).into_any_flex());
    }

    let content = flex_row(items)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(style.layout.gap.max(8.0)));

    Arc::new(apply_direct_widget_style(
        ecs_button_with_child(
            ctx.entity,
            WidgetUiAction::ToggleSwitch { switch: ctx.entity },
            content,
        ),
        &style,
    ))
}

pub(crate) fn project_progress_bar(progress: &UiProgressBar, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let (fill_width, fill_offset, fill_style) = match progress.progress {
        Some(value) => (
            PROGRESS_BAR_WIDTH * value.clamp(0.0, 1.0),
            0.0,
            resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.progress.fill"]),
        ),
        None => (
            PROGRESS_INDETERMINATE_WIDTH,
            (PROGRESS_BAR_WIDTH - PROGRESS_INDETERMINATE_WIDTH) * 0.35,
            resolve_style_for_entity_classes(
                ctx.world,
                ctx.entity,
                ["template.progress.fill", "template.progress.indeterminate"],
            ),
        ),
    };

    let fill: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(fill_width.max(0.0))))
            .height(Dim::Fixed(Length::px(PROGRESS_BAR_HEIGHT))),
        &fill_style,
    ));
    let fill_layer = if fill_offset > 0.0 {
        Arc::new(transformed(fill).translate((fill_offset, 0.0))) as UiView
    } else {
        fill
    };

    Arc::new(apply_widget_style(
        sized_box(zstack(vec![fill_layer]).alignment(UnitPoint::LEFT))
            .width(Dim::Fixed(Length::px(PROGRESS_BAR_WIDTH)))
            .height(Dim::Fixed(Length::px(PROGRESS_BAR_HEIGHT))),
        &style,
    ))
}

pub(crate) fn project_text_input(input: &UiTextInput, ctx: ProjectionCtx<'_>) -> UiView {
    project_ecs_text_input(
        ctx,
        input.value.clone(),
        input.placeholder.clone(),
        true,
        InsertNewline::Never,
        false,
        |entity, value| WidgetUiAction::SetTextInput {
            input: entity,
            value,
        },
    )
}

pub(crate) fn project_password_input(input: &UiPasswordInput, ctx: ProjectionCtx<'_>) -> UiView {
    let mask = input.mask;
    project_ecs_text_input(
        ctx,
        masked_text(&input.value, mask),
        input.placeholder.clone(),
        true,
        InsertNewline::Never,
        input.read_only,
        move |entity, display_value| WidgetUiAction::SetPasswordInputDisplay {
            input: entity,
            display_value,
        },
    )
}

pub(crate) fn project_multiline_text_input(
    input: &UiMultilineTextInput,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    project_ecs_text_input(
        ctx,
        input.value.clone(),
        input.placeholder.clone(),
        input.clip,
        InsertNewline::OnEnter,
        input.read_only,
        |entity, value| WidgetUiAction::SetMultilineTextInput {
            input: entity,
            value,
        },
    )
}

fn project_ecs_text_input(
    ctx: ProjectionCtx<'_>,
    value: String,
    placeholder: String,
    clip: bool,
    insert_newline: InsertNewline,
    disabled: bool,
    map_action: impl Fn(Entity, String) -> WidgetUiAction + Send + Sync + 'static,
) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let scale = style.layout.scale.max(0.01);
    let entity = ctx.entity;
    let mut styled = ecs_text_input(entity, value, move |value| map_action(entity, value))
        .placeholder(placeholder)
        .text_size(style.text.size)
        .text_alignment(map_text_alignment_for_input(style.text.text_align))
        .clip(clip)
        .insert_newline(insert_newline)
        .disabled(disabled);

    if let Some(font_stack) = font_stack_from_style(&style) {
        styled = styled.font(font_stack);
    }

    let styled = styled.placeholder_color(placeholder_color_from_style(&style));

    if let Some(text_color) = style.colors.text {
        return Arc::new(
            transformed(
                styled
                    .text_color(text_color)
                    .padding(style_padding(style.layout.padding))
                    .corner_radius(Length::px(style.layout.corner_radius))
                    .border(
                        style
                            .colors
                            .border
                            .unwrap_or(crate::xilem::Color::TRANSPARENT),
                        Length::px(style.layout.border_width),
                    )
                    .background_color(style.colors.bg.unwrap_or(crate::xilem::Color::TRANSPARENT))
                    .box_shadow(style.box_shadow.unwrap_or_default()),
            )
            .scale(scale),
        );
    }

    Arc::new(
        transformed(
            styled
                .padding(style_padding(style.layout.padding))
                .corner_radius(Length::px(style.layout.corner_radius))
                .border(
                    style
                        .colors
                        .border
                        .unwrap_or(crate::xilem::Color::TRANSPARENT),
                    Length::px(style.layout.border_width),
                )
                .background_color(style.colors.bg.unwrap_or(crate::xilem::Color::TRANSPARENT))
                .box_shadow(style.box_shadow.unwrap_or_default()),
        )
        .scale(scale),
    )
}

pub(crate) fn project_image(image_component: &UiImage, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let Some(image_brush) = image_component.image_brush() else {
        let fallback_text = image_component.alt_text.clone().unwrap_or_default();
        return Arc::new(apply_widget_style(
            apply_label_style(label(fallback_text), &style),
            &style,
        ));
    };

    let mut image_view = xilem_image(image_brush).decorative(image_component.decorative);
    if let Some(alt_text) = &image_component.alt_text {
        image_view = image_view.alt_text(alt_text.clone());
    }

    Arc::new(apply_widget_style(
        image_view.fit(image_component.fit),
        &style,
    ))
}

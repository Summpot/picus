use super::{
    core::{BuiltinUiAction, ProjectionCtx, UiView},
    utils::{localized_font_stack, translate_text},
};
use crate::{
    ecs::{
        LocalizeText, PartSwitchThumb, PartSwitchTrack, UiBadge, UiButton, UiCheckbox, UiLabel,
        UiProgressBar, UiSlider, UiSwitch, UiTextInput,
    },
    i18n::resolve_localized_text,
    styling::{
        apply_direct_widget_style, apply_label_style, apply_widget_style, font_stack_from_style,
        resolve_style,
    },
    views::{ecs_button_with_child, ecs_checkbox, ecs_slider, ecs_text_input},
    widget_actions::WidgetUiAction,
};
use bevy_ecs::{hierarchy::Children, prelude::*};
use masonry::layout::Length;
use std::sync::Arc;
use tracing::trace;
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    FlexExt as _, badge, flex_row, label, progress_bar, sized_box, transformed,
};

fn child_entity_views(ctx: &ProjectionCtx<'_>) -> Vec<(Entity, UiView)> {
    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();

    child_entities
        .into_iter()
        .zip(ctx.children.iter().cloned())
        .collect::<Vec<_>>()
}

fn first_part_view<P: Component>(
    ctx: &ProjectionCtx<'_>,
    pairs: &[(Entity, UiView)],
) -> Option<UiView> {
    pairs
        .iter()
        .find_map(|(entity, view)| ctx.world.get::<P>(*entity).map(|_| view.clone()))
}

fn placeholder_color_from_style(style: &crate::styling::ResolvedStyle) -> xilem::Color {
    style
        .colors
        .text
        .unwrap_or(xilem::Color::WHITE)
        .with_alpha(0.72)
}

fn map_text_alignment_for_input(
    text_align: crate::styling::TextAlign,
) -> masonry::parley::Alignment {
    match text_align {
        crate::styling::TextAlign::Start => masonry::parley::Alignment::Start,
        crate::styling::TextAlign::Center => masonry::parley::Alignment::Center,
        crate::styling::TextAlign::End => masonry::parley::Alignment::End,
    }
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
        badge(apply_label_style(label(text), &style)),
        &style,
    ))
}

pub(crate) fn project_checkbox(checkbox: &UiCheckbox, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);

    let mut checkbox_view = ecs_checkbox(
        ctx.entity,
        checkbox.label.clone(),
        checkbox.checked,
        move |checked| WidgetUiAction::SetCheckbox {
            checkbox: ctx.entity,
            checked,
        },
    )
    .text_size(style.text.size);

    if let Some(font_stack) = font_stack_from_style(&style) {
        checkbox_view = checkbox_view.font(font_stack);
    }
    if let Some(text_color) = style.colors.text {
        checkbox_view = checkbox_view
            .text_color(text_color)
            .checkmark_color(text_color);
    }

    Arc::new(apply_direct_widget_style(checkbox_view, &style))
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
    let style = resolve_style(ctx.world, ctx.entity);
    let parts = child_entity_views(&ctx);

    let track = first_part_view::<PartSwitchTrack>(&ctx, &parts)
        .unwrap_or_else(|| Arc::new(label(if switch_component.on { "On" } else { "Off" })));
    let thumb =
        first_part_view::<PartSwitchThumb>(&ctx, &parts).unwrap_or_else(|| Arc::new(label("●")));

    let content = flex_row(vec![track.into_any_flex(), thumb.into_any_flex()])
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
    let scale = style.layout.scale.max(0.01);

    Arc::new(
        transformed(
            sized_box(
                progress_bar(progress.progress)
                    .corner_radius(style.layout.corner_radius)
                    .border(
                        style.colors.border.unwrap_or(xilem::Color::TRANSPARENT),
                        style.layout.border_width,
                    )
                    .background_color(style.colors.bg.unwrap_or(xilem::Color::TRANSPARENT)),
            )
            .padding(style.layout.padding),
        )
        .scale(scale),
    )
}

pub(crate) fn project_text_input(input: &UiTextInput, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let scale = style.layout.scale.max(0.01);
    let mut styled = ecs_text_input(ctx.entity, input.value.clone(), move |value| {
        WidgetUiAction::SetTextInput {
            input: ctx.entity,
            value,
        }
    })
    .placeholder(input.placeholder.clone())
    .text_size(style.text.size)
    .text_alignment(map_text_alignment_for_input(style.text.text_align));

    if let Some(font_stack) = font_stack_from_style(&style) {
        styled = styled.font(font_stack);
    }

    let styled = styled.placeholder_color(placeholder_color_from_style(&style));

    if let Some(text_color) = style.colors.text {
        return Arc::new(
            transformed(
                styled
                    .text_color(text_color)
                    .padding(style.layout.padding)
                    .corner_radius(style.layout.corner_radius)
                    .border(
                        style.colors.border.unwrap_or(xilem::Color::TRANSPARENT),
                        style.layout.border_width,
                    )
                    .background_color(style.colors.bg.unwrap_or(xilem::Color::TRANSPARENT))
                    .box_shadow(style.box_shadow.unwrap_or_default()),
            )
            .scale(scale),
        );
    }

    Arc::new(
        transformed(
            styled
                .padding(style.layout.padding)
                .corner_radius(style.layout.corner_radius)
                .border(
                    style.colors.border.unwrap_or(xilem::Color::TRANSPARENT),
                    style.layout.border_width,
                )
                .background_color(style.colors.bg.unwrap_or(xilem::Color::TRANSPARENT))
                .box_shadow(style.box_shadow.unwrap_or_default()),
        )
        .scale(scale),
    )
}

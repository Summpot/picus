use super::{
    core::{ProjectionCtx, UiView},
    utils::{
        VectorIcon, app_i18n_font_stack, estimate_text_width_px, estimate_wrapped_lines,
        hide_style_without_collapsing_layout, translate_text, vector_icon,
    },
};
use crate::{
    ecs::{OverlayComputedPosition, PartDialogBody, PartDialogDismiss, PartDialogTitle, UiDialog},
    overlay::OverlayUiAction,
    styling::{
        apply_direct_widget_style, apply_flex_alignment, apply_label_style, apply_widget_style,
        resolve_style, resolve_style_for_classes,
    },
    views::{ecs_button_with_child, opaque_hitbox_for_entity},
};
use bevy_ecs::{hierarchy::Children, prelude::Entity};
use masonry::layout::{Dim, Length};
use std::sync::Arc;
use xilem::{palette::css::BLACK, style::BoxShadow, style::Style as _};
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt as _, MainAxisAlignment, flex_col, flex_row, label, transformed,
};

pub(crate) const DIALOG_SURFACE_MIN_WIDTH: f64 = 240.0;
pub(crate) const DIALOG_SURFACE_MAX_WIDTH: f64 = 400.0;
pub(crate) const DIALOG_DISMISS_ICON_SIZE_PX: f64 = 16.0;
pub(crate) const DIALOG_DISMISS_BUTTON_SIZE_PX: f64 = 32.0;

pub(crate) fn dialog_surface_padding(layout_padding: f64) -> f64 {
    layout_padding.max(12.0)
}

pub(crate) fn dialog_surface_gap(layout_gap: f64) -> f64 {
    layout_gap.max(10.0)
}

pub(crate) fn estimate_dialog_surface_width_px(
    title: &str,
    body: &str,
    title_size: f32,
    body_size: f32,
    horizontal_padding: f64,
) -> f64 {
    let mut widest = estimate_text_width_px(title, title_size).max(DIALOG_DISMISS_BUTTON_SIZE_PX);

    for line in body.lines() {
        widest = widest.max(estimate_text_width_px(line, body_size));
    }

    (widest + horizontal_padding * 2.0 + 40.0)
        .clamp(DIALOG_SURFACE_MIN_WIDTH, DIALOG_SURFACE_MAX_WIDTH)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Layout estimator inputs intentionally mirror independently styled dialog fields"
)]
pub(crate) fn estimate_dialog_surface_height_px(
    title: &str,
    body: &str,
    dialog_surface_width: f64,
    title_size: f32,
    body_size: f32,
    gap: f64,
    horizontal_padding: f64,
    vertical_padding: f64,
) -> f64 {
    let title_line_height = (title_size as f64 * 1.35).max(18.0);
    let body_line_height = (body_size as f64 * 1.45).max(18.0);
    let dismiss_width = DIALOG_DISMISS_BUTTON_SIZE_PX;
    let dismiss_height = DIALOG_DISMISS_BUTTON_SIZE_PX;

    let header_text_max_width =
        (dialog_surface_width - horizontal_padding * 2.0 - dismiss_width - gap).max(120.0);
    let body_text_max_width = (dialog_surface_width - horizontal_padding * 2.0 - 8.0).max(120.0);
    let title_lines = estimate_wrapped_lines(title, title_size, header_text_max_width);
    let body_lines = estimate_wrapped_lines(body, body_size, body_text_max_width);
    let header_height = (title_lines as f64 * title_line_height).max(dismiss_height);

    (vertical_padding * 2.0 + header_height + gap + body_lines as f64 * body_line_height + gap)
        .max(120.0)
}

pub(crate) fn project_dialog(dialog: &UiDialog, ctx: ProjectionCtx<'_>) -> UiView {
    let mut dialog_style = resolve_style(ctx.world, ctx.entity);
    if dialog_style.colors.bg.is_none() {
        dialog_style.colors.bg = Some(xilem::Color::from_rgb8(0x18, 0x1E, 0x2D));
    }
    if dialog_style.colors.border.is_none() {
        dialog_style.colors.border = Some(xilem::Color::from_rgb8(0x3A, 0x48, 0x68));
    }
    if dialog_style.layout.padding <= 0.0 {
        dialog_style.layout.padding = 18.0;
    }
    if dialog_style.layout.corner_radius <= 0.0 {
        dialog_style.layout.corner_radius = 12.0;
    }
    if dialog_style.layout.border_width <= 0.0 {
        dialog_style.layout.border_width = 1.0;
    }
    if dialog_style.box_shadow.is_none() {
        dialog_style.box_shadow =
            Some(BoxShadow::new(BLACK.with_alpha(0.36), (0.0, 10.0)).blur(22.0));
    }

    let mut title_style = resolve_style_for_classes(ctx.world, ["overlay.dialog.title"]);
    let mut body_style = resolve_style_for_classes(ctx.world, ["overlay.dialog.body"]);
    let mut dismiss_style = resolve_style_for_classes(ctx.world, ["overlay.dialog.dismiss"]);
    if dismiss_style.layout.padding <= 0.0 {
        dismiss_style.layout.padding = 8.0;
    }

    let title = translate_text(ctx.world, dialog.title_key.as_deref(), &dialog.title);
    let body = translate_text(ctx.world, dialog.body_key.as_deref(), &dialog.body);
    let _dismiss_label = translate_text(
        ctx.world,
        dialog.dismiss_key.as_deref(),
        &dialog.dismiss_label,
    );

    if (dialog.title_key.is_some() || dialog.body_key.is_some() || dialog.dismiss_key.is_some())
        && let Some(stack) = app_i18n_font_stack(ctx.world)
    {
        title_style.font_family = Some(stack.clone());
        body_style.font_family = Some(stack.clone());
        dismiss_style.font_family = Some(stack);
    }

    let computed_position = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();

    let is_positioned = computed_position.is_positioned;
    if !is_positioned {
        hide_style_without_collapsing_layout(&mut dialog_style);
        hide_style_without_collapsing_layout(&mut title_style);
        hide_style_without_collapsing_layout(&mut body_style);
        hide_style_without_collapsing_layout(&mut dismiss_style);
    }

    let estimated_width = estimate_dialog_surface_width_px(
        &title,
        &body,
        title_style.text.size,
        body_style.text.size,
        dialog_style.layout.padding.max(12.0),
    );

    let hinted_width = dialog.width.unwrap_or(estimated_width);

    let dialog_gap = dialog_style.layout.gap.max(10.0);
    let estimated_height = estimate_dialog_surface_height_px(
        &title,
        &body,
        hinted_width,
        title_style.text.size,
        body_style.text.size,
        dialog_surface_gap(dialog_gap),
        dialog_surface_padding(dialog_style.layout.padding),
        dialog_surface_padding(dialog_style.layout.padding),
    );

    let dialog_surface_width = if computed_position.width > 1.0 {
        computed_position.width
    } else if let Some(width) = dialog.width {
        width
    } else {
        estimated_width
    };

    let dialog_surface_height = if computed_position.height > 1.0 {
        computed_position.height
    } else if let Some(height) = dialog.height {
        height
    } else {
        estimated_height
    };

    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    let child_parts = child_entities
        .into_iter()
        .zip(ctx.children.iter().cloned())
        .collect::<Vec<_>>();

    let part_view = |predicate: &dyn Fn(Entity) -> bool| {
        child_parts
            .iter()
            .find_map(|(entity, view)| predicate(*entity).then_some(view.clone()))
    };

    let title_view = if is_positioned {
        part_view(&|entity| ctx.world.get::<PartDialogTitle>(entity).is_some())
            .unwrap_or_else(|| Arc::new(apply_label_style(label(title.clone()), &title_style)))
    } else {
        Arc::new(apply_label_style(label(title.clone()), &title_style))
    };

    let body_view = if is_positioned {
        part_view(&|entity| ctx.world.get::<PartDialogBody>(entity).is_some())
            .unwrap_or_else(|| Arc::new(apply_label_style(label(body.clone()), &body_style)))
    } else {
        Arc::new(apply_label_style(label(body.clone()), &body_style))
    };

    let dismiss_button = apply_direct_widget_style(
        ecs_button_with_child(
            ctx.entity,
            OverlayUiAction::DismissDialog,
            vector_icon(
                VectorIcon::X,
                DIALOG_DISMISS_ICON_SIZE_PX,
                dismiss_style.colors.text.unwrap_or(xilem::Color::WHITE),
            ),
        ),
        &dismiss_style,
    )
    .into_any_flex();

    let extra_body_children = child_parts.into_iter().filter_map(|(entity, view)| {
        (ctx.world.get::<PartDialogTitle>(entity).is_none()
            && ctx.world.get::<PartDialogBody>(entity).is_none()
            && ctx.world.get::<PartDialogDismiss>(entity).is_none())
        .then_some(view.into_any_flex())
    });

    let header = flex_row((title_view.flex(1.0).into_any_flex(), dismiss_button))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .width(Dim::Stretch)
        .gap(Length::px(dialog_gap));

    let mut body_children = vec![body_view.into_any_flex()];
    body_children.extend(extra_body_children);
    let body = flex_col(body_children)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch)
        .gap(Length::px(dialog_gap));

    let dialog_children = vec![header.into_any_flex(), body.flex(1.0).into_any_flex()];

    let dialog_surface = xilem_masonry::view::sized_box(apply_widget_style(
        apply_flex_alignment(
            flex_col(dialog_children).cross_axis_alignment(CrossAxisAlignment::Stretch),
            &dialog_style,
        )
        .gap(Length::px(dialog_gap)),
        &dialog_style,
    ))
    .fixed_width(Length::px(dialog_surface_width))
    .fixed_height(Length::px(dialog_surface_height));

    let dialog_panel = transformed(opaque_hitbox_for_entity(ctx.entity, dialog_surface))
        .translate((computed_position.x, computed_position.y));

    Arc::new(dialog_panel)
}

#[cfg(test)]
mod tests {
    use super::{
        DIALOG_DISMISS_BUTTON_SIZE_PX, DIALOG_SURFACE_MAX_WIDTH, DIALOG_SURFACE_MIN_WIDTH,
        estimate_dialog_surface_height_px, estimate_dialog_surface_width_px,
    };

    #[test]
    fn dialog_surface_width_estimation_is_clamped() {
        let width = estimate_dialog_surface_width_px(
            "Very long modal title that should hit max width",
            "This is a long body line that should also be measured for width and then clamped.",
            24.0,
            16.0,
            16.0,
        );

        assert!((DIALOG_SURFACE_MIN_WIDTH..=DIALOG_SURFACE_MAX_WIDTH).contains(&width));
        assert_eq!(
            estimate_dialog_surface_width_px("", "", 24.0, 16.0, 16.0),
            DIALOG_SURFACE_MIN_WIDTH
        );
    }

    #[test]
    fn dialog_surface_height_estimation_uses_fixed_icon_close_footprint() {
        let height =
            estimate_dialog_surface_height_px("Title", "Body", 280.0, 24.0, 16.0, 10.0, 16.0, 16.0);

        assert!(height >= DIALOG_DISMISS_BUTTON_SIZE_PX + 32.0);
    }
}

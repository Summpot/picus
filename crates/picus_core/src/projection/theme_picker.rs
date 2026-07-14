use std::sync::Arc;

use crate::xilem::style::Style as _;
use crate::masonry_core::layout::{Dim, Length};
use picus_view::view::{
    CrossAxisAlignment, FlexExt as _, flex_col, flex_row, label, portal, transformed,
};

use crate::{
    ActiveStyleVariant,
    ecs::{OverlayAnchorRect, UiThemePicker, UiThemePickerMenu},
    overlay::OverlayUiAction,
    retained_bridge::{button_with_child_view, opaque_hitbox_for_entity},
    styling::{
        apply_direct_widget_style, apply_flex_alignment, apply_label_style, apply_widget_style,
        resolve_style, resolve_style_for_classes,
    },
};

use super::{
    core::{ProjectionCtx, UiView},
    dropdown::{estimate_dropdown_surface_width_px, estimate_dropdown_viewport_height_px},
    popover::popover_geometry,
    utils::{VectorIcon, apply_app_i18n_font_stack_for_text, translate_text, vector_icon},
};

fn selected_theme_index(world: &bevy_ecs::world::World, picker: &UiThemePicker) -> Option<usize> {
    let active_variant = world
        .get_resource::<ActiveStyleVariant>()
        .and_then(|active| active.0.as_deref());
    picker.active_index_for_variant(active_variant)
}

pub(crate) fn project_theme_picker(_: &UiThemePicker, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let icon: UiView = match style.colors.text {
        Some(icon_color) => vector_icon(VectorIcon::SunMoon, 16.0, icon_color),
        None => Arc::new(label("")),
    };

    Arc::new(apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleThemePicker, icon),
        &style,
    ))
}

pub(crate) fn project_theme_picker_menu(
    panel: &UiThemePickerMenu,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    let picker = match ctx.world.get::<UiThemePicker>(panel.anchor) {
        Some(picker) => picker,
        None => return Arc::new(label("")),
    };

    let mut menu_style = resolve_style_for_classes(ctx.world, ["overlay.dropdown.menu"]);

    let mut item_style = resolve_style_for_classes(ctx.world, ["overlay.dropdown.item"]);

    if picker
        .options
        .iter()
        .any(|option| option.label_key.is_some())
    {
        apply_app_i18n_font_stack_for_text(&mut item_style, ctx.world);
    }

    let translated_options = picker
        .options
        .iter()
        .map(|option| translate_text(ctx.world, option.label_key.as_deref(), &option.label))
        .collect::<Vec<_>>();

    let selected_index = selected_theme_index(ctx.world, picker);
    let icon_color = item_style.colors.text;

    let anchor_width = ctx
        .world
        .get::<OverlayAnchorRect>(ctx.entity)
        .map(|anchor_rect| anchor_rect.width)
        .unwrap_or(40.0);

    let estimated_width = estimate_dropdown_surface_width_px(
        anchor_width.max(1.0),
        translated_options.iter().map(String::as_str),
        item_style.text.size,
        item_style.layout.padding * 2.0 + menu_style.layout.padding * 2.0 + 18.0,
    );
    let estimated_height = estimate_dropdown_viewport_height_px(
        translated_options.len().max(1),
        item_style.text.size,
        item_style.layout.padding,
        menu_style.layout.gap,
    );

    let computed_position = popover_geometry(
        ctx.world,
        ctx.entity,
        (estimated_width, estimated_height),
        &mut [&mut menu_style, &mut item_style],
    );

    let items = translated_options
        .into_iter()
        .enumerate()
        .map(|(index, label_text)| {
            let mut content_items = Vec::new();
            if let Some(icon_color) = icon_color {
                let indicator = if selected_index == Some(index) {
                    vector_icon(VectorIcon::RadioOn, 14.0, icon_color)
                } else {
                    vector_icon(VectorIcon::RadioOff, 14.0, icon_color)
                };
                content_items.push(indicator.into_any_flex());
            }
            content_items.push(
                apply_label_style(label(label_text), &item_style)
                    .flex(1.0)
                    .into_any_flex(),
            );
            let content = flex_row(content_items)
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(Length::px(item_style.layout.gap));

            let item_button = button_with_child_view(
                ctx.entity,
                OverlayUiAction::SelectThemePickerItem { index },
                content,
            )
            .width(Dim::Stretch);

            apply_direct_widget_style(item_button, &item_style).into_any_flex()
        })
        .collect::<Vec<_>>();

    let panel_content = portal(
        apply_flex_alignment(
            flex_col(items).cross_axis_alignment(CrossAxisAlignment::Stretch),
            &menu_style,
        )
        .width(Dim::Stretch)
        .gap(Length::px(menu_style.layout.gap)),
    )
    .dims((
        Length::px(computed_position.width),
        Length::px(computed_position.height),
    ));

    Arc::new(
        transformed(opaque_hitbox_for_entity(
            ctx.entity,
            apply_widget_style(panel_content, &menu_style),
        ))
        .translate((computed_position.x, computed_position.y)),
    )
}

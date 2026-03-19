use std::sync::Arc;

use masonry::layout::{Dim, Length};
use xilem::{Color, palette::css::BLACK, style::BoxShadow, style::Style as _};
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt as _, MainAxisAlignment, flex_col, flex_row, label, portal,
    transformed,
};

use crate::{
    ActiveStyleVariant,
    ecs::{OverlayAnchorRect, UiThemePicker, UiThemePickerMenu},
    overlay::OverlayUiAction,
    styling::{
        apply_direct_widget_style, apply_flex_alignment, apply_label_style, apply_widget_style,
        resolve_style, resolve_style_for_classes,
    },
    views::{ecs_button_with_child, opaque_hitbox_for_entity},
};

use super::{
    core::{ProjectionCtx, UiView},
    dropdown::{estimate_dropdown_surface_width_px, estimate_dropdown_viewport_height_px},
    popover::popover_geometry,
    utils::{VectorIcon, app_i18n_font_stack, translate_text, vector_icon},
};

fn selected_theme_index(world: &bevy_ecs::world::World, picker: &UiThemePicker) -> Option<usize> {
    let active_variant = world
        .get_resource::<ActiveStyleVariant>()
        .and_then(|active| active.0.as_deref());
    picker.active_index_for_variant(active_variant)
}

pub(crate) fn project_theme_picker(_: &UiThemePicker, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    if style.layout.padding <= 0.0 {
        style.layout.padding = 6.0;
    }
    if style.layout.corner_radius <= 0.0 {
        style.layout.corner_radius = 999.0;
    }
    if style.layout.border_width <= 0.0 {
        style.layout.border_width = 1.0;
    }

    let icon_color = style
        .colors
        .text
        .unwrap_or(Color::from_rgb8(0xF3, 0xF3, 0xF3));
    let icon = vector_icon(VectorIcon::SunMoon, 16.0, icon_color);

    let button = apply_direct_widget_style(
        ecs_button_with_child(ctx.entity, OverlayUiAction::ToggleThemePicker, icon),
        &style,
    );

    Arc::new(
        flex_row(vec![button.into_any_flex()])
            .main_axis_alignment(MainAxisAlignment::End)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .width(Dim::Stretch),
    )
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
    if menu_style.colors.bg.is_none() {
        menu_style.colors.bg = Some(Color::from_rgb8(0x1F, 0x1F, 0x1F));
    }
    if menu_style.colors.border.is_none() {
        menu_style.colors.border = Some(Color::from_rgb8(0x3F, 0x3F, 0x3F));
    }
    if menu_style.layout.padding <= 0.0 {
        menu_style.layout.padding = 8.0;
    }
    if menu_style.layout.corner_radius <= 0.0 {
        menu_style.layout.corner_radius = 10.0;
    }
    if menu_style.layout.border_width <= 0.0 {
        menu_style.layout.border_width = 1.0;
    }
    if menu_style.box_shadow.is_none() {
        menu_style.box_shadow = Some(BoxShadow::new(BLACK.with_alpha(0.28), (0.0, 8.0)).blur(16.0));
    }

    let mut item_style = resolve_style_for_classes(ctx.world, ["overlay.dropdown.item"]);
    if item_style.layout.padding <= 0.0 {
        item_style.layout.padding = 8.0;
    }
    if item_style.text.size <= 0.0 {
        item_style.text.size = 15.0;
    }

    if picker
        .options
        .iter()
        .any(|option| option.label_key.is_some())
        && let Some(stack) = app_i18n_font_stack(ctx.world)
    {
        item_style.font_family = Some(stack);
    }

    let translated_options = picker
        .options
        .iter()
        .map(|option| translate_text(ctx.world, option.label_key.as_deref(), &option.label))
        .collect::<Vec<_>>();

    let selected_index = selected_theme_index(ctx.world, picker);
    let icon_color = item_style
        .colors
        .text
        .unwrap_or(Color::from_rgb8(0xF3, 0xF3, 0xF3));

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
    let item_gap = menu_style.layout.gap.max(6.0);
    let estimated_height = estimate_dropdown_viewport_height_px(
        translated_options.len().max(1),
        item_style.text.size,
        item_style.layout.padding,
        item_gap,
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
            let indicator = if selected_index == Some(index) {
                vector_icon(VectorIcon::RadioOn, 14.0, icon_color)
            } else {
                vector_icon(VectorIcon::RadioOff, 14.0, icon_color)
            };
            let content = flex_row(vec![
                indicator.into_any_flex(),
                apply_label_style(label(label_text), &item_style)
                    .flex(1.0)
                    .into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(8.0));

            let item_button = ecs_button_with_child(
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
        .gap(Length::px(item_gap)),
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

use std::sync::Arc;

use crate::xilem::Color;
use crate::xilem::style::Style as _;
use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::Component,
};
use masonry_core::imaging::Painter;
use masonry_core::kurbo::{Axis, BezPath, Circle, Line, Point, Rect, Stroke};
use masonry_core::peniko::{self, Gradient};
use masonry_core::{
    layout::{Dim, Length},
    properties::Dimensions,
};
use picus_view::picus_widget::widgets::InsertNewline;
use picus_view::view::{
    CrossAxisAlignment, FlexExt as _, MainAxisAlignment, canvas, divider_h, divider_v, flex_col,
    flex_item, flex_row, image as xilem_image, label, radio_group as xilem_radio_group, sized_box,
    spinner, split, transformed, zstack,
};

use crate::{
    icon::icon,
    ecs::{
        AnchoredTo, MessageBarKind, OverlayComputedPosition, PartScrollBarHorizontal,
        PartScrollBarVertical, PartScrollThumbHorizontal, PartScrollThumbVertical,
        PartScrollViewport, ScrollAxis, SplitDirection, ToastKind, UiBreadcrumbItem, UiCanvas,
        UiCanvasCommand, UiCanvasPathCommand, UiCanvasPosition, UiColorPicker, UiColorPickerPanel,
        UiContextMenu, UiDataTable, UiDatePicker, UiDatePickerPanel, UiDivider, UiExpander,
        UiGradientStop, UiGroupBox, UiListSelectionMode, UiListView, UiMenuBar, UiMenuBarItem,
        UiMenuItemPanel, UiMessageBar, UiNavigationItem, UiNavigationView, UiRadioGroup,
        NAV_PANE_COMPACT_WIDTH, NAV_PANE_EXPANDED_WIDTH,
        UiScrollView, UiSearch, UiSortDirection, UiSpinner, UiSplitPane, UiTabBar, UiTable,
        UiTimePicker, UiTimePickerPanel, UiToast, UiTooltip, UiTreeNode,
    },
    icons::FluentIcon,
    overlay::OverlayUiAction,
    retained_bridge::{
        button_view, button_with_child_view, color_spectrum_view, drag_thumb_view,
        opaque_hitbox_for_entity, radio_button_view, scroll_portal, slider_view, text_input_view,
    },
    styling::{
        ResolvedStyle, apply_direct_text_input_style, apply_direct_widget_style,
        apply_flex_alignment, apply_label_style, apply_widget_style, font_stack_from_style,
        resolve_style, resolve_style_for_classes, resolve_style_for_entity_classes,
    },
    widget_actions::WidgetUiAction,
};

use super::core::{ProjectionCtx, UiView};
use super::popover::popover_geometry;
use super::utils::{VectorIcon, hide_style_without_collapsing_layout, vector_icon};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if is_leap { 29 } else { 28 }
        }
        _ => 30,
    }
}

/// Returns weekday of the 1st of the month. 0=Sun, 1=Mon, …, 6=Sat.
fn day_of_week_for_first(year: i32, month: u32) -> u32 {
    let t: [u32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = year;
    if month < 3 {
        y -= 1;
    }
    let y = y as u32;
    (y + y / 4 - y / 100 + y / 400 + t[(month - 1) as usize] + 1) % 7
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "——",
    }
}

const DATE_PICKER_CELL_SIZE: f64 = 32.0;
const DATE_PICKER_DOW_HEIGHT: f64 = 24.0;
const DATE_PICKER_NAV_HEIGHT: f64 = 32.0;
const DATE_PICKER_GRID_GAP: f64 = 2.0;
const DATE_PICKER_ROW_GAP: f64 = 4.0;

fn tree_node_depth(world: &bevy_ecs::world::World, entity: bevy_ecs::entity::Entity) -> u32 {
    let mut depth = 0u32;
    let mut current = entity;
    while let Some(child_of) = world.get::<ChildOf>(current) {
        current = child_of.parent();
        if world.get::<UiTreeNode>(current).is_some() {
            depth += 1;
        }
    }
    depth
}

/// Resolve the panel style from the active theme.
fn default_panel_style(world: &bevy_ecs::world::World, class: &str) -> ResolvedStyle {
    resolve_style_for_classes(world, [class])
}

fn default_item_style(world: &bevy_ecs::world::World, class: &str) -> ResolvedStyle {
    resolve_style_for_classes(world, [class])
}

fn apply_color_overrides(base: &mut ResolvedStyle, overrides: &ResolvedStyle) {
    if overrides.colors.bg.is_some() {
        base.colors.bg = overrides.colors.bg;
    }
    if overrides.colors.text.is_some() {
        base.colors.text = overrides.colors.text;
    }
    if overrides.colors.border.is_some() {
        base.colors.border = overrides.colors.border;
    }
}

fn selected_row_style(
    world: &bevy_ecs::world::World,
    base_class: &str,
    selected_class: &str,
) -> ResolvedStyle {
    let mut style = default_item_style(world, base_class);
    let overrides = resolve_style_for_classes(world, [base_class, selected_class]);
    apply_color_overrides(&mut style, &overrides);
    style
}

fn apply_optional_item_padding(style: &mut ResolvedStyle, padding: Option<f64>) {
    if let Some(padding) = padding {
        style.layout.padding = padding;
    }
}

/// Returns the entity and (x, y) of a positioned overlay, or None if not yet positioned.
fn overlay_position(
    world: &bevy_ecs::world::World,
    entity: bevy_ecs::entity::Entity,
) -> Option<(f64, f64)> {
    let pos = world.get::<OverlayComputedPosition>(entity)?;
    if pos.is_positioned {
        Some((pos.x, pos.y))
    } else {
        None
    }
}

/// Create a hidden empty placeholder when an overlay isn't positioned yet.
fn hidden_placeholder() -> UiView {
    Arc::new(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(0.0)))
            .height(Dim::Fixed(Length::px(0.0))),
    )
}

fn child_entity_views(ctx: &ProjectionCtx<'_>) -> Vec<(Entity, UiView)> {
    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    child_entities
        .into_iter()
        .zip(ctx.children.iter().cloned())
        .collect::<Vec<_>>()
}

fn first_part_entity<P: Component>(
    ctx: &ProjectionCtx<'_>,
    pairs: &[(Entity, UiView)],
) -> Option<Entity> {
    pairs
        .iter()
        .find_map(|(entity, _)| ctx.world.get::<P>(*entity).map(|_| *entity))
}

const SCROLLBAR_THICKNESS: f64 = 12.0;
const SCROLLBAR_MIN_THUMB: f64 = 24.0;

fn thumb_length(viewport: f64, content: f64) -> f64 {
    if content <= 0.0 {
        return viewport.max(0.0);
    }
    let ratio = (viewport / content).clamp(0.0, 1.0);
    (viewport * ratio).clamp(SCROLLBAR_MIN_THUMB.min(viewport), viewport)
}

fn thumb_offset(current_offset: f64, max_offset: f64, track_len: f64, thumb_len: f64) -> f64 {
    let travel = (track_len - thumb_len).max(0.0);
    if max_offset <= f64::EPSILON {
        0.0
    } else {
        (current_offset / max_offset).clamp(0.0, 1.0) * travel
    }
}

pub(crate) fn project_scroll_view(scroll_view: &UiScrollView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let pairs = child_entity_views(&ctx);

    let viewport_part = first_part_entity::<PartScrollViewport>(&ctx, &pairs);
    let vertical_track_part = first_part_entity::<PartScrollBarVertical>(&ctx, &pairs);
    let vertical_thumb_part = first_part_entity::<PartScrollThumbVertical>(&ctx, &pairs);
    let horizontal_track_part = first_part_entity::<PartScrollBarHorizontal>(&ctx, &pairs);
    let horizontal_thumb_part = first_part_entity::<PartScrollThumbHorizontal>(&ctx, &pairs);

    let mut scroll_state = *scroll_view;
    scroll_state.clamp_scroll_offset();

    let viewport_w = (scroll_state.viewport_size.x as f64).max(32.0);
    let viewport_h = (scroll_state.viewport_size.y as f64).max(32.0);
    let content_w = (scroll_state.content_size.x as f64).max(viewport_w);
    let content_h = (scroll_state.content_size.y as f64).max(viewport_h);

    let content_views = pairs
        .iter()
        .filter_map(|(entity, view)| {
            let is_template_part = ctx.world.get::<PartScrollViewport>(*entity).is_some()
                || ctx.world.get::<PartScrollBarVertical>(*entity).is_some()
                || ctx.world.get::<PartScrollThumbVertical>(*entity).is_some()
                || ctx.world.get::<PartScrollBarHorizontal>(*entity).is_some()
                || ctx
                    .world
                    .get::<PartScrollThumbHorizontal>(*entity)
                    .is_some();
            (!is_template_part).then_some(view.clone().into_any_flex())
        })
        .collect::<Vec<_>>();

    let content_view = if content_views.is_empty() {
        flex_col(vec![label("").into_any_flex()])
    } else {
        flex_col(content_views)
    }
    .cross_axis_alignment(CrossAxisAlignment::Start);

    let portal = scroll_portal(
        content_view,
        Point::new(
            scroll_state.scroll_offset.x as f64,
            scroll_state.scroll_offset.y as f64,
        ),
    )
    .constrain_horizontal(!scroll_state.show_horizontal_scrollbar)
    .constrain_vertical(!scroll_state.show_vertical_scrollbar)
    .content_must_fill(true);

    let viewport_style = viewport_part
        .map(|entity| resolve_style(ctx.world, entity))
        .unwrap_or_else(|| resolve_style_for_classes(ctx.world, ["template.scroll_view.viewport"]));

    let viewport_surface = apply_widget_style(
        sized_box(portal).width(Dim::Stretch).height(Dim::Stretch),
        &viewport_style,
    );

    let max_x = (content_w - viewport_w).max(0.0);
    let max_y = (content_h - viewport_h).max(0.0);

    let show_vertical = scroll_state.show_vertical_scrollbar && max_y > f64::EPSILON;
    let show_horizontal = scroll_state.show_horizontal_scrollbar && max_x > f64::EPSILON;

    let vertical_bar_view = if show_vertical {
        let track_style = vertical_track_part
            .map(|entity| resolve_style(ctx.world, entity))
            .unwrap_or_else(|| {
                resolve_style_for_classes(ctx.world, ["template.scroll_view.scrollbar.vertical"])
            });
        let thumb_style = vertical_thumb_part
            .map(|entity| resolve_style(ctx.world, entity))
            .unwrap_or_else(|| {
                resolve_style_for_classes(ctx.world, ["template.scroll_view.thumb.vertical"])
            });

        let track_len = viewport_h;
        let thumb_len = thumb_length(viewport_h, content_h);
        let thumb_y = thumb_offset(
            scroll_state.scroll_offset.y as f64,
            max_y,
            track_len,
            thumb_len,
        );

        let track = apply_widget_style(
            sized_box(label(""))
                .width(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS)))
                .height(Dim::Fixed(Length::px(track_len))),
            &track_style,
        );

        let thumb_body = if let Some(thumb_entity) = vertical_thumb_part {
            drag_thumb_view(thumb_entity, ScrollAxis::Vertical, "")
        } else {
            drag_thumb_view(ctx.entity, ScrollAxis::Vertical, "")
        };

        let thumb = apply_widget_style(
            sized_box(thumb_body)
                .width(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS)))
                .height(Dim::Fixed(Length::px(thumb_len))),
            &thumb_style,
        );

        Some(zstack((track, transformed(thumb).translate((0.0, thumb_y)))).into_any_flex())
    } else {
        None
    };

    let horizontal_bar_view = if show_horizontal {
        let track_style = horizontal_track_part
            .map(|entity| resolve_style(ctx.world, entity))
            .unwrap_or_else(|| {
                resolve_style_for_classes(ctx.world, ["template.scroll_view.scrollbar.horizontal"])
            });
        let thumb_style = horizontal_thumb_part
            .map(|entity| resolve_style(ctx.world, entity))
            .unwrap_or_else(|| {
                resolve_style_for_classes(ctx.world, ["template.scroll_view.thumb.horizontal"])
            });

        let track_len = viewport_w;
        let thumb_len = thumb_length(viewport_w, content_w);
        let thumb_x = thumb_offset(
            scroll_state.scroll_offset.x as f64,
            max_x,
            track_len,
            thumb_len,
        );

        let track = apply_widget_style(
            sized_box(label(""))
                .width(Dim::Fixed(Length::px(track_len)))
                .height(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS))),
            &track_style,
        );

        let thumb_body = if let Some(thumb_entity) = horizontal_thumb_part {
            drag_thumb_view(thumb_entity, ScrollAxis::Horizontal, "")
        } else {
            drag_thumb_view(ctx.entity, ScrollAxis::Horizontal, "")
        };

        let thumb = apply_widget_style(
            sized_box(thumb_body)
                .width(Dim::Fixed(Length::px(thumb_len)))
                .height(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS))),
            &thumb_style,
        );

        Some(zstack((track, transformed(thumb).translate((thumb_x, 0.0)))).into_any_flex())
    } else {
        None
    };

    let mut top_row = vec![viewport_surface.flex(1.0).into_any_flex()];
    if let Some(vertical_bar) = vertical_bar_view {
        top_row.push(vertical_bar);
    }

    let mut rows = vec![flex_item(flex_row(top_row).gap(Length::px(0.0)), 1.0).into_any_flex()];

    if let Some(horizontal_bar) = horizontal_bar_view {
        let mut bottom_row = vec![horizontal_bar];
        if show_vertical {
            bottom_row.push(
                sized_box(label(""))
                    .width(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS)))
                    .height(Dim::Fixed(Length::px(SCROLLBAR_THICKNESS)))
                    .into_any_flex(),
            );
        }
        rows.push(flex_row(bottom_row).gap(Length::px(0.0)).into_any_flex());
    }

    Arc::new(
        sized_box(apply_widget_style(
            apply_flex_alignment(flex_col(rows), &style)
                .gap(Length::px(0.0))
                .dims(
                    Dimensions::AUTO
                        .with_width(Dim::Stretch)
                        .with_height(Dim::Stretch),
                ),
            &style,
        ))
        .dims(
            Dimensions::AUTO
                .with_width(Dim::Stretch)
                .with_height(Dim::Stretch),
        ),
    )
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

fn canvas_path(commands: &[UiCanvasPathCommand]) -> BezPath {
    let mut path = BezPath::new();
    for command in commands {
        match *command {
            UiCanvasPathCommand::MoveTo { x, y } => path.move_to((x, y)),
            UiCanvasPathCommand::LineTo { x, y } => path.line_to((x, y)),
            UiCanvasPathCommand::QuadTo { x1, y1, x, y } => path.quad_to((x1, y1), (x, y)),
            UiCanvasPathCommand::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => path.curve_to((x1, y1), (x2, y2), (x, y)),
            UiCanvasPathCommand::ClosePath => path.close_path(),
        }
    }
    path
}

/// Build a peniko linear gradient from picus gradient stops.
fn linear_gradient(start: (f64, f64), end: (f64, f64), stops: &[UiGradientStop]) -> Gradient {
    let mut gradient = Gradient::new_linear(start, end);
    gradient.stops = peniko::ColorStops::from(
        stops
            .iter()
            .map(|stop| peniko::ColorStop::from((stop.offset, stop.color)))
            .collect::<Vec<_>>()
            .as_slice(),
    );
    gradient
}

/// Build a peniko radial gradient from picus gradient stops.
fn radial_gradient(
    center: (f64, f64),
    inner_radius: f32,
    outer_radius: f32,
    stops: &[UiGradientStop],
) -> Gradient {
    let mut gradient = Gradient::new_radial(center, outer_radius);
    gradient.stops = peniko::ColorStops::from(
        stops
            .iter()
            .map(|stop| peniko::ColorStop::from((stop.offset, stop.color)))
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let _ = inner_radius; // inner radius uses two-point radial; single-radius uses center
    gradient
}

pub(crate) fn project_canvas(canvas_component: &UiCanvas, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let commands = canvas_component.commands.clone();
    let mut canvas_view = canvas(
        move |_: &mut (),
              _: &mut masonry_core::core::MutateCtx<'_>,
              scene: &mut masonry_core::imaging::record::Scene,
              size| {
            let mut painter = Painter::new(scene);
            for command in &commands {
                match command {
                    UiCanvasCommand::FillCanvas { color } => {
                        let rect = Rect::new(0.0, 0.0, size.width, size.height);
                        painter.fill(rect, *color).draw();
                    }
                    UiCanvasCommand::StrokeCanvas {
                        color,
                        stroke_width,
                    } => {
                        let rect = Rect::new(0.0, 0.0, size.width, size.height);
                        painter
                            .stroke(rect, &Stroke::new(*stroke_width), *color)
                            .draw();
                    }
                    UiCanvasCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    } => {
                        painter
                            .fill(Rect::new(*x, *y, *x + *width, *y + *height), *color)
                            .draw();
                    }
                    UiCanvasCommand::FillRoundedRect {
                        x,
                        y,
                        width,
                        height,
                        radius,
                        color,
                    } => {
                        painter
                            .fill(
                                Rect::new(*x, *y, *x + *width, *y + *height)
                                    .to_rounded_rect(*radius),
                                *color,
                            )
                            .draw();
                    }
                    UiCanvasCommand::StrokeRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                        stroke_width,
                    } => {
                        painter
                            .stroke(
                                Rect::new(*x, *y, *x + *width, *y + *height),
                                &Stroke::new(*stroke_width),
                                *color,
                            )
                            .draw();
                    }
                    UiCanvasCommand::StrokeRoundedRect {
                        x,
                        y,
                        width,
                        height,
                        radius,
                        color,
                        stroke_width,
                    } => {
                        painter
                            .stroke(
                                Rect::new(*x, *y, *x + *width, *y + *height)
                                    .to_rounded_rect(*radius),
                                &Stroke::new(*stroke_width),
                                *color,
                            )
                            .draw();
                    }
                    UiCanvasCommand::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        color,
                        stroke_width,
                    } => {
                        painter
                            .stroke(
                                Line::new((*x1, *y1), (*x2, *y2)),
                                &Stroke::new(*stroke_width),
                                *color,
                            )
                            .draw();
                    }
                    UiCanvasCommand::FillCircle {
                        cx,
                        cy,
                        radius,
                        color,
                    } => {
                        painter
                            .fill(Circle::new((*cx, *cy), *radius), *color)
                            .draw();
                    }
                    UiCanvasCommand::StrokeCircle {
                        cx,
                        cy,
                        radius,
                        color,
                        stroke_width,
                    } => {
                        painter
                            .stroke(
                                Circle::new((*cx, *cy), *radius),
                                &Stroke::new(*stroke_width),
                                *color,
                            )
                            .draw();
                    }
                    UiCanvasCommand::FillPath { commands, color } => {
                        painter.fill(canvas_path(commands), *color).draw();
                    }
                    UiCanvasCommand::StrokePath {
                        commands,
                        color,
                        stroke_width,
                    } => {
                        painter
                            .stroke(canvas_path(commands), &Stroke::new(*stroke_width), *color)
                            .draw();
                    }
                    UiCanvasCommand::FillLinearGradientRect {
                        x,
                        y,
                        width,
                        height,
                        start_x,
                        start_y,
                        end_x,
                        end_y,
                        stops,
                    } => {
                        let gradient =
                            linear_gradient((*start_x, *start_y), (*end_x, *end_y), stops);
                        painter
                            .fill(Rect::new(*x, *y, *x + *width, *y + *height), &gradient)
                            .draw();
                    }
                    UiCanvasCommand::FillRadialGradientCircle {
                        cx,
                        cy,
                        radius,
                        inner_radius,
                        stops,
                    } => {
                        let gradient = radial_gradient(
                            (*cx, *cy),
                            *inner_radius as f32,
                            *radius as f32,
                            stops,
                        );
                        painter
                            .fill(Circle::new((*cx, *cy), *radius), &gradient)
                            .draw();
                    }
                }
            }
        },
    );
    if let Some(alt_text) = &canvas_component.alt_text {
        canvas_view = canvas_view.alt_text(alt_text.clone());
    }

    let mut layers: Vec<UiView> = vec![Arc::new(canvas_view)];
    for (entity, child) in child_entity_views(&ctx) {
        let offset = ctx
            .world
            .get::<UiCanvasPosition>(entity)
            .copied()
            .unwrap_or_default()
            .offset(canvas_component.size);
        layers.push(Arc::new(transformed(child).translate(offset)));
    }

    Arc::new(apply_widget_style(zstack(layers), &style))
}

// ---------------------------------------------------------------------------
// Radio Group
// ---------------------------------------------------------------------------

pub(crate) fn project_radio_group(radio_group: &UiRadioGroup, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);

    let items = radio_group
        .options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let selected = i == radio_group.selected;
            let mut item_style = resolve_style_for_classes(ctx.world, ["widget.radio.item"]);
            let indicator_style = if selected {
                resolve_style_for_classes(
                    ctx.world,
                    ["widget.radio.indicator", "widget.radio.indicator.selected"],
                )
            } else {
                resolve_style_for_classes(ctx.world, ["widget.radio.indicator"])
            };
            item_style.layout.border_width = indicator_style.layout.border_width.max(1.0);
            item_style.colors.border = indicator_style.colors.border;

            let mut btn = radio_button_view(
                ctx.entity,
                WidgetUiAction::SelectRadioItem {
                    group: ctx.entity,
                    index: i,
                },
                opt.clone(),
                selected,
            )
            .text_size(item_style.text.size);

            if let Some(font_stack) = font_stack_from_style(&item_style) {
                btn = btn.font(font_stack);
            }

            if let Some(text_color) = item_style.colors.text.or(style.colors.text) {
                btn = btn.text_color(text_color);
            }
            if let Some(checkmark_color) = indicator_style.colors.text {
                btn = btn.checkmark_color(checkmark_color);
            }

            apply_direct_widget_style(btn, &item_style).into_any_flex()
        })
        .collect::<Vec<_>>();

    let group = xilem_radio_group::<(), (), _>(
        apply_flex_alignment(flex_col(items), &style).gap(Length::px(style.layout.gap.max(4.0))),
    );

    Arc::new(apply_widget_style(group, &style))
}

// ---------------------------------------------------------------------------
// Tab Bar
// ---------------------------------------------------------------------------

pub(crate) fn project_tab_bar(tab_bar: &UiTabBar, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);

    let content: UiView = ctx
        .children
        .get(tab_bar.active)
        .cloned()
        .unwrap_or_else(|| Arc::new(label("")));

    // When headers are hidden only show the active content (page-container mode).
    if !tab_bar.show_headers {
        return Arc::new(apply_widget_style(
            apply_flex_alignment(flex_col(vec![content.into_any_flex()]), &style)
                .gap(Length::px(0.0)),
            &style,
        ));
    }

    let header_style = resolve_style_for_classes(ctx.world, ["widget.tab.header"]);
    let active_style = resolve_style_for_classes(ctx.world, ["widget.tab.active"]);

    // Resolve the selected-indicator-pipe style.
    let pipe_style = resolve_style_for_classes(ctx.world, ["widget.tab.selected-pipe"]);
    let pipe_height = if pipe_style.layout.border_width > 0.0 {
        pipe_style.layout.border_width
    } else {
        0.0
    };
    let pipe_width = 28.0;

    let headers = tab_bar
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab_label)| {
            let is_active = i == tab_bar.active;
            let s = if is_active {
                &active_style
            } else {
                &header_style
            };
            let label_view = apply_label_style(label(tab_label.clone()), s);
            let styled_btn = apply_direct_widget_style(
                button_with_child_view(
                    ctx.entity,
                    WidgetUiAction::SelectTab {
                        bar: ctx.entity,
                        index: i,
                    },
                    label_view,
                ),
                s,
            );

            let mut indicator_style = pipe_style.clone();
            indicator_style.transition = Some(crate::StyleTransition {
                duration: 0.12,
                easing: None,
            });
            indicator_style.layout.scale = if is_active { 1.0 } else { 0.45 };
            indicator_style.colors.bg = pipe_style.colors.bg.map(|pipe_color| {
                if is_active {
                    pipe_color
                } else {
                    pipe_color.with_alpha(0.0)
                }
            });

            let indicator = flex_row(vec![
                apply_widget_style(
                    sized_box(label(""))
                        .width(Dim::Fixed(Length::px(pipe_width)))
                        .height(Dim::Fixed(Length::px(pipe_height))),
                    &indicator_style,
                )
                .into_any_flex(),
            ])
            .main_axis_alignment(MainAxisAlignment::Center)
            .width(Dim::Stretch);

            flex_col(vec![styled_btn.into_any_flex(), indicator.into_any_flex()])
                .gap(Length::px(0.0))
                .into_any_flex()
        })
        .collect::<Vec<_>>();

    let header_row = flex_row(headers).into_any_flex();

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(vec![header_row, content.into_any_flex()]), &style)
            .gap(Length::px(0.0)),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Tree Node
// ---------------------------------------------------------------------------

pub(crate) fn project_tree_node(tree_node: &UiTreeNode, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let depth = tree_node_depth(ctx.world, ctx.entity);
    let indent = (depth as f64) * 16.0;

    let has_children = !ctx.children.is_empty();

    let header: UiView = if has_children {
        let mut items = Vec::new();
        if let Some(icon_color) = style.colors.text {
            let icon = if tree_node.is_expanded {
                vector_icon(VectorIcon::ChevronDown, 12.0, icon_color)
            } else {
                vector_icon(VectorIcon::ChevronRight, 12.0, icon_color)
            };
            items.push(icon.into_any_flex());
        }
        items.push(apply_label_style(label(tree_node.label.clone()), &style).into_any_flex());
        let content = flex_row(items).gap(Length::px(6.0));

        let btn = button_with_child_view(
            ctx.entity,
            WidgetUiAction::ToggleTreeNode { node: ctx.entity },
            content,
        );
        Arc::new(apply_direct_widget_style(btn, &style))
    } else {
        Arc::new(apply_label_style(label(tree_node.label.clone()), &style))
    };

    let header_padded = sized_box(header).width(Dim::Stretch).into_any_flex();
    // We use padding / margin via a row with a spacer
    let spacer = sized_box(label(""))
        .width(Dim::Fixed(Length::px(indent)))
        .height(Dim::Fixed(Length::px(1.0)))
        .into_any_flex();
    let header_row = flex_row(vec![spacer, header_padded]);

    if tree_node.is_expanded && has_children {
        let children = ctx
            .children
            .into_iter()
            .map(|c| c.into_any_flex())
            .collect::<Vec<_>>();
        Arc::new(apply_widget_style(
            apply_flex_alignment(
                flex_col(vec![
                    header_row.into_any_flex(),
                    apply_flex_alignment(flex_col(children), &style).into_any_flex(),
                ]),
                &style,
            ),
            &style,
        ))
    } else {
        Arc::new(apply_widget_style(
            apply_flex_alignment(flex_col(vec![header_row.into_any_flex()]), &style),
            &style,
        ))
    }
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

pub(crate) fn project_table(table: &UiTable, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let header_style = resolve_style_for_classes(ctx.world, ["widget.table.header"]);
    let cell_style = resolve_style_for_classes(ctx.world, ["widget.table.cell"]);

    // Header row
    let header_cells = table
        .columns
        .iter()
        .map(|col| {
            apply_widget_style(
                sized_box(apply_label_style(label(col.clone()), &header_style)).width(Dim::Stretch),
                &header_style,
            )
            .flex(1.0)
            .into_any_flex()
        })
        .collect::<Vec<_>>();
    let header_row = flex_row(header_cells).into_any_flex();

    // Data rows
    let data_rows = table
        .rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let row_style = cell_style.clone();
            let _ = row_idx;
            let cells = row
                .iter()
                .map(|cell| {
                    apply_widget_style(
                        sized_box(apply_label_style(label(cell.clone()), &cell_style))
                            .width(Dim::Stretch),
                        &row_style,
                    )
                    .flex(1.0)
                    .into_any_flex()
                })
                .collect::<Vec<_>>();
            flex_row(cells).into_any_flex()
        })
        .collect::<Vec<_>>();

    let mut all_rows = vec![header_row];
    all_rows.extend(data_rows);

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(all_rows), &style).gap(Length::px(style.layout.gap.max(1.0))),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// List View
// ---------------------------------------------------------------------------

pub(crate) fn project_list_view(list_view: &UiListView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let mut item_style = default_item_style(ctx.world, "widget.list_view.item");
    apply_optional_item_padding(&mut item_style, list_view.item_padding);
    if item_style.colors.text.is_none() {
        item_style.colors.text = style.colors.text;
    }
    let mut selected_style = selected_row_style(
        ctx.world,
        "widget.list_view.item",
        "widget.list_view.item.selected",
    );
    apply_optional_item_padding(&mut selected_style, list_view.item_padding);
    let selected_indices = list_view.clamped_selected_indices();

    if list_view.items.is_empty() {
        let empty_text = list_view.empty_text.clone().unwrap_or_default();
        return Arc::new(apply_widget_style(
            apply_flex_alignment(
                flex_col(vec![
                    apply_label_style(label(empty_text), &item_style).into_any_flex(),
                ]),
                &style,
            ),
            &style,
        ));
    }

    let rows = list_view
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let row_style = if selected_indices.contains(&index) {
                &selected_style
            } else {
                &item_style
            };
            let label_view = apply_label_style(label(item.clone()), row_style);

            let row_view: UiView = if matches!(list_view.selection_mode, UiListSelectionMode::None)
            {
                Arc::new(apply_widget_style(
                    sized_box(label_view).width(Dim::Stretch),
                    row_style,
                ))
            } else {
                Arc::new(apply_direct_widget_style(
                    button_with_child_view(
                        ctx.entity,
                        WidgetUiAction::SelectListItem {
                            list_view: ctx.entity,
                            index,
                        },
                        label_view,
                    ),
                    row_style,
                ))
            };

            if let Some(height) = list_view.item_height {
                sized_box(row_view)
                    .height(Dim::Fixed(Length::px(height)))
                    .into_any_flex()
            } else {
                row_view.into_any_flex()
            }
        })
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(rows), &style).gap(Length::px(style.layout.gap.max(1.0))),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Data Table
// ---------------------------------------------------------------------------

pub(crate) fn project_data_table(table: &UiDataTable, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let mut header_style = resolve_style_for_classes(ctx.world, ["widget.data_table.header"]);
    if header_style.colors.text.is_none() {
        header_style.colors.text = style.colors.text;
    }

    let mut cell_style = default_item_style(ctx.world, "widget.data_table.cell");
    if cell_style.colors.text.is_none() {
        cell_style.colors.text = style.colors.text;
    }

    let row_style = resolve_style_for_classes(ctx.world, ["widget.data_table.row"]);
    let mut striped_style = row_style.clone();
    let striped_overrides = resolve_style_for_classes(ctx.world, ["widget.data_table.row.striped"]);
    apply_color_overrides(&mut striped_style, &striped_overrides);
    let selected_style = selected_row_style(
        ctx.world,
        "widget.data_table.row",
        "widget.data_table.row.selected",
    );
    let selected_rows = table.clamped_selected_rows();

    let column_count = table
        .columns
        .len()
        .max(
            table
                .rows
                .iter()
                .map(|row| row.cells.len())
                .max()
                .unwrap_or(0),
        )
        .max(1);

    let header_cells = (0..column_count)
        .map(|index| {
            let column = table.columns.get(index);
            let mut text = column
                .map(|column| column.label.clone())
                .unwrap_or_default();
            if let Some(sort) = table.sort.filter(|sort| sort.column == index) {
                text.push_str(match sort.direction {
                    UiSortDirection::Ascending => " ^",
                    UiSortDirection::Descending => " v",
                });
            }
            let content = apply_widget_style(
                sized_box(apply_label_style(label(text), &header_style)).width(Dim::Stretch),
                &header_style,
            );
            let cell: UiView = if column.is_some_and(|column| column.sortable) {
                Arc::new(apply_direct_widget_style(
                    button_with_child_view(
                        ctx.entity,
                        WidgetUiAction::SortDataTableColumn {
                            table: ctx.entity,
                            column: index,
                        },
                        content,
                    ),
                    &header_style,
                ))
            } else {
                Arc::new(content)
            };
            if let Some(width) = column.and_then(|column| column.width) {
                sized_box(cell)
                    .width(Dim::Fixed(Length::px(width)))
                    .into_any_flex()
            } else {
                cell.flex(1.0).into_any_flex()
            }
        })
        .collect::<Vec<_>>();
    let header_row = table
        .show_header
        .then(|| flex_row(header_cells).into_any_flex());

    let data_rows = table
        .sorted_row_indices()
        .into_iter()
        .enumerate()
        .map(|(display_index, row_index)| {
            let row = &table.rows[row_index];
            let active_row_style = if selected_rows.contains(&row_index) {
                &selected_style
            } else if table.striped && display_index % 2 == 1 {
                &striped_style
            } else {
                &row_style
            };
            let cells = (0..column_count)
                .map(|column_index| {
                    let cell_content = row.cells.get(column_index).cloned().unwrap_or_default();
                    let cell: UiView = match cell_content {
                        crate::ecs::UiDataCell::Text(text) => Arc::new(apply_widget_style(
                            sized_box(apply_label_style(label(text), &cell_style))
                                .width(Dim::Stretch),
                            &cell_style,
                        )),
                        crate::ecs::UiDataCell::Image(image) => {
                            if let Some(brush) = image.image_brush() {
                                let mut image_view =
                                    xilem_image(brush).decorative(image.decorative);
                                if let Some(alt) = &image.alt_text {
                                    image_view = image_view.alt_text(alt.clone());
                                }
                                Arc::new(apply_widget_style(
                                    sized_box(image_view.fit(image.fit)).width(Dim::Stretch),
                                    &cell_style,
                                ))
                            } else {
                                let fallback = image.alt_text.clone().unwrap_or_default();
                                Arc::new(apply_widget_style(
                                    sized_box(apply_label_style(label(fallback), &cell_style))
                                        .width(Dim::Stretch),
                                    &cell_style,
                                ))
                            }
                        }
                    };
                    if let Some(width) = table
                        .columns
                        .get(column_index)
                        .and_then(|column| column.width)
                    {
                        sized_box(cell)
                            .width(Dim::Fixed(Length::px(width)))
                            .into_any_flex()
                    } else {
                        cell.flex(1.0).into_any_flex()
                    }
                })
                .collect::<Vec<_>>();
            let content = flex_row(cells);
            let row_view = if matches!(table.selection_mode, UiListSelectionMode::None) {
                let row_view: UiView = Arc::new(apply_widget_style(content, active_row_style));
                row_view
            } else {
                let row_view: UiView = Arc::new(apply_direct_widget_style(
                    button_with_child_view(
                        ctx.entity,
                        WidgetUiAction::SelectDataTableRow {
                            table: ctx.entity,
                            row: row_index,
                        },
                        content,
                    ),
                    active_row_style,
                ));
                row_view
            };

            if let Some(height) = table.row_height {
                sized_box(row_view)
                    .height(Dim::Fixed(Length::px(height)))
                    .into_any_flex()
            } else {
                row_view.into_any_flex()
            }
        })
        .collect::<Vec<_>>();

    let mut all_rows = header_row.into_iter().collect::<Vec<_>>();
    all_rows.extend(data_rows);

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(all_rows), &style).gap(Length::px(style.layout.gap.max(1.0))),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Menu Bar
// ---------------------------------------------------------------------------

pub(crate) fn project_menu_bar(_: &UiMenuBar, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|c| c.into_any_flex())
        .collect::<Vec<_>>();
    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_row(children), &style).gap(Length::px(style.layout.gap.max(0.0))),
        &style,
    ))
}

pub(crate) fn project_menu_bar_item(item: &UiMenuBarItem, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let mut items = vec![apply_label_style(label(item.label.clone()), &style).into_any_flex()];
    if let Some(icon_color) = style.colors.text {
        let icon = if item.is_open {
            vector_icon(VectorIcon::ChevronUp, 10.0, icon_color)
        } else {
            vector_icon(VectorIcon::ChevronDown, 10.0, icon_color)
        };
        items.push(icon.into_any_flex());
    }
    let content = flex_row(items).gap(Length::px(4.0));
    Arc::new(apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleMenuBarItem, content),
        &style,
    ))
}

pub(crate) fn project_menu_item_panel(_: &UiMenuItemPanel, ctx: ProjectionCtx<'_>) -> UiView {
    let anchor = ctx.world.get::<AnchoredTo>(ctx.entity).map(|a| a.0);

    let pos = match overlay_position(ctx.world, ctx.entity) {
        Some(p) => p,
        None => return hidden_placeholder(),
    };

    let menu_style = default_panel_style(ctx.world, "overlay.dropdown.menu");
    let item_style = default_item_style(ctx.world, "overlay.dropdown.item");

    let items: Vec<_> = anchor
        .and_then(|a| ctx.world.get::<UiMenuBarItem>(a))
        .map(|bar_item| {
            bar_item
                .items
                .iter()
                .enumerate()
                .map(|(i, menu_item)| {
                    apply_direct_widget_style(
                        button_with_child_view(
                            ctx.entity,
                            OverlayUiAction::SelectMenuBarItem { index: i },
                            apply_label_style(label(menu_item.label.clone()), &item_style),
                        ),
                        &item_style,
                    )
                    .into_any_flex()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let computed_pos = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let panel_width = if computed_pos.width > 1.0 {
        computed_pos.width
    } else {
        160.0
    };
    let panel_height = if computed_pos.height > 1.0 {
        computed_pos.height
    } else {
        items.len() as f64 * 32.0 + 16.0
    };

    let panel_content = flex_col(items).gap(Length::px(menu_style.layout.gap.max(4.0)));

    let scrollable = crate::xilem::view::portal(panel_content)
        .dims((Length::px(panel_width), Length::px(panel_height)));

    Arc::new(
        transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity,
            apply_widget_style(scrollable, &menu_style),
        ))
        .translate(pos),
    )
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

pub(crate) fn project_tooltip(tooltip: &UiTooltip, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = default_panel_style(ctx.world, "overlay.tooltip");

    let computed_pos = popover_geometry(ctx.world, ctx.entity, (96.0, 28.0), &mut [&mut style]);

    let text_lbl = apply_label_style(label(tooltip.text.clone()), &style);
    let panel = apply_widget_style(
        sized_box(text_lbl).width(Dim::Fixed(Length::px(computed_pos.width))),
        &style,
    );

    Arc::new(
        transformed(opaque_hitbox_for_entity(ctx.entity, panel))
            .translate((computed_pos.x, computed_pos.y)),
    )
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

pub(crate) fn project_spinner(sp: &UiSpinner, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let spin_view: UiView = if let Some(color) = style.colors.text {
        Arc::new(spinner().color(color))
    } else {
        Arc::new(spinner())
    };

    if let Some(lbl) = &sp.label {
        let label_view = apply_label_style(label(lbl.clone()), &style);
        Arc::new(apply_widget_style(
            apply_flex_alignment(
                flex_row(vec![spin_view.into_any_flex(), label_view.into_any_flex()]),
                &style,
            )
            .gap(Length::px(8.0)),
            &style,
        ))
    } else {
        Arc::new(apply_widget_style(
            apply_flex_alignment(flex_row(vec![spin_view.into_any_flex()]), &style),
            &style,
        ))
    }
}

// ---------------------------------------------------------------------------
// Color Picker (WinUI-inspired vertical layout)
// ---------------------------------------------------------------------------

/// Spectrum pad size (WinUI vertical mode uses a large box; keep flyout-friendly).
const SPECTRUM_SIZE: f64 = 220.0;
/// Natural slider height room for thumb + track (do not squash to 12px).
const SLIDER_ROW_HEIGHT: f64 = 28.0;
const PREVIEW_SIZE: f64 = 36.0;
const CHANNEL_INPUT_WIDTH: f64 = 72.0;
const PANEL_CONTENT_WIDTH: f64 = SPECTRUM_SIZE;

pub(crate) fn project_color_picker(picker: &UiColorPicker, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let mut preview_style =
        resolve_style_for_classes(ctx.world, ["overlay.color_picker.preview"]);
    // Composite over a neutral checker-like mid gray when translucent.
    let display_a = if picker.alpha_enabled {
        picker.a
    } else {
        255
    };
    preview_style.colors.bg = Some(Color::from_rgba8(picker.r, picker.g, picker.b, display_a));
    let preview = apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(18.0)))
            .height(Dim::Fixed(Length::px(18.0))),
        &preview_style,
    );
    let hex = picker.hex_string();
    let mut items = vec![
        preview.into_any_flex(),
        apply_label_style(label(hex), &style).into_any_flex(),
    ];
    if let Some(icon_color) = style.colors.text {
        let icon = if picker.is_open {
            vector_icon(VectorIcon::ChevronUp, 10.0, icon_color)
        } else {
            vector_icon(VectorIcon::ChevronDown, 10.0, icon_color)
        };
        items.push(icon.into_any_flex());
    }
    let content = flex_row(items).gap(Length::px(6.0));
    Arc::new(apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleColorPicker, content),
        &style,
    ))
}

fn color_picker_channel_row(
    entity: Entity,
    world: &bevy_ecs::world::World,
    channel: crate::ColorPickerChannel,
    value_text: String,
    label_text: &str,
    placeholder: &str,
) -> picus_view::view::AnyFlexChild<(), ()> {
    let label_style =
        resolve_style_for_classes(world, ["overlay.color_picker.channel_label"]);
    let input_style = resolve_style_for_classes(world, ["overlay.color_picker.value"]);
    let channel_for_change = channel;
    let channel_for_enter = channel;
    let input = apply_direct_text_input_style(
        text_input_view(entity, value_text, move |text| {
            OverlayUiAction::SetColorChannel {
                channel: channel_for_change,
                text,
            }
        })
        .on_enter(move |text| {
            crate::events::emit_ui_action(
                entity,
                OverlayUiAction::SetColorChannel {
                    channel: channel_for_enter,
                    text,
                },
            );
        })
        .placeholder(placeholder),
        &input_style,
    );
    let input_box = sized_box(input)
        .width(Dim::Fixed(Length::px(CHANNEL_INPUT_WIDTH)))
        .height(Dim::Fixed(Length::px(28.0)));
    flex_row(vec![
        input_box.into_any_flex(),
        apply_label_style(label(label_text.to_string()), &label_style).into_any_flex(),
    ])
    .gap(Length::px(8.0))
    .into_any_flex()
}

pub(crate) fn project_color_picker_panel(
    panel: &UiColorPickerPanel,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    let pos = match overlay_position(ctx.world, ctx.entity) {
        Some(p) => p,
        None => return hidden_placeholder(),
    };

    let panel_style = default_panel_style(ctx.world, "overlay.color_picker.panel");

    let picker = ctx
        .world
        .get::<UiColorPicker>(panel.anchor)
        .copied()
        .unwrap_or_default();
    let cur_r = picker.r;
    let cur_g = picker.g;
    let cur_b = picker.b;
    let cur_a = picker.a;
    let alpha_enabled = picker.alpha_enabled;

    let (cur_h, cur_s, cur_v) = crate::rgb_to_hsv(cur_r, cur_g, cur_b);
    let cur_h_deg = cur_h as f64;
    let cur_s_f64 = cur_s as f64;
    let cur_v_f64 = cur_v as f64;
    let cur_alpha_f = cur_a as f64 / 255.0;

    // --- SV spectrum (2D draggable pad) ---
    let sv_spectrum = color_spectrum_view(
        ctx.entity,
        cur_h_deg,
        cur_s_f64,
        cur_v_f64,
        |s, v| OverlayUiAction::SelectColorSv { s, v },
    );
    let sv_area = Arc::new(
        sized_box(sv_spectrum)
            .width(Dim::Fixed(Length::px(SPECTRUM_SIZE)))
            .height(Dim::Fixed(Length::px(SPECTRUM_SIZE))),
    );

    // --- Hue slider (third dimension; WinUI ColorPickerSlider below spectrum) ---
    let hue_slider = slider_view(
        ctx.entity,
        0.0,
        360.0,
        cur_h_deg,
        |h| OverlayUiAction::SelectColorHue { h },
    )
    .step(1.0);
    let hue_area = Arc::new(
        sized_box(hue_slider)
            .width(Dim::Fixed(Length::px(SPECTRUM_SIZE)))
            .height(Dim::Fixed(Length::px(SLIDER_ROW_HEIGHT))),
    );

    // --- Alpha / opacity slider (WinUI AlphaSlider) ---
    let alpha_area = if alpha_enabled {
        let alpha_slider = slider_view(
            ctx.entity,
            0.0,
            1.0,
            cur_alpha_f,
            |a| OverlayUiAction::SelectColorAlpha { a },
        )
        .step(0.01);
        Some(Arc::new(
            sized_box(alpha_slider)
                .width(Dim::Fixed(Length::px(SPECTRUM_SIZE)))
                .height(Dim::Fixed(Length::px(SLIDER_ROW_HEIGHT))),
        ))
    } else {
        None
    };

    // --- Color preview + hex input (always visible, WinUI HexTextBox) ---
    let mut preview_style =
        resolve_style_for_classes(ctx.world, ["overlay.color_picker.preview"]);
    preview_style.colors.bg = Some(Color::from_rgba8(
        cur_r,
        cur_g,
        cur_b,
        if alpha_enabled { cur_a } else { 255 },
    ));
    let preview_box = sized_box(label(""))
        .width(Dim::Fixed(Length::px(PREVIEW_SIZE)))
        .height(Dim::Fixed(Length::px(PREVIEW_SIZE)));
    let preview_styled = apply_widget_style(preview_box, &preview_style);

    let current_hex = crate::format_color_hex(cur_r, cur_g, cur_b, cur_a, alpha_enabled);
    let hex_placeholder = if alpha_enabled {
        "#AARRGGBB"
    } else {
        "#RRGGBB"
    };
    let hex_input_style =
        resolve_style_for_classes(ctx.world, ["overlay.color_picker.value"]);
    let hex_input = apply_direct_text_input_style(
        text_input_view(
            ctx.entity,
            current_hex,
            |text| OverlayUiAction::SetHexColor { hex: text },
        )
        .on_enter(move |text| {
            crate::events::emit_ui_action(
                ctx.entity,
                OverlayUiAction::SetHexColor { hex: text },
            );
        })
        .placeholder(hex_placeholder),
        &hex_input_style,
    );
    let hex_box = sized_box(hex_input)
        .width(Dim::Fixed(Length::px(SPECTRUM_SIZE - PREVIEW_SIZE - 8.0)))
        .height(Dim::Fixed(Length::px(PREVIEW_SIZE)));
    let preview_row = flex_row(vec![
        preview_styled.into_any_flex(),
        hex_box.into_any_flex(),
    ])
    .gap(Length::px(8.0));

    // --- RGB (+ Opacity) channel text inputs (WinUI ColorChannelTextInput) ---
    let mut channel_rows = vec![
        color_picker_channel_row(
            ctx.entity,
            ctx.world,
            crate::ColorPickerChannel::Red,
            cur_r.to_string(),
            "Red",
            "0–255",
        ),
        color_picker_channel_row(
            ctx.entity,
            ctx.world,
            crate::ColorPickerChannel::Green,
            cur_g.to_string(),
            "Green",
            "0–255",
        ),
        color_picker_channel_row(
            ctx.entity,
            ctx.world,
            crate::ColorPickerChannel::Blue,
            cur_b.to_string(),
            "Blue",
            "0–255",
        ),
    ];
    if alpha_enabled {
        channel_rows.push(color_picker_channel_row(
            ctx.entity,
            ctx.world,
            crate::ColorPickerChannel::Alpha,
            format!("{}%", picker.opacity_percent()),
            "Opacity",
            "100%",
        ));
    }
    let channels = flex_col(channel_rows).gap(Length::px(8.0));

    // --- Assemble panel (vertical WinUI orientation) ---
    let mut panel_items = vec![
        sv_area.into_any_flex(),
        hue_area.into_any_flex(),
    ];
    if let Some(alpha) = alpha_area {
        panel_items.push(alpha.into_any_flex());
    }
    panel_items.push(preview_row.into_any_flex());
    panel_items.push(channels.into_any_flex());
    let content = flex_col(panel_items).gap(Length::px(10.0));

    let computed_pos = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let panel_width = if computed_pos.width > 1.0 {
        computed_pos.width
    } else {
        PANEL_CONTENT_WIDTH + 24.0
    };
    // Spectrum + hue + optional alpha + preview + 3–4 channel rows + gaps/padding.
    let channel_count = if alpha_enabled { 4.0 } else { 3.0 };
    let estimated_height = SPECTRUM_SIZE
        + SLIDER_ROW_HEIGHT
        + if alpha_enabled {
            SLIDER_ROW_HEIGHT + 10.0
        } else {
            0.0
        }
        + PREVIEW_SIZE
        + channel_count * 36.0
        + 80.0;
    let panel_height = if computed_pos.height > 1.0 {
        computed_pos.height
    } else {
        estimated_height
    };

    let panel_view = apply_widget_style(
        crate::xilem::view::portal(content)
            .dims((Length::px(panel_width), Length::px(panel_height))),
        &panel_style,
    );

    Arc::new(
        transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity, panel_view,
        ))
        .translate(pos),
    )
}

// ---------------------------------------------------------------------------
// Group Box
// ---------------------------------------------------------------------------

pub(crate) fn project_group_box(group_box: &UiGroupBox, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);

    let mut title_style = resolve_style_for_classes(ctx.world, ["widget.group_box.title"]);
    if title_style.colors.text.is_none() {
        title_style.colors.text = style.colors.text;
    }
    let title_view = apply_label_style(label(group_box.title.clone()), &title_style);

    let mut content_items = vec![title_view.into_any_flex()];
    content_items.extend(ctx.children.into_iter().map(|c| c.into_any_flex()));

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(content_items), &style).gap(Length::px(style.layout.gap)),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Split Pane
// ---------------------------------------------------------------------------

pub(crate) fn project_split_pane(pane: &UiSplitPane, ctx: ProjectionCtx<'_>) -> UiView {
    let fallback: UiView = Arc::new(label(""));
    let child1 = ctx
        .children
        .first()
        .cloned()
        .unwrap_or_else(|| fallback.clone());
    let child2 = ctx
        .children
        .get(1)
        .cloned()
        .unwrap_or_else(|| fallback.clone());

    let axis = match pane.direction {
        SplitDirection::Horizontal => Axis::Horizontal,
        SplitDirection::Vertical => Axis::Vertical,
    };

    Arc::new(
        split(child1, child2)
            .split_axis(axis)
            .split_point(pane.ratio as f64)
            .draggable(true)
            .solid_bar(false),
    )
}

// ---------------------------------------------------------------------------
// Toast
// ---------------------------------------------------------------------------

pub(crate) fn project_toast(toast: &UiToast, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = default_panel_style(ctx.world, "overlay.toast");
    let intent_style = match toast.kind {
        ToastKind::Info => resolve_style_for_classes(ctx.world, ["overlay.toast.info"]),
        ToastKind::Success => resolve_style_for_classes(ctx.world, ["overlay.toast.success"]),
        ToastKind::Warning => resolve_style_for_classes(ctx.world, ["overlay.toast.warning"]),
        ToastKind::Error => resolve_style_for_classes(ctx.world, ["overlay.toast.error"]),
    };
    let mut intent_color = intent_style
        .colors
        .text
        .or(intent_style.colors.border)
        .or(style.colors.border)
        .or(style.colors.text)
        .unwrap_or(Color::TRANSPARENT);

    let mut dismiss_style = resolve_style_for_classes(ctx.world, ["overlay.toast.dismiss"]);
    if dismiss_style.colors.text.is_none() {
        dismiss_style.colors.text = style.colors.text;
    }

    let computed_pos = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let pos = (computed_pos.x, computed_pos.y);

    if !computed_pos.is_positioned {
        hide_style_without_collapsing_layout(&mut style);
        hide_style_without_collapsing_layout(&mut dismiss_style);
        intent_color = Color::TRANSPARENT;
    }

    let toast_width = if computed_pos.width > 1.0 {
        computed_pos.width
    } else {
        toast.min_width.max(180.0)
    };
    let outer_inset = style.layout.padding * 2.0 + style.layout.border_width * 2.0;
    let content_width = (toast_width - outer_inset).max(120.0);

    let intent_icon = match toast.kind {
        ToastKind::Info | ToastKind::Warning => VectorIcon::Info,
        ToastKind::Success => VectorIcon::Check,
        ToastKind::Error => VectorIcon::X,
    };
    let icon = vector_icon(intent_icon, 16.0, intent_color);

    let mut stripe_style = ResolvedStyle::default();
    stripe_style.colors.bg = Some(intent_color);
    stripe_style.layout.corner_radius = style.layout.corner_radius.min(3.0);
    let stripe: UiView = Arc::new(apply_widget_style(
        sized_box(label("")).dims(
            Dimensions::AUTO
                .with_width(Dim::Fixed(Length::px(4.0)))
                .with_height(Dim::Stretch),
        ),
        &stripe_style,
    ));

    let msg = apply_label_style(label(toast.message.clone()), &style);
    let mut items = vec![
        stripe.into_any_flex(),
        icon.into_any_flex(),
        msg.flex(1.0).into_any_flex(),
    ];
    if toast.show_close_button {
        let dismiss_icon = match dismiss_style.colors.text {
            Some(text_color) => vector_icon(VectorIcon::X, 14.0, text_color),
            None => Arc::new(label("")),
        };
        let dismiss = sized_box(apply_direct_widget_style(
            button_with_child_view(ctx.entity, OverlayUiAction::DismissToast, dismiss_icon),
            &dismiss_style,
        ))
        .dims(
            Dimensions::AUTO
                .with_width(Dim::Fixed(Length::px(28.0)))
                .with_height(Dim::Fixed(Length::px(28.0))),
        );
        items.push(dismiss.into_any_flex());
    }

    let panel = apply_widget_style(
        sized_box(
            flex_row(items)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(Length::px(style.layout.gap.max(8.0))),
        )
        .width(Dim::Fixed(Length::px(content_width))),
        &style,
    );

    Arc::new(transformed(opaque_hitbox_for_entity(ctx.entity, panel)).translate(pos))
}

// ---------------------------------------------------------------------------
// Date Picker
// ---------------------------------------------------------------------------

pub(crate) fn project_date_picker(picker: &UiDatePicker, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let date_str = format!("{} {}, {}", month_name(picker.month), picker.day, picker.year);
    let mut items = vec![apply_label_style(label(date_str), &style).into_any_flex()];
    if let Some(icon_color) = style.colors.text {
        let icon = if picker.is_open {
            vector_icon(VectorIcon::ChevronUp, 10.0, icon_color)
        } else {
            vector_icon(VectorIcon::ChevronDown, 10.0, icon_color)
        };
        items.push(icon.into_any_flex());
    }
    let content = flex_row(items).gap(Length::px(6.0));
    Arc::new(apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleDatePicker, content),
        &style,
    ))
}

pub(crate) fn project_date_picker_panel(
    panel_comp: &UiDatePickerPanel,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    let pos = match overlay_position(ctx.world, ctx.entity) {
        Some(p) => p,
        None => return hidden_placeholder(),
    };

    let panel_style = default_panel_style(ctx.world, "overlay.date_picker.panel");
    let cell_style = resolve_style_for_classes(ctx.world, ["overlay.date_picker.cell"]);
    let mut today_style = cell_style.clone();
    let today_overrides = resolve_style_for_classes(ctx.world, ["overlay.date_picker.cell.today"]);
    apply_color_overrides(&mut today_style, &today_overrides);
    let mut selected_style = cell_style.clone();
    let selected_overrides =
        resolve_style_for_classes(ctx.world, ["overlay.date_picker.cell.selected"]);
    apply_color_overrides(&mut selected_style, &selected_overrides);

    let view_year = panel_comp.view_year;
    let view_month = panel_comp.view_month;
    let selected_day = ctx
        .world
        .get::<UiDatePicker>(panel_comp.anchor)
        .and_then(|dp| {
            if dp.year == view_year && dp.month == view_month {
                Some(dp.day)
            } else {
                None
            }
        });

    // Navigation row
    let nav_style = resolve_style_for_classes(ctx.world, ["overlay.date_picker.nav"]);
    let nav_icon_color = nav_style.colors.text.unwrap_or(crate::xilem::Color::BLACK);
    let prev_icon = vector_icon(VectorIcon::ChevronLeft, DATE_PICKER_NAV_HEIGHT, nav_icon_color);
    let next_icon = vector_icon(VectorIcon::ChevronRight, DATE_PICKER_NAV_HEIGHT, nav_icon_color);
    let prev_btn = button_with_child_view(
        ctx.entity,
        OverlayUiAction::NavigateDateMonth { forward: false },
        prev_icon,
    );
    let next_btn = button_with_child_view(
        ctx.entity,
        OverlayUiAction::NavigateDateMonth { forward: true },
        next_icon,
    );
    let month_lbl = apply_label_style(
        label(format!("{} {view_year}", month_name(view_month))),
        &nav_style,
    );
    let nav_row = flex_row(vec![
        sized_box(apply_direct_widget_style(prev_btn, &cell_style))
            .dims((
                Length::px(DATE_PICKER_NAV_HEIGHT),
                Length::px(DATE_PICKER_NAV_HEIGHT),
            ))
            .into_any_flex(),
        sized_box(month_lbl)
            .dims((
                Length::px(
                    DATE_PICKER_CELL_SIZE * 5.0 + DATE_PICKER_GRID_GAP * 4.0,
                ),
                Length::px(DATE_PICKER_NAV_HEIGHT),
            ))
            .into_any_flex(),
        sized_box(apply_direct_widget_style(next_btn, &cell_style))
            .dims((
                Length::px(DATE_PICKER_NAV_HEIGHT),
                Length::px(DATE_PICKER_NAV_HEIGHT),
            ))
            .into_any_flex(),
    ])
    .gap(Length::px(DATE_PICKER_GRID_GAP));

    // Day-of-week headers
    let dow_labels = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].map(|d| {
        sized_box(apply_label_style(label(d), &cell_style))
            .dims((
                Length::px(DATE_PICKER_CELL_SIZE),
                Length::px(DATE_PICKER_DOW_HEIGHT),
            ))
            .into_any_flex()
    });
    let dow_row = flex_row(dow_labels.into_iter().collect::<Vec<_>>())
        .gap(Length::px(DATE_PICKER_GRID_GAP));

    // Calendar grid
    let first_dow = day_of_week_for_first(view_year, view_month) as usize;
    let num_days = days_in_month(view_year, view_month) as usize;
    let total_cells = first_dow + num_days;
    let num_rows = total_cells.div_ceil(7);

    let mut week_rows = Vec::new();
    for week in 0..num_rows {
        let mut week_cells = Vec::new();
        for dow in 0..7 {
            let cell_index = week * 7 + dow;
            let day_num = if cell_index < first_dow {
                None
            } else {
                let d = cell_index - first_dow + 1;
                if d <= num_days { Some(d as u32) } else { None }
            };

            let cell: UiView = if let Some(day) = day_num {
                let s = if Some(day) == selected_day {
                    &selected_style
                } else {
                    &cell_style
                };
                let day_label: UiView = Arc::new(apply_label_style(label(day.to_string()), s));
                let btn = button_with_child_view(
                    ctx.entity,
                    OverlayUiAction::SelectDateDay { day },
                    day_label,
                );
                Arc::new(sized_box(apply_direct_widget_style(btn, s)).dims((
                    Length::px(DATE_PICKER_CELL_SIZE),
                    Length::px(DATE_PICKER_CELL_SIZE),
                )))
            } else {
                Arc::new(sized_box(apply_label_style(label(""), &cell_style)).dims((
                    Length::px(DATE_PICKER_CELL_SIZE),
                    Length::px(DATE_PICKER_CELL_SIZE),
                )))
            };
            week_cells.push(cell.into_any_flex());
        }
        week_rows.push(
            flex_row(week_cells)
                .gap(Length::px(DATE_PICKER_GRID_GAP))
                .into_any_flex(),
        );
    }

    let mut all_rows = vec![nav_row.into_any_flex(), dow_row.into_any_flex()];
    all_rows.extend(week_rows);

    let content = flex_col(all_rows).gap(Length::px(DATE_PICKER_ROW_GAP));
    let panel_width = DATE_PICKER_CELL_SIZE * 7.0 + DATE_PICKER_GRID_GAP * 6.0;
    let panel_height = DATE_PICKER_NAV_HEIGHT
        + DATE_PICKER_DOW_HEIGHT
        + (num_rows as f64 * DATE_PICKER_CELL_SIZE)
        + ((num_rows + 1) as f64 * DATE_PICKER_ROW_GAP);

    let panel_view = apply_widget_style(
        crate::xilem::view::portal(content)
            .dims((Length::px(panel_width), Length::px(panel_height))),
        &panel_style,
    );

    Arc::new(
        transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity, panel_view,
        ))
        .translate(pos),
    )
}

// ---------------------------------------------------------------------------
// Divider
// ---------------------------------------------------------------------------

/// Project a `UiDivider` component into a rendered divider line.
pub(crate) fn project_divider(div: &UiDivider, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let view: UiView = match div.axis {
        Axis::Horizontal => Arc::new(divider_h::<(), ()>()),
        Axis::Vertical => Arc::new(divider_v::<(), ()>()),
    };
    Arc::new(apply_widget_style(view, &style))
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

/// Project a `UiToolbar` marker as a horizontal flex row with toolbar styling.
pub(crate) fn project_toolbar(ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children: Vec<_> = ctx
        .children
        .into_iter()
        .map(|c| c.into_any_flex())
        .collect();
    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_row(children), &style),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

/// Project a `UiCard` marker as a vertical flex container with card styling.
pub(crate) fn project_card(ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children: Vec<_> = ctx
        .children
        .into_iter()
        .map(|c| c.into_any_flex())
        .collect();
    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(children), &style),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Breadcrumb
// ---------------------------------------------------------------------------

/// Project a `UiBreadcrumb` container: renders children as a horizontal list
/// with chevron separators between items. The last item is styled as the
/// current page (plain text).
pub(crate) fn project_breadcrumb(ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let child_views: Vec<UiView> = ctx.children.into_iter().collect();

    if child_views.is_empty() {
        let empty: UiView = Arc::new(label(""));
        return Arc::new(apply_widget_style(empty, &style));
    }

    // Build a row with chevron separators between each breadcrumb item.
    let mut items: Vec<picus_view::view::AnyFlexChild<(), ()>> = Vec::new();
    for (i, child) in child_views.iter().enumerate() {
        if i > 0 {
            let separator_style =
                resolve_style_for_classes(ctx.world, ["widget.breadcrumb.separator"]);
            let chevron: UiView =
                Arc::new(apply_label_style(label(" \u{203A} "), &separator_style));
            items.push(chevron.into_any_flex());
        }
        items.push(child.clone().into_any_flex());
    }

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_row(items), &style),
        &style,
    ))
}

/// Project a `UiBreadcrumbItem` as a clickable/interactive label segment.
pub(crate) fn project_breadcrumb_item(item: &UiBreadcrumbItem, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let label_view = apply_label_style(label(item.label.clone()), &style);
    Arc::new(apply_widget_style(label_view, &style))
}

// ---------------------------------------------------------------------------
// Message Bar
// ---------------------------------------------------------------------------

/// Project a `UiMessageBar` as a coloured banner with severity-based styling.
pub(crate) fn project_message_bar(bar: &UiMessageBar, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);

    // Merge in class-based styling for the severity kind.
    let kind_class = match bar.kind {
        MessageBarKind::Info => "overlay.toast.info",
        MessageBarKind::Success => "overlay.toast.success",
        MessageBarKind::Warning => "overlay.toast.warning",
        MessageBarKind::Error => "overlay.toast.error",
    };
    let kind_style = resolve_style_for_classes(ctx.world, [kind_class]);
    if let Some(bg) = kind_style.colors.bg {
        style.colors.bg = Some(bg);
    }
    if let Some(border) = kind_style.colors.border {
        style.colors.border = Some(border);
    }
    if let Some(text) = kind_style.colors.text {
        style.colors.text = Some(text);
    }
    let message_label: UiView = Arc::new(apply_label_style(label(bar.message.clone()), &style));

    let mut row_children: Vec<UiView> = Vec::new();
    row_children.push(message_label);

    if bar.dismissible {
        let mut dismiss_style = style.clone();
        if let Some(text) = style.colors.text {
            dismiss_style.colors.text = Some(text.with_alpha(0.6));
        }
        let dismiss: UiView = Arc::new(apply_label_style(label(" \u{00D7} "), &dismiss_style));
        row_children.push(dismiss);
    }

    Arc::new(apply_widget_style(
        apply_flex_alignment(
            flex_row(
                row_children
                    .into_iter()
                    .map(|v| v.into_any_flex())
                    .collect::<Vec<_>>(),
            ),
            &style,
        ),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Project a `UiSearch` as a text input with a leading search icon.
pub(crate) fn project_search(search: &UiSearch, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);

    let icon_color = style
        .colors
        .text
        .map_or(Color::TRANSPARENT, |color| color.with_alpha(0.72));
    let icon: UiView = Arc::new(icon(FluentIcon::Search, 16.0, icon_color));

    let entity = ctx.entity;
    let mut input_style = style.clone();
    input_style.layout.padding = 0.0;
    input_style.layout.border_width = 0.0;
    input_style.layout.corner_radius = 0.0;
    input_style.colors.bg = Some(Color::TRANSPARENT);
    input_style.colors.border = Some(Color::TRANSPARENT);
    input_style.box_shadow = None;

    let input = text_input_view(entity, search.value.clone(), move |value| {
        WidgetUiAction::SetSearch {
            search: entity,
            value,
        }
    })
    .placeholder(search.placeholder.clone())
    .clip(true)
    .insert_newline(InsertNewline::Never);

    let input: UiView = Arc::new(
        sized_box(apply_direct_text_input_style(input, &input_style)).width(Dim::Stretch),
    );

    let row = flex_row(vec![
        icon.into_any_flex(),
        flex_item(input, 1.0).into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(Length::px(style.layout.gap))
    .width(Dim::Stretch);

    Arc::new(apply_widget_style(row, &style))
}

// ---------------------------------------------------------------------------
// Navigation View
// ---------------------------------------------------------------------------

const NAV_INDICATOR_WIDTH: f64 = 3.0;
const NAV_INDICATOR_HEIGHT: f64 = 16.0;
const NAV_ITEM_ICON_SIZE: f64 = 20.0;

/// Project a [`UiNavigationView`] into a sidebar + content layout.
pub(crate) fn project_navigation_view(nav: &UiNavigationView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let mut sidebar_style = resolve_style_for_classes(ctx.world, ["nav.sidebar"]);
    let content_style = resolve_style_for_classes(ctx.world, ["nav.content"]);
    // Resolve toggle against the nav entity so hover/pressed InteractionState apply.
    let toggle_style = resolve_style_for_entity_classes(ctx.world, ctx.entity, ["nav.toggle"]);
    let pairs = child_entity_views(&ctx);

    let pane_width = if nav.is_pane_open {
        NAV_PANE_EXPANDED_WIDTH
    } else {
        NAV_PANE_COMPACT_WIDTH
    };

    // Compact mode keeps a tight icon rail; expanded keeps themed padding.
    if !nav.is_pane_open {
        sidebar_style.layout.padding = 4.0;
    }

    let mut item_views = pairs
        .iter()
        .filter_map(|(entity, view)| {
            ctx.world
                .get::<UiNavigationItem>(*entity)
                .filter(|item| item.nav == ctx.entity && item.index < nav.items.len())
                .map(|item| (item.index, view.clone()))
        })
        .collect::<Vec<_>>();
    item_views.sort_by_key(|(index, _)| *index);
    let item_views = item_views
        .into_iter()
        .map(|(_, view)| view.into_any_flex())
        .collect::<Vec<_>>();

    let content_views = pairs
        .iter()
        .filter(|(entity, _)| ctx.world.get::<UiNavigationItem>(*entity).is_none())
        .map(|(_, view)| view.clone())
        .collect::<Vec<_>>();

    // --- Pane toggle (hamburger), WinUI NavigationView style ---
    let toggle_icon = FluentIcon::GlobalNavigationButton;
    let toggle_glyph = toggle_icon.glyph_source();
    let mut toggle_icon_style = ResolvedStyle::default();
    toggle_icon_style.colors.text = toggle_style.colors.text.or(style.colors.text);
    toggle_icon_style.text.size = toggle_style.text.size.max(14.0);
    toggle_icon_style.font_family = Some(toggle_glyph.font_family_vec());
    let toggle_icon_view: UiView = Arc::new(
        sized_box(apply_label_style(
            label(toggle_glyph.glyph().to_string()),
            &toggle_icon_style,
        ))
        .width(Dim::Fixed(Length::px(NAV_ITEM_ICON_SIZE)))
        .height(Dim::Fixed(Length::px(NAV_ITEM_ICON_SIZE))),
    );
    // Expanded pane shows a short action label beside the hamburger glyph.
    let toggle_content: UiView = if nav.is_pane_open {
        let mut label_style = toggle_style.clone();
        label_style.colors.text = toggle_style.colors.text.or(style.colors.text);
        let label_view: UiView = Arc::new(apply_label_style(label("Menu"), &label_style));
        Arc::new(
            flex_row(vec![
                toggle_icon_view.into_any_flex(),
                label_view.into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(8.0)),
        )
    } else {
        Arc::new(
            flex_row(vec![toggle_icon_view.into_any_flex()])
                .main_axis_alignment(MainAxisAlignment::Center)
                .cross_axis_alignment(CrossAxisAlignment::Center),
        )
    };
    let toggle_btn: UiView = Arc::new(apply_direct_widget_style(
        button_with_child_view(
            ctx.entity,
            WidgetUiAction::ToggleNavigationPane { nav: ctx.entity },
            toggle_content,
        ),
        &toggle_style,
    ));

    // --- Sidebar column: toggle + scrollable items ---
    let items_col = flex_col(item_views).gap(Length::px(sidebar_style.layout.gap.max(0.0)));
    let items_portal = scroll_portal(items_col, Point::ORIGIN)
        .constrain_horizontal(true)
        .constrain_vertical(false)
        .content_must_fill(true);
    let sidebar_body = flex_col(vec![
        toggle_btn.into_any_flex(),
        flex_item(items_portal, 1.0).into(),
    ])
    .gap(Length::px(sidebar_style.layout.gap.max(4.0)));
    let sidebar: UiView = Arc::new(
        sized_box(apply_widget_style(sidebar_body, &sidebar_style))
            .width(Dim::Fixed(Length::px(pane_width)))
            .height(Dim::Stretch),
    );

    // --- Content area: show only the selected child, flex-grow to fill space ---
    let content: UiView = content_views
        .get(nav.selected)
        .cloned()
        .unwrap_or_else(|| Arc::new(label("")));

    let content_body = flex_col(vec![
        flex_item(content, 1.0), // flex:1 preserved via From<FlexItem>, NOT .into_any_flex()
    ])
    .dims(
        Dimensions::AUTO
            .with_width(Dim::Stretch)
            .with_height(Dim::Stretch),
    );
    let content_area = sized_box(apply_widget_style(
        scroll_portal(content_body, Point::ORIGIN)
            .constrain_horizontal(true)
            .constrain_vertical(true)
            .content_must_fill(true),
        &content_style,
    ))
    .dims(
        Dimensions::AUTO
            .with_width(Dim::Stretch)
            .with_height(Dim::Stretch),
    );

    // --- Layout: sidebar | content (flex-grow: 1) ---
    let row = flex_row(vec![
        sidebar.into_any_flex(),
        flex_item(content_area, 1.0).into(), // .into() preserves flex params via From<FlexItem>
    ]);
    let clipped_row = scroll_portal(row, Point::ORIGIN)
        .constrain_horizontal(true)
        .constrain_vertical(true)
        .content_must_fill(true);

    Arc::new(
        sized_box(apply_widget_style(clipped_row, &style)).dims(
            Dimensions::AUTO
                .with_width(Dim::Stretch)
                .with_height(Dim::Stretch),
        ),
    )
}

/// Project a single ECS-backed sidebar item for [`UiNavigationView`].
pub(crate) fn project_navigation_item(item: &UiNavigationItem, ctx: ProjectionCtx<'_>) -> UiView {
    let Some(nav) = ctx.world.get::<UiNavigationView>(item.nav) else {
        return Arc::new(label(""));
    };
    let Some(nav_item) = nav.items.get(item.index) else {
        return Arc::new(label(""));
    };

    let is_active = item.index == nav.selected;
    let pane_open = nav.is_pane_open;
    let mut style = if is_active {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["nav.item", "nav.item.active"])
    } else {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["nav.item"])
    };

    // Smooth background transition for active item switching.
    if is_active && style.transition.is_none() {
        style.transition = Some(crate::styling::StyleTransition {
            duration: 0.15,
            easing: None,
        });
    }

    // Left selection indicator — WinUI NavigationView vertical accent bar.
    let mut indicator_style = resolve_style_for_classes(ctx.world, ["nav.item.indicator"]);
    if !is_active {
        indicator_style.colors.bg = indicator_style
            .colors
            .bg
            .map(|c| c.with_alpha(0.0))
            .or(Some(Color::TRANSPARENT));
    }
    if indicator_style.layout.corner_radius <= 0.0 {
        indicator_style.layout.corner_radius = NAV_INDICATOR_WIDTH * 0.5;
    }
    let indicator: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .width(Dim::Fixed(Length::px(NAV_INDICATOR_WIDTH)))
            .height(Dim::Fixed(Length::px(NAV_INDICATOR_HEIGHT))),
        &indicator_style,
    ));

    let icon_view: Option<UiView> = nav_item.icon.map(|icon| -> UiView {
        let mut icon_style = ResolvedStyle::default();
        icon_style.colors.text = style.colors.text;
        icon_style.text.size = style.text.size.max(14.0);
        icon_style.font_family = Some(icon.font_family_vec());
        Arc::new(
            sized_box(apply_label_style(label(icon.glyph().to_string()), &icon_style))
                .width(Dim::Fixed(Length::px(NAV_ITEM_ICON_SIZE)))
                .height(Dim::Fixed(Length::px(NAV_ITEM_ICON_SIZE))),
        ) as UiView
    });

    // Compact pane: icon only (or first-letter fallback when no icon is set).
    let content: UiView = if pane_open {
        let label_view: UiView =
            Arc::new(apply_label_style(label(nav_item.label.clone()), &style));
        if let Some(icon) = icon_view {
            Arc::new(
                flex_row(vec![icon.into_any_flex(), label_view.into_any_flex()])
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(Length::px(8.0)),
            )
        } else {
            label_view
        }
    } else if let Some(icon) = icon_view {
        icon
    } else {
        let letter = nav_item
            .label
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "·".to_string());
        Arc::new(apply_label_style(label(letter), &style))
    };

    // Keep the indicator rail reserved so active/inactive items share layout.
    // Compact mode still left-aligns inside the button so the indicator stays
    // on the leading edge of the rail.
    let row = flex_row(vec![
        flex_col(vec![indicator.into_any_flex()])
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .into_any_flex(),
        flex_item(content, 1.0).into(),
    ])
    .main_axis_alignment(MainAxisAlignment::Start)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(Length::px(if pane_open { 8.0 } else { 4.0 }));

    Arc::new(apply_direct_widget_style(
        button_with_child_view(
            ctx.entity,
            WidgetUiAction::SelectNavigationItem {
                nav: item.nav,
                index: item.index,
            },
            row,
        ),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Time Picker
// ---------------------------------------------------------------------------

pub(crate) fn project_time_picker(picker: &UiTimePicker, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let time_str = if picker.use_24h {
        format!("{:02}:{:02}", picker.hour, picker.minute)
    } else {
        let (h12, is_pm) = picker.hour_12();
        format!(
            "{}:{:02} {}",
            h12,
            picker.minute,
            if is_pm { "PM" } else { "AM" }
        )
    };
    let mut items = Vec::new();
    if let Some(icon_color) = style.colors.text {
        let icon = if picker.is_open {
            vector_icon(VectorIcon::ChevronUp, 10.0, icon_color)
        } else {
            vector_icon(VectorIcon::Clock, 12.0, icon_color)
        };
        items.push(icon.into_any_flex());
    }
    items.push(apply_label_style(label(time_str), &style).into_any_flex());
    let content = flex_row(items)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0));
    Arc::new(apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleTimePicker, content),
        &style,
    ))
}

pub(crate) fn project_time_picker_panel(
    panel_comp: &UiTimePickerPanel,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    let pos = match overlay_position(ctx.world, ctx.entity) {
        Some(p) => p,
        None => return hidden_placeholder(),
    };

    let panel_style = default_panel_style(ctx.world, "overlay.time_picker.panel");
    let cell_style = resolve_style_for_classes(ctx.world, ["overlay.time_picker.cell"]);
    let mut selected_style = cell_style.clone();
    let selected_overrides =
        resolve_style_for_classes(ctx.world, ["overlay.time_picker.cell.selected"]);
    apply_color_overrides(&mut selected_style, &selected_overrides);

    let anchor_entity = ctx.world.get::<AnchoredTo>(ctx.entity).map(|a| a.0);

    let use_24h = panel_comp.use_24h;

    // Fetch current picker values
    let (cur_hour, cur_minute) = anchor_entity
        .and_then(|a| ctx.world.get::<UiTimePicker>(a))
        .map(|p| (p.hour, p.minute))
        .unwrap_or((12, 0));

    let (cur_h12, cur_is_pm) = if use_24h {
        (cur_hour, false)
    } else {
        let picker = anchor_entity
            .and_then(|a| ctx.world.get::<UiTimePicker>(a))
            .unwrap();
        picker.hour_12()
    };

    // --- Hour selector ---
    let hour_count: u8 = if use_24h { 24 } else { 12 };
    let hour_start: u8 = if use_24h { 0 } else { 1 };
    let mut hour_buttons = Vec::new();
    for h in hour_start..hour_start + hour_count {
        let hour_val = h;
        let is_sel = if use_24h {
            hour_val == cur_hour
        } else {
            hour_val == cur_h12
        };
        let s = if is_sel { &selected_style } else { &cell_style };
        let label_text = format!("{:02}", hour_val);
        let btn = button_view(
            ctx.entity,
            OverlayUiAction::SelectTimeHour { hour: hour_val },
            label_text,
        );
        hour_buttons.push(apply_direct_widget_style(btn, s).flex(1.0).into_any_flex());
    }
    let hour_col = flex_col(hour_buttons).gap(Length::px(2.0));

    // --- Minute selector ---
    let mut minute_buttons = Vec::new();
    for m in 0u8..60u8 {
        let is_sel = m == cur_minute;
        let s = if is_sel { &selected_style } else { &cell_style };
        let label_text = format!("{:02}", m);
        let btn = button_view(
            ctx.entity,
            OverlayUiAction::SelectTimeMinute { minute: m },
            label_text,
        );
        minute_buttons.push(apply_direct_widget_style(btn, s).flex(1.0).into_any_flex());
    }
    let min_col = flex_col(minute_buttons).gap(Length::px(2.0));

    // --- Separator ---
    let sep = apply_label_style(label(":"), &cell_style)
        .flex(1.0)
        .into_any_flex();

    let mut columns = vec![hour_col.into_any_flex(), sep, min_col.into_any_flex()];

    // --- AM/PM selector (12h mode) ---
    if !use_24h {
        let am_style = if cur_is_pm {
            &cell_style
        } else {
            &selected_style
        };
        let pm_style = if cur_is_pm {
            &selected_style
        } else {
            &cell_style
        };
        let am_btn = button_view(
            ctx.entity,
            OverlayUiAction::SelectTimePeriod { is_pm: false },
            "AM".to_string(),
        );
        let pm_btn = button_view(
            ctx.entity,
            OverlayUiAction::SelectTimePeriod { is_pm: true },
            "PM".to_string(),
        );
        let am_pm_col = flex_col(vec![
            apply_direct_widget_style(am_btn, am_style).into_any_flex(),
            apply_direct_widget_style(pm_btn, pm_style).into_any_flex(),
        ])
        .gap(Length::px(2.0));
        columns.push(am_pm_col.into_any_flex());
    }

    // --- Done button ---
    let done_btn = button_view(
        ctx.entity,
        OverlayUiAction::DismissTimePicker,
        "Done".to_string(),
    );
    let done_row = flex_row(vec![
        apply_direct_widget_style(done_btn, &cell_style).into_any_flex(),
    ]);

    let content = flex_col(vec![
        flex_row(columns).gap(Length::px(4.0)).into_any_flex(),
        done_row.into_any_flex(),
    ])
    .gap(Length::px(6.0));

    let computed_pos = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let panel_width = if computed_pos.width > 1.0 {
        computed_pos.width
    } else {
        220.0
    };
    let panel_height = if computed_pos.height > 1.0 {
        computed_pos.height
    } else {
        300.0
    };

    let panel_view = apply_widget_style(
        crate::xilem::view::portal(content)
            .dims((Length::px(panel_width), Length::px(panel_height))),
        &panel_style,
    );

    Arc::new(
        transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity, panel_view,
        ))
        .translate(pos),
    )
}

// ---------------------------------------------------------------------------
// Expander
// ---------------------------------------------------------------------------

pub(crate) fn project_expander(expander: &UiExpander, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let header_text = apply_label_style(label(expander.header.clone()), &style);

    let mut header_items = Vec::new();
    if let Some(icon_color) = style.colors.text {
        let chevron = if expander.is_expanded {
            vector_icon(VectorIcon::ChevronDown, 10.0, icon_color)
        } else {
            vector_icon(VectorIcon::ChevronRight, 10.0, icon_color)
        };
        header_items.push(chevron.into_any_flex());
    }
    header_items.push(header_text.into_any_flex());

    let header_row = flex_row(header_items)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0));

    let header_btn = apply_direct_widget_style(
        button_with_child_view(ctx.entity, OverlayUiAction::ToggleExpander, header_row),
        &style,
    );

    let mut items = vec![header_btn.into_any_flex()];

    if expander.is_expanded {
        for child in &ctx.children {
            items.push(child.clone().into_any_flex());
        }
    }

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(items), &style).gap(Length::px(style.layout.gap.max(4.0))),
        &style,
    ))
}

// ---------------------------------------------------------------------------
// Context Menu
// ---------------------------------------------------------------------------

pub(crate) fn project_context_menu(menu: &UiContextMenu, ctx: ProjectionCtx<'_>) -> UiView {
    let pos = match overlay_position(ctx.world, ctx.entity) {
        Some(p) => p,
        None => return hidden_placeholder(),
    };

    let menu_style = default_panel_style(ctx.world, "overlay.context_menu.panel");
    let mut item_style = default_item_style(ctx.world, "overlay.context_menu.item");
    if item_style.colors.text.is_none() {
        item_style.colors.text = menu_style.colors.text;
    }
    let mut disabled_style = item_style.clone();
    if let Some(text) = disabled_style.colors.text {
        disabled_style.colors.text = Some(text.with_alpha(0.4));
    }

    let mut menu_items: Vec<UiView> = Vec::new();
    for (i, item) in menu.items.iter().enumerate() {
        if item.separator_after && !menu_items.is_empty() {
            let separator = sized_box(label(""))
                .height(Dim::Fixed(Length::px(1.0)))
                .width(Dim::Stretch);
            let sep_style =
                resolve_style_for_classes(ctx.world, ["overlay.context_menu.separator"]);
            menu_items.push(Arc::new(apply_widget_style(separator, &sep_style)));
        }

        let label_view = apply_label_style(label(item.label.clone()), &item_style);

        let row_content: UiView = if let Some(icon) = item.icon_glyph {
            let mut icon_style = ResolvedStyle::default();
            icon_style.colors.text = if item.enabled {
                item_style.colors.text
            } else {
                disabled_style.colors.text
            };
            icon_style.text.size = item_style.text.size * 0.9;
            icon_style.font_family = Some(icon.font_family_vec());
            let icon_view = apply_label_style(label(icon.glyph().to_string()), &icon_style);
            Arc::new(
                flex_row(vec![icon_view.into_any_flex(), label_view.into_any_flex()])
                    .gap(Length::px(6.0)),
            )
        } else {
            Arc::new(label_view)
        };

        if item.enabled {
            let btn = button_with_child_view(
                ctx.entity,
                OverlayUiAction::SelectContextMenuItem { index: i },
                row_content,
            );
            menu_items.push(Arc::new(apply_direct_widget_style(btn, &item_style)));
        } else {
            menu_items.push(Arc::new(apply_widget_style(row_content, &disabled_style)));
        }
    }

    let flex_items: Vec<_> = menu_items.into_iter().map(|v| v.into_any_flex()).collect();

    let computed_pos = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let panel_width = if computed_pos.width > 1.0 {
        computed_pos.width
    } else {
        180.0
    };
    let panel_height = if computed_pos.height > 1.0 {
        computed_pos.height
    } else {
        flex_items.len() as f64 * 32.0 + 16.0
    };

    let content = flex_col(flex_items).gap(Length::px(menu_style.layout.gap.max(4.0)));
    let scrollable = crate::xilem::view::portal(content)
        .dims((Length::px(panel_width), Length::px(panel_height)));

    Arc::new(
        transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity,
            apply_widget_style(scrollable, &menu_style),
        ))
        .translate(pos),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // days_in_month
    // -----------------------------------------------------------------------

    #[test]
    fn days_in_january() {
        assert_eq!(days_in_month(2024, 1), 31);
    }

    #[test]
    fn days_in_february_leap() {
        assert_eq!(days_in_month(2024, 2), 29);
    }

    #[test]
    fn days_in_february_non_leap() {
        assert_eq!(days_in_month(2023, 2), 28);
    }

    #[test]
    fn days_in_february_century_non_leap() {
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn days_in_february_century_leap() {
        assert_eq!(days_in_month(2000, 2), 29);
    }

    #[test]
    fn days_in_april() {
        assert_eq!(days_in_month(2024, 4), 30);
    }

    #[test]
    fn days_in_december() {
        assert_eq!(days_in_month(2024, 12), 31);
    }

    // -----------------------------------------------------------------------
    // day_of_week_for_first
    // -----------------------------------------------------------------------

    #[test]
    fn dow_jan_1_2020_was_wednesday() {
        // 2020-01-01 was a Wednesday (0=Sun)
        assert_eq!(day_of_week_for_first(2020, 1), 3);
    }

    #[test]
    fn dow_jan_1_2024_was_monday() {
        assert_eq!(day_of_week_for_first(2024, 1), 1);
    }

    // -----------------------------------------------------------------------
    // month_name
    // -----------------------------------------------------------------------

    #[test]
    fn month_name_values() {
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
        assert_eq!(month_name(13), "——");
    }
}

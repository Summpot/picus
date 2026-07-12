//! Shared test helpers and infrastructure for picus_core tests.
//!
//! This module is compiled only under `#[cfg(test)]` and provides types,
//! projection functions, and utility helpers used by test modules across
//! multiple source files.

#![allow(dead_code)]

use std::sync::Once;

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_input::{
    ButtonInput, ButtonState,
    mouse::{MouseButton, MouseButtonInput},
};
use bevy_math::{Rect, Vec2};
use bevy_window::{PrimaryWindow, Window, WindowResized};
use masonry_core::{
    core::{Widget, WidgetId, WidgetRef, WindowEvent},
    dpi::PhysicalSize,
};

use crate::{
    AppPicusExt, UiEventQueue, UiProjectorRegistry, UiRoot, UiView, register_builtin_projectors,
    synthesize_roots_with_stats,
};

// ---------------------------------------------------------------------------
// Shared test component types
// ---------------------------------------------------------------------------

#[derive(Component, Debug, Clone, Copy)]
pub struct TestRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct TypeStyled;

#[derive(Component, Debug, Clone, Copy)]
pub struct ToastProbe;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestAction {
    Clicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCloseTestAction {
    Closed,
}

pub fn project_test_root(_: &TestRoot, ctx: crate::ProjectionCtx<'_>) -> UiView {
    std::sync::Arc::new(crate::retained_bridge::button_view(
        ctx.entity,
        TestAction::Clicked,
        "Click",
    ))
}

pub fn project_toast_probe(_: &ToastProbe, ctx: crate::ProjectionCtx<'_>) -> UiView {
    std::sync::Arc::new(
        crate::xilem::view::transformed(crate::retained_bridge::opaque_hitbox_for_entity(
            ctx.entity,
            crate::xilem::view::label("Toast"),
        ))
        .translate((620.0, 48.0)),
    )
}

// ---------------------------------------------------------------------------
// Tracing
// ---------------------------------------------------------------------------

pub fn init_test_tracing() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("picus_core=debug"))
            .with_test_writer()
            .try_init();
    });
}

// ---------------------------------------------------------------------------
// Pointer / input helpers
// ---------------------------------------------------------------------------

pub fn set_window_cursor_position(app: &mut App, window_entity: Entity, position: Vec2) {
    let world = app.world_mut();
    let mut window = world
        .get_mut::<Window>(window_entity)
        .expect("window should exist");
    window.set_cursor_position(Some(position));
}

pub fn send_primary_click(app: &mut App, window_entity: Entity, position: Vec2) {
    {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut primary_window = query
            .single_mut(world)
            .expect("primary window should exist");
        primary_window.set_cursor_position(Some(position));
    }

    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: window_entity,
    });
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window: window_entity,
    });

    app.update();
}

/// Simulate a real Bevy click that simultaneously triggers
/// `handle_global_overlay_clicks` (via `ButtonInput::just_pressed`) and
/// `inject_bevy_input_into_masonry` (via `MouseButtonInput` events).
///
/// This is the faithful equivalent of a physical left-click in reactive mode:
/// both the `ButtonInput` state transition and the `MouseButtonInput` event
/// stream are produced in the same frame, so overlay-dismiss logic *and*
/// retained-widget pointer injection both run.
pub fn send_real_click(app: &mut App, window_entity: Entity, position: Vec2) {
    set_window_cursor_position(app, window_entity, position);

    if !app.world().contains_resource::<ButtonInput<MouseButton>>() {
        app.world_mut()
            .insert_resource(ButtonInput::<MouseButton>::default());
    }

    {
        let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        input.release(MouseButton::Left);
        input.clear();
        input.press(MouseButton::Left);
    }

    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: window_entity,
    });
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window: window_entity,
    });

    app.update();

    let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    input.release(MouseButton::Left);
    input.clear();
}

pub fn run_global_overlay_click(app: &mut App, window_entity: Entity, position: Vec2) {
    set_window_cursor_position(app, window_entity, position);

    if !app.world().contains_resource::<ButtonInput<MouseButton>>() {
        app.world_mut()
            .insert_resource(ButtonInput::<MouseButton>::default());
    }

    {
        let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        input.release(MouseButton::Left);
        input.clear();
        input.press(MouseButton::Left);
    }

    app.update();

    let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    input.release(MouseButton::Left);
    input.clear();
}

pub fn hit_path_for_position(
    app: &mut App,
    window_entity: Entity,
    position: Vec2,
) -> Vec<WidgetId> {
    set_window_cursor_position(app, window_entity, position);

    let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary_mut()
        .expect("primary window runtime should exist after app.update()");
    let _ = window_runtime.render_root.redraw();
    window_runtime.get_hit_path((position.x as f64, position.y as f64).into())
}

// ---------------------------------------------------------------------------
// Widget traversal helpers
// ---------------------------------------------------------------------------

pub fn find_widget_id_by_debug_text(
    widget: WidgetRef<'_, dyn Widget>,
    expected_debug_text: &str,
) -> Option<WidgetId> {
    for child in widget.children() {
        if let Some(id) = find_widget_id_by_debug_text(child, expected_debug_text) {
            return Some(id);
        }
    }

    (widget.get_debug_text().as_deref() == Some(expected_debug_text)).then_some(widget.id())
}

pub fn first_widget_id_by_short_name(
    widget: WidgetRef<'_, dyn Widget>,
    short_type_name: &str,
) -> Option<WidgetId> {
    if widget.short_type_name() == short_type_name {
        return Some(widget.id());
    }

    widget
        .children()
        .into_iter()
        .find_map(|child| first_widget_id_by_short_name(child, short_type_name))
}

pub fn first_widget_by_short_name_and_debug_text<'w>(
    widget: WidgetRef<'w, dyn Widget>,
    short_type_name: &str,
    debug_text: &str,
) -> Option<WidgetRef<'w, dyn Widget>> {
    if widget.short_type_name() == short_type_name
        && widget
            .get_debug_text()
            .as_deref()
            .is_some_and(|text| text == debug_text)
    {
        return Some(widget);
    }

    widget.children().into_iter().find_map(|child| {
        first_widget_by_short_name_and_debug_text(child, short_type_name, debug_text)
    })
}

pub fn collect_widget_bounds_by_short_name(
    widget: WidgetRef<'_, dyn Widget>,
    short_type_name: &str,
    bounds: &mut Vec<Rect>,
) {
    for child in widget.children() {
        collect_widget_bounds_by_short_name(child, short_type_name, bounds);
    }

    if widget.short_type_name() == short_type_name {
        let ctx = widget.ctx();
        let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
        let size = ctx.border_box().size();
        bounds.push(Rect::from_corners(
            Vec2::new(origin.x as f32, origin.y as f32),
            Vec2::new(
                (origin.x + size.width) as f32,
                (origin.y + size.height) as f32,
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Widget geometry helpers
// ---------------------------------------------------------------------------

pub fn widget_center_for_widget_id(app: &App, widget_id: WidgetId) -> Vec2 {
    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget = window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree");

    let ctx = widget.ctx();
    let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
    let size = ctx.border_box().size();
    Vec2::new(
        (origin.x + size.width * 0.5) as f32,
        (origin.y + size.height * 0.5) as f32,
    )
}

pub fn widget_inset_point_for_widget_id(app: &App, widget_id: WidgetId, inset: f64) -> Vec2 {
    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget = window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree");

    let ctx = widget.ctx();
    let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
    Vec2::new((origin.x + inset) as f32, (origin.y + inset) as f32)
}

pub fn widget_center_for_entity(app: &App, entity: Entity) -> Vec2 {
    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget_id = window_runtime
        .find_widget_id_for_entity_bits(entity.to_bits(), true)
        .or_else(|| window_runtime.find_widget_id_for_entity_bits(entity.to_bits(), false))
        .expect("entity should resolve to a Masonry widget");
    widget_center_for_widget_id(app, widget_id)
}

pub fn widget_rect_for_entity(app: &App, entity: Entity) -> Rect {
    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget_id = window_runtime
        .find_widget_id_for_entity_bits(entity.to_bits(), false)
        .expect("entity should resolve to a Masonry widget");
    let widget = window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree");
    let ctx = widget.ctx();
    let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
    let size = ctx.border_box().size();

    Rect {
        min: Vec2::new(origin.x as f32, origin.y as f32),
        max: Vec2::new(
            (origin.x + size.width) as f32,
            (origin.y + size.height) as f32,
        ),
    }
}

pub fn widget_height_for_entity(app: &App, entity: Entity) -> f64 {
    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget_id = window_runtime
        .find_widget_id_for_entity_bits(entity.to_bits(), false)
        .expect("entity should resolve to a Masonry widget");
    window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree")
        .ctx()
        .border_box()
        .height()
}

pub fn widget_ids_for_entity_subtree(app: &App, entity: Entity) -> Vec<WidgetId> {
    fn collect_widget_ids(widget: WidgetRef<'_, dyn Widget>, ids: &mut Vec<WidgetId>) {
        if widget.ctx().is_stashed() {
            return;
        }
        ids.push(widget.id());
        for child in widget.children() {
            collect_widget_ids(child, ids);
        }
    }

    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget_id = window_runtime
        .find_widget_id_for_entity_bits(entity.to_bits(), false)
        .expect("entity should resolve to a Masonry widget");
    let widget = window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree");
    let mut ids = Vec::new();
    collect_widget_ids(widget, &mut ids);
    ids
}

pub fn portal_rects_for_entity(app: &App, entity: Entity) -> Vec<Rect> {
    fn collect_portal_rects(widget: WidgetRef<'_, dyn Widget>, rects: &mut Vec<Rect>) {
        if widget.ctx().is_stashed() {
            return;
        }

        if widget.short_type_name() == "Portal" {
            let ctx = widget.ctx();
            let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
            let size = ctx.border_box().size();
            rects.push(Rect {
                min: Vec2::new(origin.x as f32, origin.y as f32),
                max: Vec2::new(
                    (origin.x + size.width) as f32,
                    (origin.y + size.height) as f32,
                ),
            });
        }

        for child in widget.children() {
            collect_portal_rects(child, rects);
        }
    }

    let runtime = app.world().non_send::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary()
        .expect("primary window runtime should exist");
    let widget_id = window_runtime
        .find_widget_id_for_entity_bits(entity.to_bits(), false)
        .expect("entity should resolve to a Masonry widget");
    let widget = window_runtime
        .render_root
        .get_widget(widget_id)
        .expect("widget id should resolve in render tree");
    let mut rects = Vec::new();
    collect_portal_rects(widget, &mut rects);
    rects
}

// ---------------------------------------------------------------------------
// Window / runtime helpers
// ---------------------------------------------------------------------------

pub fn resize_primary_window(app: &mut App, window_entity: Entity, width: f32, height: f32) {
    {
        let mut window = app
            .world_mut()
            .get_mut::<Window>(window_entity)
            .expect("window should exist");
        window.resolution.set(width, height);
    }

    app.world_mut().write_message(WindowResized {
        window: window_entity,
        width: 1.0,
        height: 1.0,
    });

    app.update();
}

pub fn resize_masonry_runtime(app: &mut App, width: u32, height: u32) {
    let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
    let window_runtime = runtime
        .primary_mut()
        .expect("primary window runtime should exist");
    let _ = window_runtime
        .render_root
        .handle_window_event(WindowEvent::Resize(PhysicalSize::new(width, height)));
    let _ = window_runtime.render_root.redraw();
}

// ---------------------------------------------------------------------------
// Navigation view probe
// ---------------------------------------------------------------------------

pub fn spawn_navigation_height_probe(app: &mut App) -> Entity {
    let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
    let body = app
        .world_mut()
        .spawn((
            crate::UiFlexColumn,
            crate::InlineStyle {
                layout: crate::LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::bevy_ecs::hierarchy::ChildOf(root),
        ))
        .id();
    let nav = app
        .world_mut()
        .spawn((
            crate::UiNavigationView::new([
                crate::NavigationViewItem::new("First"),
                crate::NavigationViewItem::new("Second"),
            ]),
            crate::InlineStyle {
                layout: crate::LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::bevy_ecs::hierarchy::ChildOf(body),
        ))
        .id();

    app.world_mut().spawn((
        crate::UiLabel::new("Selected page"),
        crate::bevy_ecs::hierarchy::ChildOf(nav),
    ));

    nav
}

pub fn spawn_navigation_clipping_probe(app: &mut App) -> Entity {
    let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
    let nav = app
        .world_mut()
        .spawn((
            crate::UiNavigationView::new([
                crate::NavigationViewItem::new("First"),
                crate::NavigationViewItem::new("Second"),
            ]),
            crate::InlineStyle {
                layout: crate::LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            crate::bevy_ecs::hierarchy::ChildOf(root),
        ))
        .id();
    let scroll = app
        .world_mut()
        .spawn((
            crate::UiScrollView::new(Vec2::new(1040.0, 560.0), Vec2::new(1040.0, 5200.0))
                .with_vertical_scrollbar(true)
                .with_horizontal_scrollbar(false),
            crate::bevy_ecs::hierarchy::ChildOf(nav),
        ))
        .id();
    let page = app
        .world_mut()
        .spawn((
            crate::UiFlexColumn,
            crate::bevy_ecs::hierarchy::ChildOf(scroll),
        ))
        .id();

    for index in 0..80 {
        app.world_mut().spawn((
            crate::UiLabel::new(format!("Overflow row {index}")),
            crate::bevy_ecs::hierarchy::ChildOf(page),
        ));
    }

    app.world_mut().spawn((
        crate::UiLabel::new("Footer below navigation"),
        crate::bevy_ecs::hierarchy::ChildOf(root),
    ));

    nav
}

// ---------------------------------------------------------------------------
// Overlay helpers
// ---------------------------------------------------------------------------

pub fn open_combo_dropdown(app: &mut App, combo: Entity) -> Entity {
    app.world()
        .resource::<UiEventQueue>()
        .push_typed(combo, crate::OverlayUiAction::ToggleCombo);

    app.update();

    let mut query = app.world_mut().query::<(Entity, &crate::AnchoredTo)>();
    query
        .iter(app.world())
        .find_map(|(entity, anchored_to)| {
            app.world()
                .get::<crate::UiDropdownMenu>(entity)
                .is_some_and(|_| anchored_to.0 == combo)
                .then_some(entity)
        })
        .expect("combo toggle should create dropdown")
}

pub fn assert_overlay_defaults_for_entity(
    world: &World,
    entity: Entity,
    label: &str,
    expected_config: crate::OverlayConfig,
    expected_state: crate::OverlayState,
    expect_anchor_rect: bool,
) {
    let config = world
        .get::<crate::OverlayConfig>(entity)
        .unwrap_or_else(|| panic!("{label} should receive overlay config"));
    assert_eq!(*config, expected_config);

    let state = world
        .get::<crate::OverlayState>(entity)
        .unwrap_or_else(|| panic!("{label} should receive overlay state"));
    assert_eq!(*state, expected_state);

    let position = world
        .get::<crate::OverlayComputedPosition>(entity)
        .unwrap_or_else(|| panic!("{label} should receive computed position"));
    assert_eq!(*position, crate::OverlayComputedPosition::default());

    if expect_anchor_rect {
        let anchor_rect = world
            .get::<crate::OverlayAnchorRect>(entity)
            .unwrap_or_else(|| panic!("{label} should receive overlay anchor rect"));
        assert_eq!(*anchor_rect, crate::OverlayAnchorRect::default());
    } else {
        assert!(
            world.get::<crate::OverlayAnchorRect>(entity).is_none(),
            "{label} should not receive overlay anchor rect"
        );
    }
}

// ---------------------------------------------------------------------------
// Synthesis helpers
// ---------------------------------------------------------------------------

/// Build a bare-bones app with a primary window and an overlay root, returning
/// the window entity for use in pointer-injection tests.
pub fn build_simple_windowed_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(crate::PicusPlugin);

    let mut window = Window::default();
    window.resolution.set(800.0, 600.0);
    let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

    (app, window_entity)
}

/// Convenience: register TestRoot projector, spawn UiRoot+TestRoot, update once.
pub fn bootstrap_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(crate::PicusPlugin)
        .register_projector::<TestRoot>(project_test_root);
    app.world_mut().spawn((UiRoot, TestRoot));
    app.update();
    app
}

/// Run synthesis on a bare `World` and return roots + stats.
pub fn synthesize_bare(world: &World, roots: &[Entity]) -> (Vec<UiView>, crate::UiSynthesisStats) {
    let mut registry = UiProjectorRegistry::default();
    register_builtin_projectors(&mut registry);
    synthesize_roots_with_stats(world, &registry, roots.iter().copied())
}

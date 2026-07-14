use bevy_ecs::{entity::Entity, prelude::*};
use bevy_math::Vec2;

use crate::{
    ProjectionCtx, StyleClass, UiLabel, UiView, components::UiComponentTemplate,
    templates::ensure_template_part,
};

/// Scroll axis used by [`UiScrollView`] interactions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrollAxis {
    Horizontal,
    #[default]
    Vertical,
}

/// Built-in portal-backed scroll container.
///
/// This component stores logical scroll state (`scroll_offset`) together with
/// viewport/content extents. Projectors can use this state both for rendering
/// and for virtualization decisions.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiScrollView {
    pub scroll_offset: Vec2,
    pub content_size: Vec2,
    pub viewport_size: Vec2,
    pub show_horizontal_scrollbar: bool,
    pub show_vertical_scrollbar: bool,
}

impl Default for UiScrollView {
    fn default() -> Self {
        Self {
            scroll_offset: Vec2::ZERO,
            content_size: Vec2::new(960.0, 960.0),
            viewport_size: Vec2::new(420.0, 280.0),
            show_horizontal_scrollbar: false,
            show_vertical_scrollbar: true,
        }
    }
}

impl UiScrollView {
    #[must_use]
    pub fn new(viewport_size: Vec2, content_size: Vec2) -> Self {
        Self {
            viewport_size,
            content_size,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_horizontal_scrollbar(mut self, enabled: bool) -> Self {
        self.show_horizontal_scrollbar = enabled;
        self
    }

    #[must_use]
    pub fn with_vertical_scrollbar(mut self, enabled: bool) -> Self {
        self.show_vertical_scrollbar = enabled;
        self
    }

    #[must_use]
    pub fn max_scroll_offset(self) -> Vec2 {
        Vec2::new(
            (self.content_size.x - self.viewport_size.x).max(0.0),
            (self.content_size.y - self.viewport_size.y).max(0.0),
        )
    }

    pub fn clamp_scroll_offset(&mut self) {
        let max = self.max_scroll_offset();
        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max.x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max.y);
    }

    /// Virtualization helper: visible content rectangle in content-space.
    #[must_use]
    pub fn visible_rect(self) -> (Vec2, Vec2) {
        let start = self.scroll_offset.max(Vec2::ZERO);
        let end = start + self.viewport_size.max(Vec2::ZERO);
        (start, end)
    }
}

/// Emitted when a [`UiScrollView`] offset changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiScrollViewChanged {
    pub scroll_view: Entity,
    pub scroll_offset: Vec2,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartScrollViewport;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartScrollBarVertical;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartScrollBarHorizontal;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartScrollThumbVertical;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartScrollThumbHorizontal;

impl UiComponentTemplate for UiScrollView {
    fn expand(world: &mut World, entity: Entity) {
        let _viewport = ensure_template_part::<PartScrollViewport, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.scroll_view.viewport".to_string()]),
            )
        });

        let _vertical_bar = ensure_template_part::<PartScrollBarVertical, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.scroll_view.scrollbar.vertical".to_string()]),
            )
        });

        let _vertical_thumb =
            ensure_template_part::<PartScrollThumbVertical, _>(world, entity, || {
                (
                    UiLabel::new(""),
                    StyleClass(vec!["template.scroll_view.thumb.vertical".to_string()]),
                )
            });

        let _horizontal_bar =
            ensure_template_part::<PartScrollBarHorizontal, _>(world, entity, || {
                (
                    UiLabel::new(""),
                    StyleClass(vec![
                        "template.scroll_view.scrollbar.horizontal".to_string(),
                    ]),
                )
            });

        let _horizontal_thumb =
            ensure_template_part::<PartScrollThumbHorizontal, _>(world, entity, || {
                (
                    UiLabel::new(""),
                    StyleClass(vec!["template.scroll_view.thumb.horizontal".to_string()]),
                )
            });
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_scroll_view(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use crate::{HasTooltip, InteractionState, PicusPlugin, UiEventQueue, UiFlexColumn, UiRoot};
    use bevy_app::App;
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_window::{PrimaryWindow, Window};

    #[test]
    fn scroll_view_template_expands_required_parts() {
        let mut world = World::new();

        let scroll_view = world.spawn((crate::UiScrollView::default(),)).id();
        crate::expand_builtin_ui_component_templates(&mut world);

        assert!(
            crate::find_template_part::<crate::PartScrollViewport>(&world, scroll_view).is_some()
        );
        assert!(
            crate::find_template_part::<crate::PartScrollBarVertical>(&world, scroll_view)
                .is_some()
        );
        assert!(
            crate::find_template_part::<crate::PartScrollThumbVertical>(&world, scroll_view)
                .is_some()
        );
        assert!(
            crate::find_template_part::<crate::PartScrollBarHorizontal>(&world, scroll_view)
                .is_some()
        );
        assert!(
            crate::find_template_part::<crate::PartScrollThumbHorizontal>(&world, scroll_view)
                .is_some()
        );
    }

    #[test]
    fn drag_scroll_thumb_action_updates_scroll_view_offset() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let scroll_view = world
            .spawn((crate::UiScrollView {
                scroll_offset: bevy_math::Vec2::ZERO,
                content_size: bevy_math::Vec2::new(400.0, 1200.0),
                viewport_size: bevy_math::Vec2::new(300.0, 200.0),
                show_horizontal_scrollbar: false,
                show_vertical_scrollbar: true,
            },))
            .id();

        crate::expand_builtin_ui_component_templates(&mut world);

        let thumb =
            crate::find_template_part::<crate::PartScrollThumbVertical>(&world, scroll_view)
                .expect("vertical thumb part should exist");

        world.resource::<UiEventQueue>().push_typed(
            thumb,
            crate::WidgetUiAction::DragScrollThumb {
                thumb,
                axis: crate::ScrollAxis::Vertical,
                delta_pixels: 18.0,
            },
        );

        crate::handle_widget_actions(&mut world);

        let offset = world
            .get::<crate::UiScrollView>(scroll_view)
            .expect("scroll view should exist")
            .scroll_offset;
        assert!(offset.y > 0.0);

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiScrollViewChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].entity, scroll_view);
    }

    #[test]
    fn tooltip_hover_spawns_and_despawns_overlay_entity() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let source = app
            .world_mut()
            .spawn((
                crate::UiButton::new("Hover me"),
                HasTooltip::new("Tooltip text"),
                InteractionState {
                    hovered: true,
                    pressed: false,
                    focused: false,
                },
                ChildOf(root),
            ))
            .id();

        app.update();

        let mut tooltip_query = app.world_mut().query::<(
            Entity,
            &crate::UiTooltip,
            &crate::OverlayState,
            &crate::OverlayConfig,
        )>();
        let spawned_tooltips: Vec<_> = tooltip_query
            .iter(app.world())
            .filter(|(_, _, state, _)| !state.is_modal)
            .map(|(e, t, s, c)| (e, t.clone(), *s, *c))
            .collect();

        assert_eq!(
            spawned_tooltips.len(),
            1,
            "hovered button should spawn exactly one tooltip overlay"
        );
        assert_eq!(spawned_tooltips[0].1.text, "Tooltip text");

        // Clear hovered state and update again to trigger despawn.
        app.world_mut().entity_mut(source).insert(InteractionState {
            hovered: false,
            ..InteractionState::default()
        });
        app.update();

        // Tooltip should be despawned after hover ends.
        let remaining: Vec<_> = tooltip_query
            .iter(app.world())
            .filter(|(_, _, state, _)| !state.is_modal)
            .collect();
        assert_eq!(
            remaining.len(),
            0,
            "tooltip should despawn when source is no longer hovered"
        );
    }

    #[test]
    fn scroll_view_geometry_sync_clamps_out_of_bounds_offset() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, UiFlexColumn)).id();
        let scroll_view = app
            .world_mut()
            .spawn((
                crate::UiScrollView {
                    scroll_offset: bevy_math::Vec2::new(0.0, 9999.0),
                    content_size: bevy_math::Vec2::new(400.0, 1200.0),
                    viewport_size: bevy_math::Vec2::new(300.0, 200.0),
                    show_horizontal_scrollbar: false,
                    show_vertical_scrollbar: true,
                },
                ChildOf(root),
            ))
            .id();

        app.update();
        app.update();

        let state = app
            .world()
            .get::<crate::UiScrollView>(scroll_view)
            .expect("scroll view should exist");
        assert!(
            state.scroll_offset.y <= (state.content_size.y - state.viewport_size.y).max(0.0),
            "scroll offset should be clamped to max scrollable range"
        );
    }

    #[test]
    fn scroll_view_geometry_sync_expands_viewport_width_to_parent_width() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, UiFlexColumn)).id();
        let scroll_view = app
            .world_mut()
            .spawn((
                crate::UiScrollView {
                    scroll_offset: bevy_math::Vec2::ZERO,
                    content_size: bevy_math::Vec2::new(960.0, 1200.0),
                    viewport_size: bevy_math::Vec2::new(300.0, 200.0),
                    show_horizontal_scrollbar: false,
                    show_vertical_scrollbar: true,
                },
                ChildOf(root),
            ))
            .id();

        app.update();
        app.update();

        let state = app
            .world()
            .get::<crate::UiScrollView>(scroll_view)
            .expect("scroll view should exist");
        assert!(
            state.viewport_size.x > 400.0,
            "viewport width should stretch beyond the initial seed width, got {}",
            state.viewport_size.x
        );
    }

    #[test]
    fn scroll_view_left_aligns_narrow_content_after_viewport_stretch() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, UiFlexColumn)).id();
        let scroll_view = app
            .world_mut()
            .spawn((
                crate::UiScrollView {
                    scroll_offset: bevy_math::Vec2::ZERO,
                    content_size: bevy_math::Vec2::new(120.0, 1200.0),
                    viewport_size: bevy_math::Vec2::new(300.0, 200.0),
                    show_horizontal_scrollbar: false,
                    show_vertical_scrollbar: true,
                },
                ChildOf(root),
            ))
            .id();

        app.world_mut().spawn((
            crate::UiLabel::new("Left aligned scroll content"),
            ChildOf(scroll_view),
        ));

        app.update();
        app.update();

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        let window_runtime = runtime
            .primary()
            .expect("primary window runtime should exist");
        let scroll_root = window_runtime.render_root.get_layer_root(0);

        let scroll_widget_id = window_runtime
            .find_widget_id_for_entity_bits(scroll_view.to_bits(), true)
            .or_else(|| window_runtime.find_widget_id_for_entity_bits(scroll_view.to_bits(), false))
            .expect("scroll view should resolve to a Masonry widget");
        let label_widget_id =
            find_widget_id_by_debug_text(scroll_root, "Left aligned scroll content")
                .expect("label widget should exist in render tree");

        let scroll_widget = window_runtime
            .render_root
            .get_widget(scroll_widget_id)
            .expect("scroll widget id should resolve");
        let label_widget = window_runtime
            .render_root
            .get_widget(label_widget_id)
            .expect("label widget id should resolve");

        let scroll_x = scroll_widget
            .ctx()
            .to_window(crate::masonry_core::kurbo::Point::ZERO)
            .x;
        let label_x = label_widget
            .ctx()
            .to_window(crate::masonry_core::kurbo::Point::ZERO)
            .x;

        assert!(
            (label_x - scroll_x).abs() <= 4.0,
            "scroll content should start at the viewport left edge, got scroll_x={scroll_x}, label_x={label_x}"
        );
    }
}

use std::collections::HashSet;

use bevy_ecs::{entity::Entity, hierarchy::ChildOf, message::MessageReader, prelude::*};
use bevy_input::mouse::{MouseScrollUnit, MouseWheel};
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_window::{PrimaryWindow, Window};
use masonry::core::{Widget, WidgetRef};

use crate::{
    AnchoredTo, AutoDismiss, HasTooltip, InteractionState, MasonryRuntime, OverlayAnchorRect,
    OverlayComputedPosition, OverlayConfig, OverlayPlacement, OverlayState, ScrollAxis, UiCheckbox,
    UiCheckboxChanged, UiOverlayRoot, UiRadioGroup, UiRadioGroupChanged, UiScrollView,
    UiScrollViewChanged, UiSlider, UiSliderChanged, UiSwitch, UiSwitchChanged, UiTabBar,
    UiTabChanged, UiTextInput, UiTextInputChanged, UiTooltip, UiTreeNode, UiTreeNodeToggled,
    events::UiEventQueue,
};

/// Internal action enum for non-overlay widget interactions.
///
/// These actions are emitted by built-in widget projectors and consumed by
/// [`handle_widget_actions`] each frame.
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetUiAction {
    /// Select a specific item in a radio group.
    SelectRadioItem { group: Entity, index: usize },
    /// Switch the active tab in a tab bar.
    SelectTab { bar: Entity, index: usize },
    /// Expand or collapse a tree node.
    ToggleTreeNode { node: Entity },
    /// Toggle a checkbox.
    ToggleCheckbox { checkbox: Entity },
    /// Set a checkbox to an explicit checked state.
    SetCheckbox { checkbox: Entity, checked: bool },
    /// Adjust slider value using step increments.
    StepSlider { slider: Entity, delta: f64 },
    /// Set a slider value directly from a native slider interaction.
    SetSliderValue { slider: Entity, value: f64 },
    /// Toggle a switch.
    ToggleSwitch { switch: Entity },
    /// Update text input contents.
    SetTextInput { input: Entity, value: String },
    /// Drag an ECS scroll-thumb by a physical pixel delta.
    DragScrollThumb {
        thumb: Entity,
        axis: ScrollAxis,
        delta_pixels: f64,
    },
}

const SCROLLBAR_MIN_THUMB: f64 = 24.0;

fn thumb_length(viewport: f64, content: f64) -> f64 {
    if content <= 0.0 {
        return viewport.max(0.0);
    }
    let ratio = (viewport / content).clamp(0.0, 1.0);
    (viewport * ratio).clamp(SCROLLBAR_MIN_THUMB.min(viewport), viewport)
}

fn scroll_delta_from_thumb_drag(
    scroll_view: UiScrollView,
    axis: ScrollAxis,
    delta_pixels: f64,
) -> f32 {
    let (viewport, content) = match axis {
        ScrollAxis::Horizontal => (
            scroll_view.viewport_size.x as f64,
            scroll_view.content_size.x as f64,
        ),
        ScrollAxis::Vertical => (
            scroll_view.viewport_size.y as f64,
            scroll_view.content_size.y as f64,
        ),
    };

    let max_scroll = (content - viewport).max(0.0);
    if max_scroll <= f64::EPSILON {
        return 0.0;
    }

    let track_len = viewport.max(1.0);
    let thumb_len = thumb_length(viewport, content);
    let travel = (track_len - thumb_len).max(1.0);

    (delta_pixels * (max_scroll / travel)) as f32
}

fn clamp_scroll_offset_strict(scroll_view: &mut UiScrollView) {
    let max_scroll_y = (scroll_view.content_size.y - scroll_view.viewport_size.y).max(0.0);
    let max_scroll_x = (scroll_view.content_size.x - scroll_view.viewport_size.x).max(0.0);

    scroll_view.scroll_offset.x = scroll_view.scroll_offset.x.clamp(0.0, max_scroll_x);
    scroll_view.scroll_offset.y = scroll_view.scroll_offset.y.clamp(0.0, max_scroll_y);
}

fn quantize_slider_value(slider: &UiSlider, value: f64) -> f64 {
    let step = slider.step.max(f64::EPSILON);
    let steps = ((value - slider.min) / step).round();
    (slider.min + steps * step).clamp(slider.min, slider.max)
}

fn find_ancestor_scroll_view(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<UiScrollView>(entity).is_some() {
            return Some(entity);
        }

        let parent = world
            .get::<ChildOf>(entity)
            .map(|child_of| child_of.parent())?;
        entity = parent;
    }
}

fn parse_entity_bits_from_debug(debug: &str) -> Option<u64> {
    if let Some(bits) = debug.strip_prefix("opaque_hitbox_entity=") {
        return bits.parse::<u64>().ok();
    }
    if let Some(bits) = debug.strip_prefix("entity_scope=") {
        return bits.parse::<u64>().ok();
    }
    if let Some(bits) = debug.strip_prefix("entity=") {
        return bits.parse::<u64>().ok();
    }
    None
}

fn collect_scroll_view_targets_from_hit_path(
    runtime: &MasonryRuntime,
    hit_path: &[masonry::core::WidgetId],
    parents: &Query<&ChildOf>,
    scroll_markers: &Query<(), With<UiScrollView>>,
) -> Vec<Entity> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    for widget_id in hit_path.iter().rev().copied() {
        let Some(entity_bits) = runtime
            .render_root
            .get_widget(widget_id)
            .and_then(|widget| widget.get_debug_text())
            .and_then(|debug| parse_entity_bits_from_debug(&debug))
        else {
            continue;
        };

        let Some(mut entity) = Entity::try_from_bits(entity_bits) else {
            continue;
        };

        loop {
            if scroll_markers.get(entity).is_ok() {
                if seen.insert(entity) {
                    ordered.push(entity);
                }
                break;
            }

            let Ok(parent) = parents.get(entity) else {
                break;
            };
            entity = parent.parent();
        }
    }

    ordered
}

fn portal_geometry_from_subtree(widget: WidgetRef<'_, dyn Widget>) -> Option<(Vec2, Vec2)> {
    if widget.ctx().is_stashed() {
        return None;
    }

    if widget.short_type_name() == "Portal" {
        let viewport_size = widget.ctx().border_box_size();
        let viewport = Vec2::new(viewport_size.width as f32, viewport_size.height as f32);

        let content = widget
            .children()
            .into_iter()
            .find(|child| !child.ctx().is_stashed() && child.short_type_name() != "ScrollBar")
            .map(|child| {
                let size = child.ctx().border_box_size();
                Vec2::new(size.width as f32, size.height as f32)
            })
            .unwrap_or(viewport);

        return Some((viewport.max(Vec2::ZERO), content.max(Vec2::ZERO)));
    }

    for child in widget.children() {
        if let Some(geometry) = portal_geometry_from_subtree(child) {
            return Some(geometry);
        }
    }

    None
}

/// Synchronize [`UiScrollView`] geometry from Masonry layout results.
///
/// This keeps ECS `content_size`/`viewport_size` aligned with the retained portal,
/// then strictly clamps `scroll_offset` so it cannot drift past real bounds.
pub fn sync_scroll_view_layout_geometry(
    runtime: Option<NonSend<MasonryRuntime>>,
    mut scroll_views: Query<(Entity, &mut UiScrollView)>,
    ui_events: Res<UiEventQueue>,
) {
    let Some(runtime) = runtime else {
        return;
    };

    for (entity, mut scroll_view) in &mut scroll_views {
        let widget_id = runtime
            .find_widget_id_for_entity_bits(entity.to_bits(), false)
            .or_else(|| runtime.find_widget_id_for_entity_bits(entity.to_bits(), true));

        let Some(widget_id) = widget_id else {
            continue;
        };

        let Some(root_widget) = runtime.render_root.get_widget(widget_id) else {
            continue;
        };

        let Some((viewport_size, content_size)) = portal_geometry_from_subtree(root_widget) else {
            continue;
        };

        let before = scroll_view.scroll_offset;
        scroll_view.viewport_size = viewport_size;
        scroll_view.content_size = content_size;
        clamp_scroll_offset_strict(&mut scroll_view);

        if scroll_view.scroll_offset != before {
            ui_events.push_typed(
                entity,
                UiScrollViewChanged {
                    scroll_view: entity,
                    scroll_offset: scroll_view.scroll_offset,
                },
            );
        }
    }
}

/// Consume [`WidgetUiAction`] entries from [`UiEventQueue`] and apply the
/// corresponding state mutations.
///
/// After mutating each component the system re-emits the appropriate
/// high-level changed event so application code can react to it.
pub fn handle_widget_actions(world: &mut World) {
    let actions = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<WidgetUiAction>();

    for event in actions {
        match event.action {
            WidgetUiAction::SelectRadioItem { group, index } => {
                if world.get_entity(group).is_err() {
                    continue;
                }

                let changed = if let Some(mut radio_group) = world.get_mut::<UiRadioGroup>(group) {
                    radio_group.selected = index;
                    Some(UiRadioGroupChanged {
                        group,
                        selected: index,
                    })
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(group, ev);
                }
            }

            WidgetUiAction::SelectTab { bar, index } => {
                if world.get_entity(bar).is_err() {
                    continue;
                }

                let changed = if let Some(mut tab_bar) = world.get_mut::<UiTabBar>(bar) {
                    tab_bar.active = index;
                    Some(UiTabChanged { bar, active: index })
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(bar, ev);
                }
            }

            WidgetUiAction::ToggleTreeNode { node } => {
                if world.get_entity(node).is_err() {
                    continue;
                }

                let toggled = world
                    .get::<UiTreeNode>(node)
                    .map(|tree_node| !tree_node.is_expanded);

                if let Some(is_expanded) = toggled {
                    if let Some(mut tree_node) = world.get_mut::<UiTreeNode>(node) {
                        tree_node.is_expanded = is_expanded;
                    }
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(node, UiTreeNodeToggled { node, is_expanded });
                }
            }

            WidgetUiAction::ToggleCheckbox { checkbox } => {
                if world.get_entity(checkbox).is_err() {
                    continue;
                }

                let changed =
                    if let Some(mut checkbox_state) = world.get_mut::<UiCheckbox>(checkbox) {
                        checkbox_state.checked = !checkbox_state.checked;
                        Some(checkbox_state.checked)
                    } else {
                        None
                    };

                if let Some(checked) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(checkbox, UiCheckboxChanged { checkbox, checked });
                }
            }

            WidgetUiAction::SetCheckbox { checkbox, checked } => {
                if world.get_entity(checkbox).is_err() {
                    continue;
                }

                let changed =
                    if let Some(mut checkbox_state) = world.get_mut::<UiCheckbox>(checkbox) {
                        if checkbox_state.checked == checked {
                            None
                        } else {
                            checkbox_state.checked = checked;
                            Some(checked)
                        }
                    } else {
                        None
                    };

                if let Some(checked) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(checkbox, UiCheckboxChanged { checkbox, checked });
                }
            }

            WidgetUiAction::StepSlider { slider, delta } => {
                if world.get_entity(slider).is_err() {
                    continue;
                }

                if let Some(mut slider_state) = world.get_mut::<UiSlider>(slider) {
                    let step = slider_state.step.max(f64::EPSILON);
                    let next = quantize_slider_value(
                        &slider_state,
                        (slider_state.value + delta * step)
                            .clamp(slider_state.min, slider_state.max),
                    );
                    if (next - slider_state.value).abs() > f64::EPSILON {
                        slider_state.value = next;
                        world.resource::<UiEventQueue>().push_typed(
                            slider,
                            UiSliderChanged {
                                slider,
                                value: next,
                            },
                        );
                    }
                }
            }

            WidgetUiAction::SetSliderValue { slider, value } => {
                if world.get_entity(slider).is_err() {
                    continue;
                }

                if let Some(mut slider_state) = world.get_mut::<UiSlider>(slider) {
                    let next = quantize_slider_value(&slider_state, value);
                    if (next - slider_state.value).abs() > f64::EPSILON {
                        slider_state.value = next;
                        world.resource::<UiEventQueue>().push_typed(
                            slider,
                            UiSliderChanged {
                                slider,
                                value: next,
                            },
                        );
                    }
                }
            }

            WidgetUiAction::ToggleSwitch { switch } => {
                if world.get_entity(switch).is_err() {
                    continue;
                }

                let changed = if let Some(mut switch_state) = world.get_mut::<UiSwitch>(switch) {
                    switch_state.on = !switch_state.on;
                    Some(switch_state.on)
                } else {
                    None
                };

                if let Some(on) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(switch, UiSwitchChanged { switch, on });
                }
            }

            WidgetUiAction::SetTextInput { input, value } => {
                if world.get_entity(input).is_err() {
                    continue;
                }

                if let Some(mut text_input) = world.get_mut::<UiTextInput>(input) {
                    text_input.value = value.clone();
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(input, UiTextInputChanged { input, value });
                }
            }

            WidgetUiAction::DragScrollThumb {
                thumb,
                axis,
                delta_pixels,
            } => {
                if world.get_entity(thumb).is_err() {
                    continue;
                }

                let Some(scroll_entity) = find_ancestor_scroll_view(world, thumb) else {
                    continue;
                };

                let changed =
                    if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(scroll_entity) {
                        let before = scroll_view.scroll_offset;
                        let delta = scroll_delta_from_thumb_drag(*scroll_view, axis, delta_pixels);

                        match axis {
                            ScrollAxis::Horizontal => {
                                scroll_view.scroll_offset.x += delta;
                            }
                            ScrollAxis::Vertical => {
                                scroll_view.scroll_offset.y += delta;
                            }
                        }

                        clamp_scroll_offset_strict(&mut scroll_view);
                        let after = scroll_view.scroll_offset;
                        (after != before).then_some(after)
                    } else {
                        None
                    };

                if let Some(scroll_offset) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        scroll_entity,
                        UiScrollViewChanged {
                            scroll_view: scroll_entity,
                            scroll_offset,
                        },
                    );
                }
            }
        }
    }
}

/// Route mouse-wheel input to the nearest hit-tested [`UiScrollView`] entity.
///
/// This keeps ECS `scroll_offset` synchronized with pointer-wheel interactions
/// while the portal primitive handles clipping/composition.
pub fn handle_scroll_view_wheel(
    runtime: Option<NonSend<MasonryRuntime>>,
    mut wheel_events: MessageReader<MouseWheel>,
    primary_window_query: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut scroll_views: Query<&mut UiScrollView>,
    scroll_markers: Query<(), With<UiScrollView>>,
    parents: Query<&ChildOf>,
    ui_events: Res<UiEventQueue>,
) {
    let Some(runtime) = runtime else {
        return;
    };

    let Some((primary_window_entity, primary_window)) = primary_window_query.iter().next() else {
        return;
    };

    let Some(cursor_pos) = primary_window.physical_cursor_position() else {
        return;
    };

    for wheel in wheel_events.read() {
        if wheel.window != primary_window_entity {
            continue;
        }

        let hit_path = runtime.get_hit_path((cursor_pos.x as f64, cursor_pos.y as f64).into());

        let scroll_targets = collect_scroll_view_targets_from_hit_path(
            &runtime,
            &hit_path,
            &parents,
            &scroll_markers,
        );

        if scroll_targets.is_empty() {
            continue;
        }

        let factor = if wheel.unit == MouseScrollUnit::Line {
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            1.0
        };

        for scroll_entity in scroll_targets {
            let Ok(mut scroll_view) = scroll_views.get_mut(scroll_entity) else {
                continue;
            };

            let before = scroll_view.scroll_offset;

            if scroll_view.show_horizontal_scrollbar {
                scroll_view.scroll_offset.x -= wheel.x * factor;
            }
            if scroll_view.show_vertical_scrollbar {
                scroll_view.scroll_offset.y -= wheel.y * factor;
            }

            clamp_scroll_offset_strict(&mut scroll_view);

            let after = scroll_view.scroll_offset;
            if after != before {
                ui_events.push_typed(
                    scroll_entity,
                    UiScrollViewChanged {
                        scroll_view: scroll_entity,
                        scroll_offset: after,
                    },
                );
                break;
            }
        }
    }
}

/// Advance all [`AutoDismiss`] timers and despawn finished entities.
pub fn tick_auto_dismiss(
    mut commands: Commands,
    mut auto_dismiss_entities: Query<(Entity, &mut AutoDismiss)>,
    time: Res<Time>,
) {
    let delta = time.delta();

    for (entity, mut auto_dismiss) in &mut auto_dismiss_entities {
        auto_dismiss.timer.tick(delta);
        if auto_dismiss.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Backward-compatible alias retained for existing call sites.
pub fn tick_toasts(
    commands: Commands,
    auto_dismiss_entities: Query<(Entity, &mut AutoDismiss)>,
    time: Res<Time>,
) {
    tick_auto_dismiss(commands, auto_dismiss_entities, time);
}

/// Spawn or despawn tooltip overlay entities in response to hover state changes.
///
/// When an entity that carries [`HasTooltip`] becomes hovered (`InteractionState.hovered = true`) a
/// [`UiTooltip`] overlay is spawned under [`UiOverlayRoot`] anchored to that
/// entity. When the entity is no longer hovered, all tooltip overlays
/// anchored to it are despawned.
pub fn handle_tooltip_hovers(
    mut commands: Commands,
    overlay_root: Query<Entity, With<UiOverlayRoot>>,
    tooltip_sources: Query<(Entity, &HasTooltip, Option<&InteractionState>)>,
    existing_tooltips: Query<(Entity, &UiTooltip)>,
) {
    let Ok(root) = overlay_root.single() else {
        return;
    };

    let mut hovered_sources = HashSet::new();
    for (entity, _has_tooltip, state) in &tooltip_sources {
        if state.is_some_and(|state| state.hovered) {
            hovered_sources.insert(entity);
        }
    }

    // Spawn missing tooltips for hovered sources.
    for (entity, has_tooltip, state) in &tooltip_sources {
        if !state.is_some_and(|state| state.hovered) {
            continue;
        }

        let already_spawned = existing_tooltips
            .iter()
            .any(|(_, tooltip)| tooltip.anchor == entity);
        if already_spawned {
            continue;
        }

        commands.spawn((
            UiTooltip {
                text: has_tooltip.text.clone(),
                anchor: entity,
            },
            AnchoredTo(entity),
            OverlayAnchorRect::default(),
            OverlayConfig {
                placement: OverlayPlacement::Top,
                anchor: Some(entity),
                auto_flip: true,
            },
            OverlayState {
                is_modal: false,
                anchor: Some(entity),
            },
            OverlayComputedPosition::default(),
            ChildOf(root),
        ));
    }

    // Despawn tooltips whose source is no longer hovered (or no longer exists / has tooltip).
    for (tooltip_entity, tooltip) in &existing_tooltips {
        if !hovered_sources.contains(&tooltip.anchor) {
            commands.entity(tooltip_entity).despawn();
        }
    }
}

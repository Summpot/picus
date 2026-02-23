use bevy_ecs::{entity::Entity, hierarchy::ChildOf, message::MessageReader, prelude::*};
use bevy_input::mouse::{MouseScrollUnit, MouseWheel};
use bevy_time::Time;
use bevy_window::{PrimaryWindow, Window};

use crate::{
    AnchoredTo, HasTooltip, Hovered, MasonryRuntime, OverlayAnchorRect, OverlayComputedPosition,
    OverlayConfig, OverlayPlacement, OverlayState, ScrollAxis, UiCheckbox, UiCheckboxChanged,
    UiOverlayRoot, UiRadioGroup, UiRadioGroupChanged, UiScrollView, UiScrollViewChanged, UiSlider,
    UiSliderChanged, UiSwitch, UiSwitchChanged, UiTabBar, UiTabChanged, UiTextInput,
    UiTextInputChanged, UiToast, UiTooltip, UiTreeNode, UiTreeNodeToggled, events::UiEventQueue,
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
    /// Adjust slider value using step increments.
    StepSlider { slider: Entity, delta: f64 },
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
) -> f64 {
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

    delta_pixels * (max_scroll / travel)
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

fn resolve_scroll_view_target_from_hit_path(
    runtime: &MasonryRuntime,
    hit_path: &[masonry::core::WidgetId],
    parents: &Query<&ChildOf>,
    scroll_markers: &Query<(), With<UiScrollView>>,
) -> Option<Entity> {
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
                return Some(entity);
            }

            let Ok(parent) = parents.get(entity) else {
                break;
            };
            entity = parent.parent();
        }
    }

    None
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

                let toggled = if let Some(tree_node) = world.get::<UiTreeNode>(node) {
                    Some(!tree_node.is_expanded)
                } else {
                    None
                };

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

                if let Some(mut checkbox_state) = world.get_mut::<UiCheckbox>(checkbox) {
                    checkbox_state.checked = !checkbox_state.checked;
                    let checked = checkbox_state.checked;
                    drop(checkbox_state);
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
                    let next = (slider_state.value + delta * step)
                        .clamp(slider_state.min, slider_state.max);
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

                if let Some(mut switch_state) = world.get_mut::<UiSwitch>(switch) {
                    switch_state.on = !switch_state.on;
                    let on = switch_state.on;
                    drop(switch_state);
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

                if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(scroll_entity) {
                    let before = scroll_view.scroll_offset;
                    let delta =
                        scroll_delta_from_thumb_drag(*scroll_view, axis, delta_pixels) as f32;

                    match axis {
                        ScrollAxis::Horizontal => {
                            scroll_view.scroll_offset.x += delta;
                        }
                        ScrollAxis::Vertical => {
                            scroll_view.scroll_offset.y += delta;
                        }
                    }

                    scroll_view.clamp_scroll_offset();
                    let after = scroll_view.scroll_offset;
                    drop(scroll_view);

                    if after != before {
                        world.resource::<UiEventQueue>().push_typed(
                            scroll_entity,
                            UiScrollViewChanged {
                                scroll_view: scroll_entity,
                                scroll_offset: after,
                            },
                        );
                    }
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
    runtime: Option<NonSendMut<MasonryRuntime>>,
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

        let Some(scroll_entity) = resolve_scroll_view_target_from_hit_path(
            &runtime,
            &hit_path,
            &parents,
            &scroll_markers,
        ) else {
            continue;
        };

        let Ok(mut scroll_view) = scroll_views.get_mut(scroll_entity) else {
            continue;
        };

        let factor = if wheel.unit == MouseScrollUnit::Line {
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            1.0
        } as f32;

        let before = scroll_view.scroll_offset;

        if scroll_view.show_horizontal_scrollbar {
            scroll_view.scroll_offset.x -= wheel.x * factor;
        }
        if scroll_view.show_vertical_scrollbar {
            scroll_view.scroll_offset.y -= wheel.y * factor;
        }
        scroll_view.clamp_scroll_offset();

        let after = scroll_view.scroll_offset;
        if after != before {
            ui_events.push_typed(
                scroll_entity,
                UiScrollViewChanged {
                    scroll_view: scroll_entity,
                    scroll_offset: after,
                },
            );
        }
    }
}

/// Advance toast display timers and despawn any toasts whose duration has elapsed.
///
/// Toasts with `duration_secs == 0.0` are persistent and must be dismissed
/// manually via [`crate::OverlayUiAction::DismissToast`].
pub fn tick_toasts(
    mut commands: Commands,
    mut toasts: Query<(Entity, &mut UiToast)>,
    time: Res<Time>,
) {
    let delta = time.delta_secs();

    for (entity, mut toast) in &mut toasts {
        if toast.duration_secs <= 0.0 {
            continue;
        }

        toast.elapsed_secs += delta;

        if toast.elapsed_secs >= toast.duration_secs {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawn or despawn tooltip overlay entities in response to hover state changes.
///
/// When an entity that carries [`HasTooltip`] gains the [`Hovered`] marker a
/// [`UiTooltip`] overlay is spawned under [`UiOverlayRoot`] anchored to that
/// entity.  When the entity loses the [`Hovered`] marker all tooltip overlays
/// anchored to it are despawned.
pub fn handle_tooltip_hovers(
    mut commands: Commands,
    overlay_root: Query<Entity, With<UiOverlayRoot>>,
    just_hovered: Query<(Entity, &HasTooltip), Added<Hovered>>,
    existing_tooltips: Query<(Entity, &UiTooltip)>,
    mut removed_hover: RemovedComponents<Hovered>,
) {
    // Spawn new tooltips for freshly hovered entities.
    if let Ok(root) = overlay_root.single() {
        for (entity, has_tooltip) in &just_hovered {
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
    }

    // Despawn tooltips whose source entity is no longer hovered.
    let unhovered: Vec<Entity> = removed_hover.read().collect();
    for source in unhovered {
        for (tooltip_entity, tooltip) in &existing_tooltips {
            if tooltip.anchor == source {
                commands.entity(tooltip_entity).despawn();
            }
        }
    }
}

//! Drag-and-drop framework for picus.
//!
//! Provides ECS components for drag sources and drop targets, plus systems
//! that track drag operations via pointer events and dispatch typed
//! [`DragEvent`] components to affected entities.

use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window};

use crate::{
    events::{UiEventQueue, UiPointerHitEvent, UiPointerPhase},
    runtime::MasonryRuntime,
};

/// Drag data type identifier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DragDataType {
    #[default]
    Text,
    File,
    Custom(&'static str),
}

/// Payload carried during a drag operation.
#[derive(Debug, Clone)]
pub enum DragData {
    Text(String),
    File(Vec<String>),
    Custom(Entity, String),
}

impl Default for DragData {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// Visual preview configuration for drag ghost.
#[derive(Debug, Clone)]
pub struct DragPreview {
    pub opacity: f32,
    pub follow_cursor: bool,
    pub offset: (f64, f64),
}

impl Default for DragPreview {
    fn default() -> Self {
        Self {
            opacity: 0.8,
            follow_cursor: true,
            offset: (0.0, 0.0),
        }
    }
}

/// Component marking an entity as a drag source.
#[derive(Component, Debug, Clone)]
pub struct DragSource {
    pub can_drag: bool,
    pub drag_data: Option<DragData>,
    pub drag_preview: Option<DragPreview>,
}

impl Default for DragSource {
    fn default() -> Self {
        Self {
            can_drag: true,
            drag_data: None,
            drag_preview: None,
        }
    }
}

/// Component marking an entity as a drop target.
#[derive(Component, Debug, Clone)]
pub struct DropTarget {
    pub allow_drop: bool,
    pub accepted_types: Vec<DragDataType>,
}

impl Default for DropTarget {
    fn default() -> Self {
        Self {
            allow_drop: true,
            accepted_types: Vec::new(),
        }
    }
}

/// Event component attached to entities during drag operations.
#[derive(Component, Debug, Clone)]
pub enum DragEvent {
    /// Emitted on the source entity when a drag begins.
    DragStarting { data: DragData },
    /// Emitted on a target entity when the drag enters its area.
    DragEnter { source: Entity, data: DragData },
    /// Emitted while the drag hovers over a target.
    DragOver {
        source: Entity,
        data: DragData,
        position: (f64, f64),
    },
    /// Emitted on the previous target when the drag leaves its area.
    DragLeave { source: Entity },
    /// Emitted on the final target when the drag is released over it.
    Drop {
        source: Entity,
        data: DragData,
        position: (f64, f64),
    },
    /// Emitted on the source when the drag ends.
    DragCompleted { dropped: bool },
}

/// Global drag operation state.
#[derive(Resource, Debug, Default)]
pub struct DragState {
    pub is_dragging: bool,
    pub source_entity: Option<Entity>,
    pub drag_data: Option<DragData>,
    pub current_target: Option<Entity>,
    pub drag_position: (f64, f64),
}

/// Parse entity bits from a Masonry widget's debug-text encoding.
///
/// Widgets embed the owning ECS entity id as a visible prefix so that
/// hit-testing round-trips work without a separate spatial index.
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

/// Find the ECS entity under a physical cursor position by hit-testing the
/// Masonry retained widget tree.
fn entity_at_physical_position(
    runtime: &MasonryRuntime,
    physical_x: f64,
    physical_y: f64,
) -> Option<Entity> {
    let window_runtime = runtime.primary()?;
    // get_hit_path accepts physical coordinates and converts to logical
    // using the runtime's internal scale factor.
    let hit_path =
        window_runtime.get_hit_path(crate::masonry_core::kurbo::Point::new(physical_x, physical_y));

    // The last element in the hit path is the deepest widget under the pointer.
    hit_path.last().and_then(|widget_id| {
        window_runtime
            .render_root
            .get_widget(*widget_id)
            .and_then(|widget| widget.get_debug_text())
            .and_then(|debug| parse_entity_bits_from_debug(&debug))
            .and_then(Entity::try_from_bits)
    })
}

/// Track drag state from [`UiPointerHitEvent`] events.
///
/// - **Pressed** on a [`DragSource`] → starts a drag.
/// - **Released** while dragging → completes the drop.
/// - All events update the cursor position.
///
/// Events that are **not** consumed by the active drag operation are re-pushed
/// into the queue so that [`bubble_ui_pointer_events`] continues normal
/// pointer-event bubbling.
#[allow(clippy::collapsible_if)]
pub fn track_drag_state(
    mut event_queue: ResMut<UiEventQueue>,
    mut drag_state: ResMut<DragState>,
    drag_sources: Query<&DragSource>,
    drop_targets: Query<&DropTarget>,
    mut commands: Commands,
) {
    let hits = event_queue.drain_actions::<UiPointerHitEvent>();

    for hit in hits {
        let entity = hit.action.target;
        let event = hit.action;

        // --- Track cursor position for ongoing drags ---
        if drag_state.is_dragging {
            drag_state.drag_position = event.position;
        }

        // --- Pressed + DragSource: start a new drag ---
        if event.phase == UiPointerPhase::Pressed && !drag_state.is_dragging {
            if let Ok(source) = drag_sources.get(entity) {
                if source.can_drag {
                    if let Some(data) = source.drag_data.clone() {
                        drag_state.is_dragging = true;
                        drag_state.source_entity = Some(entity);
                        drag_state.drag_data = Some(data.clone());
                        drag_state.drag_position = event.position;
                        drag_state.current_target = None;

                        commands
                            .entity(entity)
                            .insert(DragEvent::DragStarting { data: data.clone() });

                        // Consumed – skip re-push.
                        continue;
                    }
                }
            }
        }

        // --- While dragging: update the hover target ---
        if drag_state.is_dragging {
            let under_pointer = entity;
            let is_drop_target = drop_targets.get(under_pointer).is_ok_and(|t| t.allow_drop);

            if is_drop_target {
                let previous = drag_state.current_target.replace(under_pointer);
                match previous {
                    Some(prev) if prev != under_pointer => {
                        if let Some(source) = drag_state.source_entity {
                            commands
                                .entity(prev)
                                .insert(DragEvent::DragLeave { source });
                        }
                        if let Some(data) = drag_state.drag_data.clone() {
                            if let Some(source) = drag_state.source_entity {
                                commands
                                    .entity(under_pointer)
                                    .insert(DragEvent::DragEnter { source, data });
                            }
                        }
                    }
                    Some(_) => {
                        // Same target – fire DragOver.
                        if let Some(data) = drag_state.drag_data.clone() {
                            if let Some(source) = drag_state.source_entity {
                                commands.entity(under_pointer).insert(DragEvent::DragOver {
                                    source,
                                    data,
                                    position: event.position,
                                });
                            }
                        }
                    }
                    None => {
                        // First target entered.
                        if let Some(data) = drag_state.drag_data.clone() {
                            if let Some(source) = drag_state.source_entity {
                                commands
                                    .entity(under_pointer)
                                    .insert(DragEvent::DragEnter { source, data });
                            }
                        }
                    }
                }
            } else if let Some(prev) = drag_state.current_target.take() {
                if let Some(source) = drag_state.source_entity {
                    commands
                        .entity(prev)
                        .insert(DragEvent::DragLeave { source });
                }
            }

            // --- Released while dragging: complete the drop ---
            if event.phase == UiPointerPhase::Released {
                let dropped = drag_state.current_target.is_some();

                if let Some(target) = drag_state.current_target {
                    if let (Some(data), Some(source)) =
                        (drag_state.drag_data.clone(), drag_state.source_entity)
                    {
                        commands.entity(target).insert(DragEvent::Drop {
                            source,
                            data,
                            position: event.position,
                        });
                    }
                }

                if let Some(source) = drag_state.source_entity {
                    commands
                        .entity(source)
                        .insert(DragEvent::DragCompleted { dropped });
                }

                // Reset drag state.
                *drag_state = DragState::default();

                // Release consumed – skip re-push.
                continue;
            }

            // All other events during a drag are consumed so that normal
            // pointer bubbling is suppressed until the drag finishes.
            continue;
        }

        // --- Events not consumed by the drag system: re-push for normal
        //     bubbling by `bubble_ui_pointer_events`. ---
        event_queue.push_typed(entity, event);
    }
}

/// Dispatch drag hover events every frame while a drag is active.
///
/// This system performs hit-testing through the Masonry retained widget tree
/// to determine which entity is under the cursor. It compares against the
/// last known hover target in [`DragState`] and inserts [`DragEvent`]
/// components (Enter / Leave / Over) as needed.
///
/// Because `track_drag_state` already dispatches enter / leave / over directly
/// from pointer events, this system serves as a **fallback** for frames where
/// the cursor moves but no button event is fired (e.g. pure mouse-move during
/// a drag).
#[allow(clippy::let_and_return)]
pub fn dispatch_drag_events(
    mut drag_state: ResMut<DragState>,
    mut commands: Commands,
    runtime: Option<NonSend<MasonryRuntime>>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
    drop_targets: Query<&DropTarget>,
) {
    if !drag_state.is_dragging {
        return;
    }

    let Some(runtime) = runtime.as_ref() else {
        return;
    };
    let Ok(window) = primary_window_query.single() else {
        return;
    };
    let Some(cursor_pos) = window.physical_cursor_position() else {
        return;
    };

    // Hit-test at the physical cursor position.
    let hit_entity = entity_at_physical_position(runtime, cursor_pos.x as f64, cursor_pos.y as f64);

    let previous_target = drag_state.current_target;

    if let Some(target_entity) = hit_entity {
        if drop_targets.get(target_entity).is_ok_and(|t| t.allow_drop) {
            let source = drag_state.source_entity.unwrap();
            let data = drag_state.drag_data.clone().unwrap();

            if previous_target != Some(target_entity) {
                // Left previous target (if any).
                if let Some(old) = previous_target {
                    commands.entity(old).insert(DragEvent::DragLeave { source });
                }
                // Entered new target.
                commands
                    .entity(target_entity)
                    .insert(DragEvent::DragEnter { source, data });
                drag_state.current_target = Some(target_entity);
            } else {
                // Still over the same target.
                commands.entity(target_entity).insert(DragEvent::DragOver {
                    source,
                    data,
                    position: (cursor_pos.x as f64, cursor_pos.y as f64),
                });
            }
        } else if let Some(old) = previous_target {
            // Under pointer is not a drop target – leave the old one.
            let source = drag_state.source_entity.unwrap();
            commands.entity(old).insert(DragEvent::DragLeave { source });
            drag_state.current_target = None;
        }
    } else if let Some(old) = previous_target {
        // No entity under pointer – leave old target.
        let source = drag_state.source_entity.unwrap();
        commands.entity(old).insert(DragEvent::DragLeave { source });
        drag_state.current_target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_entity_bits_from_debug
    // -----------------------------------------------------------------------

    #[test]
    fn parse_opaque_hitbox_entity() {
        let bits = parse_entity_bits_from_debug("opaque_hitbox_entity=42");
        assert_eq!(bits, Some(42));
    }

    #[test]
    fn parse_entity_scope_prefix() {
        let bits = parse_entity_bits_from_debug("entity_scope=999");
        assert_eq!(bits, Some(999));
    }

    #[test]
    fn parse_entity_prefix() {
        let bits = parse_entity_bits_from_debug("entity=123456789");
        assert_eq!(bits, Some(123456789));
    }

    #[test]
    fn parse_unrecognized_debug_returns_none() {
        let bits = parse_entity_bits_from_debug("Button");
        assert_eq!(bits, None);
    }

    #[test]
    fn parse_malformed_bits_returns_none() {
        let bits = parse_entity_bits_from_debug("entity=not-a-number");
        assert_eq!(bits, None);
    }

    #[test]
    fn parse_empty_string_returns_none() {
        let bits = parse_entity_bits_from_debug("");
        assert_eq!(bits, None);
    }

    // -----------------------------------------------------------------------
    // Default impls
    // -----------------------------------------------------------------------

    #[test]
    fn drag_data_default_is_text_empty() {
        let data = DragData::default();
        match data {
            DragData::Text(s) => assert_eq!(s, ""),
            _ => panic!("expected DragData::Text"),
        }
    }

    #[test]
    fn drag_preview_default_values() {
        let preview = DragPreview::default();
        assert!((preview.opacity - 0.8).abs() < f32::EPSILON);
        assert!(preview.follow_cursor);
        assert_eq!(preview.offset, (0.0, 0.0));
    }

    #[test]
    fn drag_source_default_can_drag() {
        let source = DragSource::default();
        assert!(source.can_drag);
        assert!(source.drag_data.is_none());
        assert!(source.drag_preview.is_none());
    }

    #[test]
    fn drop_target_default_accepts_all() {
        let target = DropTarget::default();
        assert!(target.allow_drop);
        assert!(target.accepted_types.is_empty());
    }

    #[test]
    fn drag_state_default_is_not_dragging() {
        let state = DragState::default();
        assert!(!state.is_dragging);
        assert!(state.source_entity.is_none());
        assert!(state.drag_data.is_none());
        assert!(state.current_target.is_none());
    }

    #[test]
    fn drag_data_type_default_is_text() {
        assert_eq!(DragDataType::default(), DragDataType::Text);
    }

    #[test]
    fn drag_event_debug_format() {
        let event = DragEvent::DragStarting {
            data: DragData::Text("hello".into()),
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("DragStarting"));
        assert!(debug.contains("hello"));
    }

    #[test]
    fn drag_event_drop_includes_src_data_position() {
        let src = Entity::from_bits(1);
        let event = DragEvent::Drop {
            source: src,
            data: DragData::File(vec!["a.txt".into()]),
            position: (100.0, 200.0),
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("100.0"));
    }

    // -----------------------------------------------------------------------
    // DragState lifecycle simulation
    // -----------------------------------------------------------------------

    /// Simulate the logic of `track_drag_state` for a complete drag cycle.
    #[test]
    fn drag_cycle_start_over_drop() {
        let mut state = DragState::default();
        let source = Entity::from_bits(10);
        let target = Entity::from_bits(20);

        // Start: press on drag source
        assert!(!state.is_dragging);
        state.is_dragging = true;
        state.source_entity = Some(source);
        state.drag_data = Some(DragData::Text("payload".into()));
        state.drag_position = (0.0, 0.0);

        // Move over target
        state.current_target = Some(target);
        assert_eq!(state.current_target, Some(target));

        // Release over target -> drop
        let dropped = state.current_target.is_some();
        assert!(dropped);

        // Reset
        state = DragState::default();
        assert!(!state.is_dragging);
        assert!(state.source_entity.is_none());
    }

    #[test]
    fn drag_cycle_cancel_by_outside_release() {
        let state = DragState {
            is_dragging: true,
            source_entity: Some(Entity::from_bits(10)),
            drag_data: Some(DragData::Text("payload".into())),
            current_target: None,
            drag_position: (0.0, 0.0),
        };

        // Release outside any target
        let dropped = state.current_target.is_some();
        assert!(!dropped, "no target -> not dropped");

        let state = DragState::default();
        assert!(!state.is_dragging);
    }

    #[test]
    fn drag_source_accepts_custom_data_type() {
        let t = DragDataType::Custom("my-app/type");
        match t {
            DragDataType::Custom(name) => assert_eq!(name, "my-app/type"),
            _ => panic!("expected Custom"),
        }
    }
}

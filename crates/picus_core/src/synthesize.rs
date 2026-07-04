use std::{collections::HashMap, sync::Arc};

use bevy_ecs::{hierarchy::Children, prelude::*};
use bevy_window::PrimaryWindow;
use picus_view::view::{FlexExt as _, flex_col, label};

use crate::{
    ecs::{UiOverlayRoot, UiRoot, UiWindow},
    projection::{UiProjectorRegistry, UiView},
    runtime::MasonryRuntime,
    styling::InteractionState,
    retained_bridge::entity_scope,
};

/// Snapshot containing synthesized views for the current frame, grouped by
/// the Bevy window entity each root is bound to.
///
/// Roots without an explicit [`UiWindow`] binding are grouped under the
/// primary window (or the first attached window runtime when no primary
/// window exists).
#[derive(Resource, Default)]
pub struct SynthesizedUiViews {
    /// Per-window composed root views.
    pub windows: HashMap<Entity, UiView>,
}

/// Snapshot metrics for the latest synthesis pass.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiSynthesisStats {
    pub root_count: usize,
    pub node_count: usize,
    pub cycle_count: usize,
    pub missing_entity_count: usize,
    pub unhandled_count: usize,
}

/// Collect all entities marked with [`UiRoot`], grouped by their target window.
///
/// Returns a map of `window_entity -> Vec<root_entity>` with deterministic
/// ordering: overlays are sorted after content roots within each window, and
/// roots are ordered by entity bits.
pub fn gather_ui_roots_by_window(world: &mut World) -> HashMap<Entity, Vec<Entity>> {
    let runtime_window_entities: Vec<Entity> = world
        .get_non_send::<MasonryRuntime>()
        .map(|runtime| runtime.window_entities().collect::<Vec<_>>())
        .unwrap_or_default();

    let primary_window_entity = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next();

    let mut entries: Vec<(Entity, Entity, bool)> = {
        let mut query = world
            .query_filtered::<(Entity, Option<&UiOverlayRoot>, Option<&UiWindow>), With<UiRoot>>();
        query
            .iter(world)
            .map(|(entity, overlay, binding)| {
                let is_overlay = overlay.is_some();
                let window = binding.map(|b| b.0).or(primary_window_entity).unwrap_or(
                    runtime_window_entities
                        .first()
                        .copied()
                        .unwrap_or(Entity::PLACEHOLDER),
                );
                (window, entity, is_overlay)
            })
            .collect::<Vec<_>>()
    };

    // Group by window.
    let mut grouped: HashMap<Entity, Vec<(Entity, bool)>> = HashMap::new();
    for (window, entity, is_overlay) in entries.drain(..) {
        grouped
            .entry(window)
            .or_default()
            .push((entity, is_overlay));
    }

    // Deterministic ordering within each window: overlays last, then by entity bits.
    let mut result: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for (window, mut roots) in grouped {
        roots.sort_by_key(|(entity, is_overlay)| (*is_overlay, entity.to_bits()));
        result.insert(
            window,
            roots.into_iter().map(|(entity, _)| entity).collect(),
        );
    }

    result
}

/// Collect all entities marked with [`UiRoot`] (flattened, for backward
/// compatibility with callers that do not need per-window grouping).
pub fn gather_ui_roots(world: &mut World) -> Vec<Entity> {
    gather_ui_roots_by_window(world)
        .into_values()
        .flatten()
        .collect()
}

/// Synthesize Xilem Masonry views and stats for provided roots.
pub fn synthesize_roots_with_stats(
    world: &World,
    registry: &UiProjectorRegistry,
    roots: impl IntoIterator<Item = Entity>,
) -> (Vec<UiView>, UiSynthesisStats) {
    let roots = roots.into_iter().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(roots.len());
    let mut stats = UiSynthesisStats {
        root_count: roots.len(),
        ..UiSynthesisStats::default()
    };
    let mut visiting = Vec::new();

    for root in roots {
        output.push(synthesize_entity(
            world,
            registry,
            root,
            &mut visiting,
            &mut stats,
        ));
    }

    (output, stats)
}

/// Synthesize Xilem Masonry views for provided roots.
pub fn synthesize_roots(
    world: &World,
    registry: &UiProjectorRegistry,
    roots: impl IntoIterator<Item = Entity>,
) -> Vec<UiView> {
    synthesize_roots_with_stats(world, registry, roots).0
}

/// Synthesize by auto-discovering all [`UiRoot`] entities.
pub fn synthesize_world(world: &mut World, registry: &UiProjectorRegistry) -> Vec<UiView> {
    let roots = gather_ui_roots(world);
    synthesize_roots(world, registry, roots)
}

fn synthesize_entity(
    world: &World,
    registry: &UiProjectorRegistry,
    entity: Entity,
    visiting: &mut Vec<Entity>,
    stats: &mut UiSynthesisStats,
) -> UiView {
    if world.get_entity(entity).is_err() {
        stats.node_count += 1;
        stats.missing_entity_count += 1;
        return Arc::new(label(format!("[missing entity {entity:?}]")));
    }

    if visiting.contains(&entity) {
        stats.node_count += 1;
        stats.cycle_count += 1;
        return Arc::new(label(format!("[cycle at {entity:?}]")));
    }

    visiting.push(entity);

    let child_entities = world
        .get::<Children>(entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();

    let children = child_entities
        .into_iter()
        .map(|child| synthesize_entity(world, registry, child, visiting, stats))
        .collect::<Vec<_>>();

    let node_id = entity.to_bits();

    let projected = registry.project_node(world, entity, node_id, children.clone());

    let base_view: UiView = if let Some(view) = projected {
        view
    } else {
        stats.unhandled_count += 1;
        let mut seq = Vec::with_capacity(children.len() + 1);
        seq.push(label(format!("[unhandled entity {entity:?}]")).into_any_flex());
        seq.extend(children.into_iter().map(|child| child.into_any_flex()));
        Arc::new(flex_col(seq))
    };

    let view: UiView = Arc::new(entity_scope(entity, base_view));

    stats.node_count += 1;

    let popped = visiting.pop();
    debug_assert_eq!(popped, Some(entity));

    view
}

/// Sync focused widget from each window's Masonry runtime back to ECS
/// [`InteractionState`].
pub fn sync_focus_state(world: &mut World) {
    let window_focused_bits: Vec<(Entity, Option<u64>)> = {
        let Some(mut runtime) = world.get_non_send_mut::<MasonryRuntime>() else {
            return;
        };
        let window_entities: Vec<Entity> = runtime.window_entities().collect();
        window_entities
            .into_iter()
            .map(|window_entity| {
                let Some(window_runtime) = runtime.window_mut(window_entity) else {
                    return (window_entity, None);
                };
                let focused_id = window_runtime.render_root.focused_widget();
                window_runtime.populate_entity_map();
                let bits =
                    focused_id.and_then(|id| window_runtime.widget_id_to_entity.get(&id).copied());
                (window_entity, bits)
            })
            .collect()
    };

    let entity_ids: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<InteractionState>>();
        query.iter(world).collect()
    };

    let all_focused_bits: Vec<u64> = window_focused_bits
        .iter()
        .filter_map(|(_, bits)| *bits)
        .collect();

    for entity in entity_ids {
        if let Some(mut state) = world.get_mut::<InteractionState>(entity) {
            let should_be_focused = all_focused_bits
                .iter()
                .any(|bits| entity.to_bits() == *bits);
            if state.focused != should_be_focused {
                state.focused = should_be_focused;
            }
        }
    }
}

pub fn synthesize_ui(world: &mut World) {
    if !world.contains_non_send::<crate::runtime::MasonryRuntime>()
        || !world.contains_resource::<UiProjectorRegistry>()
        || !world.contains_resource::<SynthesizedUiViews>()
        || !world.contains_resource::<UiSynthesisStats>()
    {
        return;
    }

    let roots_by_window = gather_ui_roots_by_window(world);

    let mut stats = UiSynthesisStats::default();

    let mut windows: HashMap<Entity, UiView> = HashMap::new();
    for (window_entity, roots) in roots_by_window {
        let (synthesized, window_stats) =
            world.resource_scope(|world, registry: Mut<UiProjectorRegistry>| {
                synthesize_roots_with_stats(world, &registry, roots)
            });
        windows.insert(window_entity, compose_window_root(&synthesized));
        stats.root_count += window_stats.root_count;
        stats.node_count += window_stats.node_count;
        stats.cycle_count += window_stats.cycle_count;
        stats.missing_entity_count += window_stats.missing_entity_count;
        stats.unhandled_count += window_stats.unhandled_count;
    }

    world.resource_mut::<SynthesizedUiViews>().windows = windows;
    *world.resource_mut::<UiSynthesisStats>() = stats;
}

/// Compose a single window's root view from its set of synthesized roots.
///
/// Mirrors the previous single-window zstack composition so overlays sort
/// last and content fills the viewport.
fn compose_window_root(roots: &[UiView]) -> UiView {
    use crate::runtime::compose_runtime_root as compose;
    compose(roots)
}

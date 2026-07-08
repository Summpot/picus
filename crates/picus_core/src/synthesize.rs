use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy_ecs::{hierarchy::Children, prelude::*};
use bevy_window::PrimaryWindow;
use picus_view::view::{FlexExt as _, flex_col, label};

use crate::{
    ecs::{
        AnchoredTo, LocalizeText, OverlayAnchorRect, OverlayComputedPosition, OverlayConfig,
        OverlayStack, OverlayState, TypographyPreset, UiOverlayRoot, UiRoot, UiWindow,
    },
    i18n::AppI18n,
    projection::{UiProjectorRegistry, UiView},
    resize::{AppBreakpoints, WindowSize},
    retained_bridge::entity_scope,
    runtime::MasonryRuntime,
    styling::{ActiveStyleVariant, ComputedStyle, CurrentColorStyle, InteractionState, StyleSheet},
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
    pub(crate) dirty_windows: HashSet<Entity>,
    pub(crate) roots_by_window: HashMap<Entity, Vec<Entity>>,
    pub(crate) entities_by_window: HashMap<Entity, HashSet<Entity>>,
    pub(crate) entity_windows: HashMap<Entity, Entity>,
    pub(crate) stats_by_window: HashMap<Entity, UiSynthesisStats>,
    pub(crate) generation: u64,
}

impl SynthesizedUiViews {
    pub(crate) fn remove_window(&mut self, window: Entity) {
        self.windows.remove(&window);
        self.dirty_windows.remove(&window);
        self.roots_by_window.remove(&window);
        self.stats_by_window.remove(&window);

        if let Some(entities) = self.entities_by_window.remove(&window) {
            for entity in entities {
                self.entity_windows.remove(&entity);
            }
        }
    }
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

impl UiSynthesisStats {
    fn add_assign(&mut self, rhs: &Self) {
        self.root_count += rhs.root_count;
        self.node_count += rhs.node_count;
        self.cycle_count += rhs.cycle_count;
        self.missing_entity_count += rhs.missing_entity_count;
        self.unhandled_count += rhs.unhandled_count;
    }
}

#[derive(Debug, Default)]
pub(crate) struct UiProjectionInvalidationSnapshot {
    all: bool,
    windows: HashSet<Entity>,
    roots: HashSet<Entity>,
}

/// Explicit invalidation queue for projection dependencies that cannot be
/// inferred from ECS component/resource change detection.
#[derive(Resource, Debug, Default)]
pub struct UiProjectionInvalidation {
    all: bool,
    windows: HashSet<Entity>,
    roots: HashSet<Entity>,
}

impl UiProjectionInvalidation {
    /// Rebuild all synthesized window roots on the next projection pass.
    pub fn request_all(&mut self) {
        self.all = true;
    }

    /// Rebuild a specific Bevy window's retained root on the next projection pass.
    pub fn request_window(&mut self, window: Entity) {
        self.windows.insert(window);
    }

    /// Rebuild the window containing `root` on the next projection pass.
    pub fn request_root(&mut self, root: Entity) {
        self.roots.insert(root);
    }

    fn take(&mut self) -> UiProjectionInvalidationSnapshot {
        UiProjectionInvalidationSnapshot {
            all: std::mem::take(&mut self.all),
            windows: std::mem::take(&mut self.windows),
            roots: std::mem::take(&mut self.roots),
        }
    }
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
    let (views, stats, _entities) =
        synthesize_roots_with_stats_and_entities(world, registry, roots);
    (views, stats)
}

fn synthesize_roots_with_stats_and_entities(
    world: &World,
    registry: &UiProjectorRegistry,
    roots: impl IntoIterator<Item = Entity>,
) -> (Vec<UiView>, UiSynthesisStats, HashSet<Entity>) {
    let roots = roots.into_iter().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(roots.len());
    let mut stats = UiSynthesisStats {
        root_count: roots.len(),
        ..UiSynthesisStats::default()
    };
    let mut entities = HashSet::new();
    let mut visiting = Vec::new();

    for root in roots {
        output.push(synthesize_entity(
            world,
            registry,
            root,
            &mut visiting,
            &mut stats,
            &mut entities,
        ));
    }

    (output, stats, entities)
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
    entities: &mut HashSet<Entity>,
) -> UiView {
    entities.insert(entity);

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
        .map(|child| synthesize_entity(world, registry, child, visiting, stats, entities))
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
        || !world.contains_resource::<UiProjectionInvalidation>()
    {
        return;
    }

    let mut roots_by_window = gather_ui_roots_by_window(world);
    if let Some(runtime) = world.get_non_send::<MasonryRuntime>() {
        for window in runtime.window_entities() {
            roots_by_window.entry(window).or_default();
        }
    }

    let dirty_entities = world.resource_scope(|world, mut registry: Mut<UiProjectorRegistry>| {
        let dirty = registry.drain_dirty_entities(world);
        let untracked_projectors = registry.has_untracked_projectors();
        if untracked_projectors {
            tracing::trace!("raw projection projectors are registered; forcing synthesis");
        }
        (dirty, untracked_projectors)
    });
    let (mut dirty_entities, has_untracked_projectors) = dirty_entities;

    let projection_resources_changed = projection_resources_changed(world);
    let invalidation = world.resource_mut::<UiProjectionInvalidation>().take();

    let mut dirty_windows = HashSet::new();
    let all_windows = {
        let views = world.resource::<SynthesizedUiViews>();
        roots_by_window
            .keys()
            .copied()
            .chain(views.windows.keys().copied())
            .collect::<HashSet<_>>()
    };

    {
        let views = world.resource::<SynthesizedUiViews>();
        if views.generation == 0
            || invalidation.all
            || projection_resources_changed
            || has_untracked_projectors
        {
            dirty_windows.extend(all_windows.iter().copied());
        }

        for window in &all_windows {
            let previous = views.roots_by_window.get(window);
            let current = roots_by_window.get(window);
            if previous != current {
                dirty_windows.insert(*window);
            }
        }

        for window in invalidation.windows {
            dirty_windows.insert(window);
        }

        for root in invalidation.roots {
            if let Some((window, _roots)) = roots_by_window
                .iter()
                .find(|(_window, roots)| roots.contains(&root))
                .or_else(|| {
                    views
                        .roots_by_window
                        .iter()
                        .find(|(_window, roots)| roots.contains(&root))
                })
            {
                dirty_windows.insert(*window);
            }
        }

        dirty_entities.sort_by_key(|entity| entity.to_bits());
        dirty_entities.dedup();
        for entity in dirty_entities {
            if let Some(window) = views.entity_windows.get(&entity) {
                dirty_windows.insert(*window);
            }
        }
    }

    if dirty_windows.is_empty() {
        return;
    }

    let dirty_windows = dirty_windows.into_iter().collect::<Vec<_>>();
    let mut updates = Vec::with_capacity(dirty_windows.len());
    for window_entity in &dirty_windows {
        let roots = roots_by_window
            .get(window_entity)
            .cloned()
            .unwrap_or_default();
        let (synthesized, window_stats, window_entities) =
            world.resource_scope(|world, registry: Mut<UiProjectorRegistry>| {
                synthesize_roots_with_stats_and_entities(world, &registry, roots)
            });
        updates.push((
            *window_entity,
            compose_window_root(&synthesized),
            window_stats,
            window_entities,
        ));
    }

    let mut stats = UiSynthesisStats::default();
    {
        let mut views = world.resource_mut::<SynthesizedUiViews>();

        let stale_windows = views
            .windows
            .keys()
            .copied()
            .filter(|window| !all_windows.contains(window))
            .collect::<Vec<_>>();
        for window in stale_windows {
            views.remove_window(window);
        }

        for (window, view, window_stats, window_entities) in updates {
            views.windows.insert(window, view);
            views.roots_by_window
                .insert(window, roots_by_window.remove(&window).unwrap_or_default());
            views.stats_by_window.insert(window, window_stats);

            if let Some(previous_entities) = views.entities_by_window.remove(&window) {
                for entity in previous_entities {
                    views.entity_windows.remove(&entity);
                }
            }
            for entity in &window_entities {
                views.entity_windows.insert(*entity, window);
            }
            views.entities_by_window.insert(window, window_entities);
            views.dirty_windows.insert(window);
        }

        for window_stats in views.stats_by_window.values() {
            stats.add_assign(window_stats);
        }
        views.generation = views.generation.saturating_add(1);
    }

    *world.resource_mut::<UiSynthesisStats>() = stats;
}

fn projection_resources_changed(world: &mut World) -> bool {
    resource_changed::<StyleSheet>(world)
        || resource_changed::<AppI18n>(world)
        || resource_changed::<WindowSize>(world)
        || resource_changed::<AppBreakpoints>(world)
        || resource_changed::<OverlayStack>(world)
        || resource_changed::<ActiveStyleVariant>(world)
}

fn resource_changed<R: Resource>(world: &World) -> bool {
    world.is_resource_added::<R>() || world.is_resource_changed::<R>()
}

pub(crate) fn register_projection_invalidation_dependencies(registry: &mut UiProjectorRegistry) {
    registry
        .register_dependency::<Children>()
        .register_dependency::<UiWindow>()
        .register_dependency::<ComputedStyle>()
        .register_dependency::<CurrentColorStyle>()
        .register_dependency::<LocalizeText>()
        .register_dependency::<TypographyPreset>()
        .register_dependency::<OverlayComputedPosition>()
        .register_dependency::<OverlayAnchorRect>()
        .register_dependency::<OverlayConfig>()
        .register_dependency::<OverlayState>()
        .register_dependency::<AnchoredTo>();
}

/// Compose a single window's root view from its set of synthesized roots.
///
/// Mirrors the previous single-window zstack composition so overlays sort
/// last and content fills the viewport.
fn compose_window_root(roots: &[UiView]) -> UiView {
    use crate::runtime::compose_runtime_root as compose;
    compose(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::register_builtin_projectors;
    use bevy_ecs::hierarchy::ChildOf;

    #[test]
    fn synthesis_stats_track_missing_entity() {
        let mut world = World::new();
        let mut registry = UiProjectorRegistry::default();
        register_builtin_projectors(&mut registry);

        let stale_root = world.spawn_empty().id();
        assert!(world.despawn(stale_root));

        let (_roots, stats) = synthesize_roots_with_stats(&world, &registry, [stale_root]);

        assert_eq!(stats.root_count, 1);
        assert_eq!(stats.node_count, 1);
        assert_eq!(stats.missing_entity_count, 1);
        assert_eq!(stats.cycle_count, 0);
    }

    #[test]
    fn builtin_registry_projects_label() {
        let mut world = World::new();
        let mut registry = UiProjectorRegistry::default();
        register_builtin_projectors(&mut registry);

        let root = world.spawn((UiRoot, crate::UiLabel::new("ok"))).id();

        let (roots, stats) = synthesize_roots_with_stats(&world, &registry, [root]);

        assert_eq!(roots.len(), 1);
        assert_eq!(stats.unhandled_count, 0);
        assert_eq!(stats.missing_entity_count, 0);
    }

    #[test]
    fn builtin_registry_projects_new_ui_primitives() {
        let mut world = World::new();
        world.insert_resource(crate::StyleSheet::default());
        let mut registry = UiProjectorRegistry::default();
        register_builtin_projectors(&mut registry);

        let root = world.spawn((UiRoot, crate::UiFlexColumn)).id();
        let grid = world.spawn((crate::UiGrid::new(2, 1), ChildOf(root))).id();
        world.spawn((
            crate::UiLabel::new("a"),
            crate::UiGridCell::new(0, 0),
            ChildOf(grid),
        ));
        world.spawn((
            crate::UiLabel::new("b"),
            crate::UiGridCell::new(1, 0),
            ChildOf(grid),
        ));
        world.spawn((
            crate::UiCanvas::new()
                .with_alt_text("drawing")
                .with_command(crate::UiCanvasCommand::FillRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                    color: crate::xilem::Color::from_rgb8(255, 0, 0),
                }),
            ChildOf(root),
        ));
        world.spawn((
            crate::UiImage::from_rgba8(1, 1, vec![255, 0, 0, 255]).with_alt_text("pixel"),
            ChildOf(root),
        ));
        world.spawn((
            crate::UiPasswordInput::new("secret").with_placeholder("password"),
            ChildOf(root),
        ));
        world.spawn((
            crate::UiMultilineTextInput::new("line one\nline two").with_placeholder("notes"),
            ChildOf(root),
        ));
        world.spawn((
            crate::UiListView::new(["alpha", "beta"]).with_selected(1),
            ChildOf(root),
        ));
        world.spawn((
            crate::UiDataTable::from_labels(["Name", "Role"])
                .with_cells("1", ["Ada", "Engineer"])
                .with_selected_row(0),
            ChildOf(root),
        ));

        let (_roots, stats) = synthesize_roots_with_stats(&world, &registry, [root]);

        assert_eq!(stats.unhandled_count, 0);
        assert_eq!(stats.missing_entity_count, 0);
    }
}

use bevy_ecs::{
    component::ComponentId,
    lifecycle::RemovedComponentEntity,
    message::MessageCursor,
    prelude::*,
};
use picus_view::AnyWidgetView;
use std::{any::TypeId, fmt, marker::PhantomData, sync::Arc};

/// Xilem state used by synthesized UI views.
pub type UiXilemState = ();
/// Xilem action type used by synthesized UI views.
pub type UiXilemAction = ();

/// Type-erased Xilem Masonry view used as projection output.
pub type UiAnyView = AnyWidgetView<UiXilemState, UiXilemAction>;
/// Shared synthesized view handle.
pub type UiView = Arc<UiAnyView>;

/// Built-in button action emitted by [`UiButton`] projector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinUiAction {
    Clicked,
}

/// Projection context passed to projector implementations.
pub struct ProjectionCtx<'a> {
    pub world: &'a World,
    pub entity: Entity,
    pub node_id: u64,
    pub children: Vec<UiView>,
}

impl fmt::Debug for ProjectionCtx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectionCtx")
            .field("entity", &self.entity)
            .field("node_id", &self.node_id)
            .field("children_len", &self.children.len())
            .finish()
    }
}

/// Maps ECS entity data into a concrete Xilem Masonry view.
pub trait UiProjector: Send + Sync + 'static {
    fn project(&self, ctx: ProjectionCtx<'_>) -> Option<UiView>;
}

struct ComponentProjector<C: Component> {
    projector: fn(&C, ProjectionCtx<'_>) -> UiView,
    _marker: PhantomData<C>,
}

impl<C: Component> UiProjector for ComponentProjector<C> {
    fn project(&self, ctx: ProjectionCtx<'_>) -> Option<UiView> {
        let component = ctx.world.get::<C>(ctx.entity)?;
        Some((self.projector)(component, ctx))
    }
}

struct ProjectionComponentDependency {
    type_id: TypeId,
    type_name: &'static str,
    component_id: Option<ComponentId>,
    ensure_component_id: fn(&mut World) -> ComponentId,
    changed_entities: fn(&mut World) -> Vec<Entity>,
    removed_reader: MessageCursor<RemovedComponentEntity>,
}

impl ProjectionComponentDependency {
    fn new<C: Component>() -> Self {
        Self {
            type_id: TypeId::of::<C>(),
            type_name: std::any::type_name::<C>(),
            component_id: None,
            ensure_component_id: |world| world.register_component::<C>(),
            changed_entities: changed_entities::<C>,
            removed_reader: MessageCursor::default(),
        }
    }

    fn component_id(&mut self, world: &mut World) -> ComponentId {
        match self.component_id {
            Some(component_id) => component_id,
            None => {
                let component_id = (self.ensure_component_id)(world);
                self.component_id = Some(component_id);
                component_id
            }
        }
    }

    fn drain_dirty_entities(&mut self, world: &mut World) -> Vec<Entity> {
        let mut dirty = (self.changed_entities)(world);
        let component_id = self.component_id(world);

        if let Some(messages) = world.removed_components().get(component_id) {
            dirty.extend(self.removed_reader.read(messages).cloned().map(|removed| {
                let entity: Entity = removed.into();
                entity
            }));
        }

        dirty
    }
}

fn changed_entities<C: Component>(world: &mut World) -> Vec<Entity> {
    let mut query = world.query_filtered::<Entity, Changed<C>>();
    query.iter(world).collect()
}

/// Registry of projector implementations.
#[derive(Resource, Default)]
pub struct UiProjectorRegistry {
    projectors: Vec<Box<dyn UiProjector>>,
    dependencies: Vec<ProjectionComponentDependency>,
    untracked_projectors: usize,
}

impl UiProjectorRegistry {
    /// Register a raw projector implementation.
    pub fn register_projector<P: UiProjector>(&mut self, projector: P) -> &mut Self {
        self.projectors.push(Box::new(projector));
        self.untracked_projectors += 1;
        self
    }

    /// Register a component whose changes should invalidate ECS-to-retained
    /// projection.
    pub fn register_dependency<C: Component>(&mut self) -> &mut Self {
        let type_id = TypeId::of::<C>();
        if !self
            .dependencies
            .iter()
            .any(|dependency| dependency.type_id == type_id)
        {
            self.dependencies
                .push(ProjectionComponentDependency::new::<C>());
        }
        self
    }

    /// Register a projector bound to a specific ECS component type.
    pub fn register_component<C: Component>(
        &mut self,
        projector: fn(&C, ProjectionCtx<'_>) -> UiView,
    ) -> &mut Self {
        self.register_dependency::<C>();
        self.projectors.push(Box::new(ComponentProjector::<C> {
            projector,
            _marker: PhantomData,
        }));
        self
    }

    pub(crate) fn drain_dirty_entities(&mut self, world: &mut World) -> Vec<Entity> {
        let mut dirty = Vec::new();
        for dependency in &mut self.dependencies {
            let changed = dependency.drain_dirty_entities(world);
            if !changed.is_empty() {
                tracing::trace!(
                    component = dependency.type_name,
                    count = changed.len(),
                    "projection dependency changed"
                );
                dirty.extend(changed);
            }
        }
        dirty
    }

    pub(crate) fn has_untracked_projectors(&self) -> bool {
        self.untracked_projectors > 0
    }

    pub(crate) fn project_node(
        &self,
        world: &World,
        entity: Entity,
        node_id: u64,
        children: Vec<UiView>,
    ) -> Option<UiView> {
        // Last registered projector wins.
        for projector in self.projectors.iter().rev() {
            let ctx = ProjectionCtx {
                world,
                entity,
                node_id,
                children: children.clone(),
            };
            if let Some(view) = projector.project(ctx) {
                return Some(view);
            }
        }

        None
    }
}

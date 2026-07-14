use bevy_ecs::{
    component::ComponentId,
    lifecycle::RemovedComponentEntity,
    message::MessageCursor,
    prelude::*,
};
use picus_view::AnyWidgetView;
use std::{any::TypeId, fmt, marker::PhantomData, sync::Arc};

// UiView is Arc-based; FlexExt is imported in helper methods that need it.

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

impl ProjectionCtx<'_> {
    /// Return a cloneable [`crate::UiActionSender`] for deferred emission.
    ///
    /// Panics if `T` was not registered with [`crate::AppPicusExt::add_ui_action`].
    #[must_use]
    pub fn action_sender<T: Clone + Send + Sync + 'static>(&self) -> crate::UiActionSender<T> {
        self.world
            .get_resource::<crate::UiActionSender<T>>()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "UiActionSender<{}> is not registered; call AppPicusExt::add_ui_action::<{}>()",
                    std::any::type_name::<T>(),
                    std::any::type_name::<T>(),
                )
            })
    }

    /// Action-aware button that uses this context's entity as the action source.
    #[must_use]
    pub fn button<A: Clone + Send + Sync + 'static>(
        &self,
        action: A,
        label: impl Into<masonry_core::core::ArcStr>,
    ) -> crate::ButtonView<A> {
        debug_assert!(
            self.world
                .get_resource::<crate::UiActionRegistry>()
                .is_some_and(|registry| registry.is_registered(std::any::TypeId::of::<A>()))
                || std::any::TypeId::of::<A>() == std::any::TypeId::of::<BuiltinUiAction>(),
            "button action type `{}` is not registered with add_ui_action",
            std::any::type_name::<A>(),
        );
        crate::retained_bridge::button(self.entity, action, label)
    }

    /// Action-aware button with a custom child view.
    #[must_use]
    pub fn button_with_child<A, V>(
        &self,
        action: A,
        child: V,
    ) -> crate::ButtonWithChildView<A, V>
    where
        A: Clone + Send + Sync + 'static,
        V: picus_view::WidgetView<(), ()>,
    {
        debug_assert!(
            self.world
                .get_resource::<crate::UiActionRegistry>()
                .is_some_and(|registry| registry.is_registered(std::any::TypeId::of::<A>()))
                || std::any::TypeId::of::<A>() == std::any::TypeId::of::<BuiltinUiAction>(),
            "button action type `{}` is not registered with add_ui_action",
            std::any::type_name::<A>(),
        );
        crate::retained_bridge::button_with_child(self.entity, action, child)
    }

    /// Apply this entity's resolved style to a child view.
    #[must_use]
    pub fn styled<V>(&self, view: V) -> UiView
    where
        V: picus_view::WidgetView<(), ()> + 'static,
    {
        let style = crate::resolve_style(self.world, self.entity);
        Arc::new(crate::apply_widget_style(view, &style))
    }

    /// Flex column with the provided children (projection helper to reduce dual writing).
    #[must_use]
    pub fn flex_col(
        &self,
        children: impl IntoIterator<Item = UiView>,
    ) -> impl picus_view::WidgetView<(), ()> {
        use picus_view::view::FlexExt as _;
        let _ = self;
        picus_view::view::flex_col(
            children
                .into_iter()
                .map(|child| child.into_any_flex())
                .collect::<Vec<_>>(),
        )
    }

    /// Flex row with the provided children (projection helper to reduce dual writing).
    #[must_use]
    pub fn flex_row(
        &self,
        children: impl IntoIterator<Item = UiView>,
    ) -> impl picus_view::WidgetView<(), ()> {
        use picus_view::view::FlexExt as _;
        let _ = self;
        picus_view::view::flex_row(
            children
                .into_iter()
                .map(|child| child.into_any_flex())
                .collect::<Vec<_>>(),
        )
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

struct ProjectionResourceDependency {
    type_id: TypeId,
    type_name: &'static str,
    resource_id: Option<ComponentId>,
    ensure_resource_id: fn(&mut World) -> ComponentId,
}

impl ProjectionResourceDependency {
    fn new<R: Resource>() -> Self {
        Self {
            type_id: TypeId::of::<R>(),
            type_name: std::any::type_name::<R>(),
            resource_id: None,
            ensure_resource_id: |world| world.register_component::<R>(),
        }
    }

    fn resource_id(&mut self, world: &mut World) -> ComponentId {
        match self.resource_id {
            Some(resource_id) => resource_id,
            None => {
                let resource_id = (self.ensure_resource_id)(world);
                self.resource_id = Some(resource_id);
                resource_id
            }
        }
    }

    fn changed(&mut self, world: &mut World) -> bool {
        let resource_id = self.resource_id(world);
        world.is_resource_added_by_id(resource_id) || world.is_resource_changed_by_id(resource_id)
    }
}

/// Registry of projector implementations.
#[derive(Resource, Default)]
pub struct UiProjectorRegistry {
    projectors: Vec<Box<dyn UiProjector>>,
    dependencies: Vec<ProjectionComponentDependency>,
    resource_dependencies: Vec<ProjectionResourceDependency>,
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

    /// Register a resource whose changes should invalidate ECS-to-retained
    /// projection.
    pub fn register_resource_dependency<R: Resource>(&mut self) -> &mut Self {
        let type_id = TypeId::of::<R>();
        if !self
            .resource_dependencies
            .iter()
            .any(|dependency| dependency.type_id == type_id)
        {
            self.resource_dependencies
                .push(ProjectionResourceDependency::new::<R>());
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

    pub(crate) fn drain_dirty_resources(&mut self, world: &mut World) -> bool {
        let mut changed = false;
        for dependency in &mut self.resource_dependencies {
            if dependency.changed(world) {
                tracing::trace!(
                    resource = dependency.type_name,
                    "projection resource dependency changed"
                );
                changed = true;
            }
        }
        changed
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

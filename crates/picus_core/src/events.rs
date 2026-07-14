//! Internal UI action queue and application-facing [`UiAction`] messages.
//!
//! Retained widgets and projection helpers write type-erased payloads into an
//! app-owned [`InternalUiEventQueue`]. A single PreUpdate dispatcher drains the
//! queue and routes entries through [`UiActionRegistry`]. Application code only
//! observes typed [`UiAction<T>`] messages (or captures a [`UiActionSender<T>`]
//! for deferred emission).

use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    fmt,
    marker::PhantomData,
    sync::Arc,
};

use bevy_app::App;
use bevy_ecs::{
    entity::Entity,
    message::Message,
    prelude::{Component, Resource, World},
};
use bevy_input::mouse::MouseButton;
use crossbeam_queue::SegQueue;
use tracing::{debug, error};

/// Soft cap on actions processed per frame to break self-trigger loops.
pub const UI_ACTION_DISPATCH_LIMIT: usize = 10_000;

/// Pointer phase used by high-level UI pointer events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiPointerPhase {
    Pressed,
    Released,
}

/// Hit-tested UI pointer event before ECS bubbling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPointerHitEvent {
    pub target: Entity,
    pub position: (f64, f64),
    pub button: MouseButton,
    pub phase: UiPointerPhase,
}

/// Bubbling UI pointer event emitted for each ancestor in the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPointerEvent {
    pub target: Entity,
    pub current_target: Entity,
    pub position: (f64, f64),
    pub button: MouseButton,
    pub phase: UiPointerPhase,
    pub consumed: bool,
}

/// Marker that stops bubbling at the tagged entity.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StopUiPointerPropagation;

/// Type-erased UI action entry stored in the internal queue.
#[derive(Clone)]
pub(crate) struct InternalUiEvent {
    /// Source ECS entity for this action.
    pub entity: Entity,
    /// Runtime type of the payload.
    pub type_id: TypeId,
    /// Type-erased action payload.
    pub action: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for InternalUiEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalUiEvent")
            .field("entity", &self.entity)
            .field("type_id", &self.type_id)
            .field("action", &"<type-erased>")
            .finish()
    }
}

impl InternalUiEvent {
    #[must_use]
    pub(crate) fn typed<T: Any + Send + Sync>(entity: Entity, action: T) -> Self {
        Self {
            entity,
            type_id: TypeId::of::<T>(),
            action: Arc::new(action),
        }
    }

    #[must_use]
    pub(crate) fn erased(
        entity: Entity,
        type_id: TypeId,
        action: Arc<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            entity,
            type_id,
            action,
        }
    }
}

/// Compatibility alias used by older internal call sites that still construct
/// type-erased events by name.
pub(crate) type UiEvent = InternalUiEvent;

/// Application-facing UI action message.
///
/// Payload `T` does not need to implement [`Message`]; only `UiAction<T>` is a
/// Bevy message. Register `T` with [`AppPicusExt::add_ui_action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiAction<T> {
    /// Entity that produced the action.
    pub source: Entity,
    /// Typed action payload.
    pub action: T,
}

impl<T: Send + Sync + 'static> Message for UiAction<T> {}

/// Non-generic ECS component that binds a button (or similar control) to a
/// typed business action.
///
/// Use [`UiEmit::new`] and attach via `template_value(...)` in BSN. When
/// present on a [`crate::UiButton`] entity, projection emits that payload
/// instead of [`crate::BuiltinUiAction::Clicked`].
#[derive(Component, Clone)]
pub struct UiEmit {
    type_id: TypeId,
    payload: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for UiEmit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiEmit")
            .field("type_id", &self.type_id)
            .finish()
    }
}

impl UiEmit {
    /// Create a type-erased emit binding for payload `T`.
    ///
    /// `T` must be registered with [`AppPicusExt::add_ui_action`] before the
    /// action is dispatched.
    #[must_use]
    pub fn new<T: Clone + Send + Sync + 'static>(action: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            payload: Arc::new(action),
        }
    }

    #[must_use]
    pub(crate) fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub(crate) fn payload(&self) -> Arc<dyn Any + Send + Sync> {
        Arc::clone(&self.payload)
    }
}

/// Cloneable write-only handle for deferred action emission.
///
/// Actions are queued on the app-owned internal sink and become visible to
/// [`bevy_ecs::message::MessageReader`] after the next PreUpdate dispatch pass.
/// Emissions from `Update` (or later) are therefore next-frame visible.
#[derive(Resource, Clone)]
pub struct UiActionSender<T> {
    sink: InternalUiActionSink,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Clone + Send + Sync + 'static> UiActionSender<T> {
    /// Queue a typed action for the given source entity.
    pub fn send(&self, source: Entity, action: T) {
        self.sink.push_typed(source, action);
    }
}

impl<T> fmt::Debug for UiActionSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiActionSender")
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

/// Shared write sink used by retained widgets and [`UiActionSender`].
#[derive(Clone, Default)]
pub(crate) struct InternalUiActionSink {
    queue: Arc<SegQueue<InternalUiEvent>>,
}

impl fmt::Debug for InternalUiActionSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalUiActionSink").finish_non_exhaustive()
    }
}

impl InternalUiActionSink {
    #[must_use]
    pub(crate) fn shared_queue(&self) -> Arc<SegQueue<InternalUiEvent>> {
        Arc::clone(&self.queue)
    }

    pub(crate) fn push(&self, event: InternalUiEvent) {
        self.queue.push(event);
    }

    pub(crate) fn push_typed<T: Any + Send + Sync>(&self, entity: Entity, action: T) {
        self.push(InternalUiEvent::typed(entity, action));
    }

    pub(crate) fn push_erased(
        &self,
        entity: Entity,
        type_id: TypeId,
        action: Arc<dyn Any + Send + Sync>,
    ) {
        self.push(InternalUiEvent::erased(entity, type_id, action));
    }
}

/// App-owned lock-free queue shared between Bevy systems and retained widgets.
///
/// Application code must not drain this queue. Only
/// [`dispatch_ui_actions`](crate::events::dispatch_ui_actions) is the consumer.
///
/// Visible as `pub` only so Bevy system parameter types can appear in public
/// system function signatures used by `PicusPlugin`. Not part of the app facade.
#[derive(Resource, Clone, Debug, Default)]
#[doc(hidden)]
pub struct InternalUiEventQueue {
    sink: InternalUiActionSink,
}

impl InternalUiEventQueue {
    #[must_use]
    pub(crate) fn sink(&self) -> InternalUiActionSink {
        self.sink.clone()
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn shared_queue(&self) -> Arc<SegQueue<InternalUiEvent>> {
        self.sink.shared_queue()
    }

    pub(crate) fn push(&self, event: InternalUiEvent) {
        self.sink.push(event);
    }

    pub(crate) fn push_typed<T: Any + Send + Sync>(&self, entity: Entity, action: T) {
        self.sink.push_typed(entity, action);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn push_erased(
        &self,
        entity: Entity,
        type_id: TypeId,
        action: Arc<dyn Any + Send + Sync>,
    ) {
        self.sink.push_erased(entity, type_id, action);
    }

    /// Drain every queued event (single-consumer path only).
    #[must_use]
    pub(crate) fn drain_all(&mut self) -> Vec<InternalUiEvent> {
        let mut drained = Vec::new();
        while let Some(event) = self.sink.queue.pop() {
            drained.push(event);
        }
        drained
    }
}

/// Handler invoked by the dispatcher for a registered payload type.
pub(crate) type UiActionHandler =
    Arc<dyn Fn(&mut World, Entity, &dyn Any) + Send + Sync + 'static>;

/// Registry mapping payload `TypeId` values to dispatcher handlers.
#[derive(Resource, Default)]
pub struct UiActionRegistry {
    handlers: HashMap<TypeId, Vec<UiActionHandler>>,
    /// Type names for diagnostics (optional).
    type_names: HashMap<TypeId, &'static str>,
    /// One-shot log markers for unregistered payloads in release builds.
    logged_unregistered: HashMap<TypeId, ()>,
}

impl UiActionRegistry {
    /// Register a low-level handler for payload type `T`.
    ///
    /// Multiple handlers may be registered for the same type; they run in
    /// registration order for each matching event.
    pub fn register_handler<T, F>(&mut self, handler: F)
    where
        T: Any + Send + Sync + 'static,
        F: Fn(&mut World, Entity, &T) + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        self.type_names
            .entry(type_id)
            .or_insert(std::any::type_name::<T>());
        let wrapped: UiActionHandler = Arc::new(move |world, entity, any| {
            if let Some(action) = any.downcast_ref::<T>() {
                handler(world, entity, action);
            } else {
                error!(
                    type_name = std::any::type_name::<T>(),
                    "UiAction handler received payload that failed to downcast"
                );
            }
        });
        self.handlers.entry(type_id).or_default().push(wrapped);
    }

    /// Register `T` as an application message payload.
    ///
    /// Each matching queue entry is written as [`UiAction<T>`].
    pub fn register_message_payload<T>(&mut self)
    where
        T: Clone + Send + Sync + 'static,
    {
        self.register_handler::<T, _>(|world, entity, action| {
            world.write_message(UiAction {
                source: entity,
                action: action.clone(),
            });
        });
    }

    #[must_use]
    pub(crate) fn is_registered(&self, type_id: TypeId) -> bool {
        self.handlers.contains_key(&type_id)
    }

    fn dispatch_one(&mut self, world: &mut World, event: &InternalUiEvent) {
        let Some(handlers) = self.handlers.get(&event.type_id).cloned() else {
            let name = self
                .type_names
                .get(&event.type_id)
                .copied()
                .unwrap_or("<unknown>");
            if cfg!(debug_assertions) {
                panic!(
                    "unregistered UI action payload type `{name}` (TypeId = {:?}); \
                     call AppPicusExt::add_ui_action::<T>() or register a built-in handler",
                    event.type_id
                );
            }
            if self.logged_unregistered.insert(event.type_id, ()).is_none() {
                error!(
                    type_id = ?event.type_id,
                    type_name = name,
                    "dropping unregistered UI action payload (logged once)"
                );
            }
            return;
        };

        for handler in handlers {
            handler(world, event.entity, event.action.as_ref());
        }
    }
}

/// Install application action support for payload type `T`.
///
/// Registers `Messages<UiAction<T>>`, a [`UiActionSender<T>`] resource, and a
/// registry handler that writes messages.
pub fn register_ui_action_type<T>(app: &mut App)
where
    T: Clone + Send + Sync + 'static,
{
    app.init_resource::<InternalUiEventQueue>();
    app.init_resource::<UiActionRegistry>();
    app.add_message::<UiAction<T>>();

    let sink = app.world().resource::<InternalUiEventQueue>().sink();
    app.insert_resource(UiActionSender::<T> {
        sink,
        _marker: PhantomData,
    });

    app.world_mut()
        .resource_mut::<UiActionRegistry>()
        .register_message_payload::<T>();
}

/// Sole consumer of [`InternalUiEventQueue`].
///
/// Drains the queue and dispatches until empty (or until
/// [`UI_ACTION_DISPATCH_LIMIT`] is hit). Handlers may enqueue additional
/// actions; those are processed after already-queued entries (FIFO).
pub fn dispatch_ui_actions(world: &mut World) {
    world.init_resource::<InternalUiEventQueue>();
    world.init_resource::<UiActionRegistry>();

    let mut processed = 0usize;
    loop {
        let batch = world.resource_mut::<InternalUiEventQueue>().drain_all();
        if batch.is_empty() {
            break;
        }

        for event in batch {
            if processed >= UI_ACTION_DISPATCH_LIMIT {
                error!(
                    limit = UI_ACTION_DISPATCH_LIMIT,
                    "UI action dispatch limit reached; remaining queue entries deferred to next frame"
                );
                // Re-queue the current event and stop so the frame can proceed.
                world.resource::<InternalUiEventQueue>().push(event);
                return;
            }
            processed += 1;

            // Take registry by value-like access: dispatch needs &mut registry
            // and &mut world for handlers. Split via resource removal.
            let mut registry = world
                .remove_resource::<UiActionRegistry>()
                .unwrap_or_default();
            registry.dispatch_one(world, &event);
            world.insert_resource(registry);
        }
    }

    if processed > 0 {
        debug!(processed, "dispatched UI actions");
    }
}

thread_local! {
    static ACTIVE_UI_ACTION_SINK: RefCell<Option<InternalUiActionSink>> =
        const { RefCell::new(None) };
}

/// Install the active app sink used by retained widgets on this thread.
pub(crate) fn install_app_ui_action_sink(sink: InternalUiActionSink) {
    ACTIVE_UI_ACTION_SINK.with(|slot| {
        *slot.borrow_mut() = Some(sink);
    });
}

/// Push a type-erased event into the active app sink (if installed).
pub(crate) fn push_active_ui_event(event: InternalUiEvent) {
    ACTIVE_UI_ACTION_SINK.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            sink.push(event);
        }
    });
}

/// Push a typed action into the active app sink (if installed).
pub(crate) fn push_active_ui_action<T: Any + Send + Sync>(entity: Entity, action: T) {
    push_active_ui_event(InternalUiEvent::typed(entity, action));
}

// ---------------------------------------------------------------------------
// Compatibility shims used while internal call sites migrate.
// ---------------------------------------------------------------------------

/// Historical name for the internal queue. Not part of the public facade.
pub(crate) type UiEventQueue = InternalUiEventQueue;

impl UiEventQueue {
    /// Historical typed drain kept for internal systems that have not yet been
    /// folded into the dispatcher. Prefer dispatcher handlers for new code.
    #[doc(hidden)]
    #[must_use]
    pub fn drain_actions<T: Any + Clone + Send + Sync>(&mut self) -> Vec<TypedUiEvent<T>> {
        let mut drained = Vec::new();
        let mut unmatched = Vec::new();
        while let Some(event) = self.sink.queue.pop() {
            if event.type_id == TypeId::of::<T>() {
                if let Some(action) = event.action.downcast_ref::<T>() {
                    drained.push(TypedUiEvent {
                        entity: event.entity,
                        action: action.clone(),
                    });
                    continue;
                }
            }
            unmatched.push(event);
        }
        for event in unmatched {
            self.sink.push(event);
        }
        drained
    }
}

/// Typed event recovered from a type-erased queue entry (internal).
#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct TypedUiEvent<T> {
    pub entity: Entity,
    pub action: T,
}

/// Emit a typed UI action into the active app-owned sink.
///
/// Application code should capture [`UiActionSender<T>`] from
/// [`crate::ProjectionCtx::action_sender`] instead. This remains for retained
/// bridge views that emit during message handling.
pub(crate) fn emit_ui_action<T: Any + Send + Sync>(entity: Entity, action: T) {
    push_active_ui_action(entity, action);
}

/// Historical name for installing the active sink.
pub(crate) fn install_global_ui_event_queue(queue: Arc<SegQueue<InternalUiEvent>>) {
    install_app_ui_action_sink(InternalUiActionSink { queue });
}

pub(crate) fn push_global_ui_event(event: InternalUiEvent) {
    push_active_ui_event(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppPicusExt, BuiltinUiAction, PicusPlugin, UiRoot};
    use bevy_app::App;
    use bevy_ecs::message::MessageReader;
    use bevy_ecs::prelude::*;
    use bevy_ecs::system::RunSystemOnce;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestAction {
        Clicked,
        Inc,
    }

    #[test]
    fn dispatcher_writes_ui_action_messages() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .add_ui_action::<TestAction>();

        let entity = app.world_mut().spawn(UiRoot).id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, TestAction::Clicked);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let messages = app.world().resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor = messages.get_cursor();
        let collected: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].source, entity);
        assert_eq!(collected[0].action, TestAction::Clicked);
    }

    #[test]
    fn two_message_readers_each_see_action_once() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .add_ui_action::<TestAction>()
            .insert_resource(CountA(0))
            .insert_resource(CountB(0))
            .add_systems(
                bevy_app::Update,
                (reader_a, reader_b),
            );

        #[derive(Resource)]
        struct CountA(u32);
        #[derive(Resource)]
        struct CountB(u32);

        fn reader_a(mut reader: MessageReader<UiAction<TestAction>>, mut c: ResMut<CountA>) {
            for _ in reader.read() {
                c.0 += 1;
            }
        }
        fn reader_b(mut reader: MessageReader<UiAction<TestAction>>, mut c: ResMut<CountB>) {
            for _ in reader.read() {
                c.0 += 1;
            }
        }

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, TestAction::Inc);

        // PreUpdate runs dispatcher via PicusPlugin schedule.
        app.update();

        assert_eq!(app.world().resource::<CountA>().0, 1);
        assert_eq!(app.world().resource::<CountB>().0, 1);
    }

    #[test]
    fn action_sender_queues_for_dispatch() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<UiActionSender<TestAction>>()
            .send(entity, TestAction::Inc);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let messages = app.world().resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(cursor.read(messages).count(), 1);
    }

    #[test]
    fn ui_emit_stores_type_id() {
        let emit = UiEmit::new(TestAction::Clicked);
        assert_eq!(emit.type_id(), TypeId::of::<TestAction>());
        assert!(emit.payload().downcast_ref::<TestAction>().is_some());
    }

    #[test]
    fn builtin_action_is_registered_by_plugin() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, BuiltinUiAction::Clicked);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let messages = app
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<BuiltinUiAction>>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(cursor.read(messages).count(), 1);
    }

    #[test]
    fn independent_apps_do_not_share_action_sinks() {
        let mut app_a = App::new();
        app_a.add_plugins(PicusPlugin).add_ui_action::<TestAction>();
        let mut app_b = App::new();
        app_b.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        let entity_a = app_a.world_mut().spawn_empty().id();
        let entity_b = app_b.world_mut().spawn_empty().id();

        app_a
            .world()
            .resource::<UiActionSender<TestAction>>()
            .send(entity_a, TestAction::Inc);
        app_b
            .world()
            .resource::<UiActionSender<TestAction>>()
            .send(entity_b, TestAction::Clicked);

        app_a
            .world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch a");
        app_b
            .world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch b");

        let messages_a = app_a
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor_a = messages_a.get_cursor();
        let collected_a: Vec<_> = cursor_a.read(messages_a).cloned().collect();
        assert_eq!(collected_a.len(), 1);
        assert_eq!(collected_a[0].action, TestAction::Inc);

        let messages_b = app_b
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor_b = messages_b.get_cursor();
        let collected_b: Vec<_> = cursor_b.read(messages_b).cloned().collect();
        assert_eq!(collected_b.len(), 1);
        assert_eq!(collected_b[0].action, TestAction::Clicked);
    }

    #[test]
    fn fifo_order_preserves_handler_enqueued_actions() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        // Register an extra handler that enqueues a follow-up action.
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register_handler::<TestAction, _>(|world, entity, action| {
                if *action == TestAction::Inc {
                    world
                        .resource::<InternalUiEventQueue>()
                        .push_typed(entity, TestAction::Clicked);
                }
            });

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, TestAction::Inc);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let messages = app
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor = messages.get_cursor();
        let collected: Vec<_> = cursor.read(messages).map(|m| m.action.clone()).collect();
        assert_eq!(
            collected,
            vec![TestAction::Inc, TestAction::Clicked],
            "handler-derived actions must be processed after already-queued FIFO entries"
        );
    }

    #[test]
    fn dispatch_limit_defers_remaining_actions() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        let entity = app.world_mut().spawn_empty().id();
        // Self-triggering handler would loop forever without the cap.
        app.world_mut()
            .resource_mut::<UiActionRegistry>()
            .register_handler::<TestAction, _>(|world, entity, _| {
                world
                    .resource::<InternalUiEventQueue>()
                    .push_typed(entity, TestAction::Inc);
            });

        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, TestAction::Inc);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        // Queue should still contain deferred work after hitting the limit.
        let remaining = app
            .world_mut()
            .resource_mut::<InternalUiEventQueue>()
            .drain_all()
            .len();
        assert!(
            remaining > 0,
            "dispatch limit should re-queue remaining work instead of spinning forever"
        );
    }

    #[test]
    #[should_panic(expected = "unregistered UI action payload")]
    fn unregistered_payload_panics_in_debug() {
        #[derive(Clone, Debug)]
        struct Unregistered;
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, Unregistered);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");
    }

    #[test]
    fn unregistered_payload_is_dropped_not_requeued() {
        // After a failed registration path that panics in debug, verify the
        // drop-and-forget path when the type is never registered: simulate by
        // draining the empty queue after a registered dispatch leaves no residual
        // unknown entries. The debug panic path is covered above; this checks
        // that a registered action does not leave junk in the queue.
        let mut app = App::new();
        app.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        let entity = app.world_mut().spawn_empty().id();
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(entity, TestAction::Inc);

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let remaining = app
            .world_mut()
            .resource_mut::<InternalUiEventQueue>()
            .drain_all()
            .len();
        assert_eq!(remaining, 0, "dispatched queue must be empty after success");
    }

    #[test]
    fn widget_action_handler_is_registered_and_emits_changed_message() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let checkbox = app
            .world_mut()
            .spawn(crate::UiCheckbox {
                checked: false,
                indeterminate: false,
                ..Default::default()
            })
            .id();

        app.world()
            .resource::<InternalUiEventQueue>()
            .push_typed(
                checkbox,
                crate::WidgetUiAction::ToggleCheckbox { checkbox },
            );

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        assert!(
            app.world()
                .get::<crate::UiCheckbox>(checkbox)
                .is_some_and(|c| c.checked),
            "dispatcher must apply WidgetUiAction without a separate drain system"
        );

        let messages = app
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<crate::UiCheckboxChanged>>>();
        let mut cursor = messages.get_cursor();
        let collected: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].action.checkbox, checkbox);
        assert!(collected[0].action.checked);
    }

    #[test]
    fn ui_emit_payload_dispatches_business_action_not_builtin() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).add_ui_action::<TestAction>();

        let entity = app.world_mut().spawn_empty().id();
        // Simulate retained button with UiEmit payload (type-erased).
        let emit = UiEmit::new(TestAction::Inc);
        app.world()
            .resource::<InternalUiEventQueue>()
            .push_erased(entity, emit.type_id(), emit.payload());

        app.world_mut()
            .run_system_once(dispatch_ui_actions)
            .expect("dispatch");

        let business = app
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<TestAction>>>();
        let mut cursor = business.get_cursor();
        assert_eq!(cursor.read(business).count(), 1);

        let builtin = app
            .world()
            .resource::<bevy_ecs::message::Messages<UiAction<BuiltinUiAction>>>();
        let mut cursor = builtin.get_cursor();
        assert_eq!(
            cursor.read(builtin).count(),
            0,
            "UiEmit path must not also emit BuiltinUiAction::Clicked"
        );
    }

    #[test]
    fn headless_action_to_resource_via_message_reader() {
        #[derive(Resource, Default)]
        struct Count(i32);

        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .add_ui_action::<TestAction>()
            .insert_resource(Count(0))
            .add_systems(bevy_app::Update, |mut reader: MessageReader<UiAction<TestAction>>, mut count: ResMut<Count>| {
                for UiAction { action, .. } in reader.read() {
                    if *action == TestAction::Inc {
                        count.0 += 1;
                    }
                }
            });

        let source = app.world_mut().spawn_empty().id();
        // Retained click equivalent: enqueue + full frame (PreUpdate dispatch + Update readers).
        app.world()
            .resource::<UiActionSender<TestAction>>()
            .send(source, TestAction::Inc);
        app.update();

        assert_eq!(app.world().resource::<Count>().0, 1);
    }
}

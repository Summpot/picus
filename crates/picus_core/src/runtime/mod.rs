//! Per-window retained Masonry runtime and paint scheduling.
//!
//! [`frame_driver`] owns dirty-reason aggregation, the present decision table
//! (Phase 1), and structured [`RedrawDemand`] wake flags (Phase 1b). Execution
//! (anim tick, encode, present) remains on [`WindowRuntime`]; Bevy still receives
//! a single `RequestRedraw` when demand is non-empty.
//!
//! [`layers`] holds the Phase 2a Masonry layer-contract gate, [`AnimLayerHost`],
//! and the P2b painter-order [`LayerRegistry`] / [`CompositorPlan`] used by
//! ordered multi-texture composite.

pub(crate) mod frame_driver;
pub(crate) mod layers;

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::{Arc, Mutex, mpsc},
};

use self::frame_driver::{
    DirtyBudget, DirtyReason, FrameDecision, FrameDriver, FrameStepResult, RedrawDemand,
    anim_present_min_interval,
};
use self::layers::{
    CompositorEntryKind, LayerRegistry, VisualRun, coalesce_visual_runs,
};

use crate::masonry_core::{
    app::{RenderRoot, RenderRootOptions, RenderRootSignal, VisualLayerKind, WindowSizePolicy},
    core::{
        DefaultProperties, ErasedAction, Handled, PointerButton, PointerButtonEvent, PointerEvent,
        PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType, PointerUpdate,
        ScrollDelta, TextEvent, Widget, WidgetId, WidgetRef, WindowEvent,
        keyboard::{Key, KeyState, Modifiers, NamedKey},
    },
    dpi::{PhysicalPosition, PhysicalSize},
    layout::UnitPoint,
    peniko::Color,
    properties::Dimensions,
};
use crate::xilem::style::Style as _;
use crate::xilem::winit::window::Window as XilemWinitWindow;
use bevy_ecs::{
    entity::Entity,
    message::{MessageReader, MessageWriter},
    prelude::{Added, FromWorld, NonSendMut, Query, Res, ResMut, With, Without, World},
};
use bevy_input::{
    ButtonState,
    keyboard::{Key as BevyKey, KeyCode, KeyboardInput},
    mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel},
};
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_window::{
    ClosingWindow, CompositeAlphaMode, CursorLeft, CursorMoved, Ime as BevyIme, PrimaryWindow,
    RawHandleWrapper, RequestRedraw, Window, WindowFocused, WindowResized,
    WindowScaleFactorChanged, WindowWrapper,
};
use bevy_winit::{EventLoopProxy, EventLoopProxyWrapper, WinitUserEvent};
use picus_imaging::{Layer as ImagingLayer, PreparedFrame, texture_render::Renderer};
use picus_surface::{
    ExistingWindowMetrics, ExternalWindowSurface, LayerMetricsGeneration, OrderedEntryKind,
    OrderedLayerEncode, PresentPolicy, RenderFrameResult,
};
use picus_view::{
    ViewCtx,
    picus_widget::{
        properties::{ContentColor, PlaceholderColor},
        widgets::{
            Divider, Label as WidgetLabel, Passthrough, Spinner, TextArea,
            TextInput as WidgetTextInput,
        },
    },
    view::{label, sized_box, zstack},
};
use xilem::core::{
    DynMessage, MessageCtx, MessageResult, ProxyError, RawProxy, SendMessage, View, ViewId,
    ViewPathTracker,
};

use crate::{
    events::{
        InternalUiActionSink, InternalUiEventQueue, install_app_ui_action_sink,
        install_global_ui_event_queue,
    },
    fonts::{XilemFontBridge, font_bytes_fingerprint},
    overlay::OverlayPointerRoutingState,
    projection::{UiAnyView, UiView},
    synthesize::SynthesizedUiViews,
};

struct QueuedViewMessage {
    path: Vec<ViewId>,
    message: SendMessage,
}

struct ChannelProxy {
    sender: mpsc::Sender<QueuedViewMessage>,
    event_loop_proxy: Arc<Mutex<Option<EventLoopProxy<WinitUserEvent>>>>,
}

impl Debug for ChannelProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelProxy").finish_non_exhaustive()
    }
}

impl RawProxy for ChannelProxy {
    fn send_message(&self, path: Arc<[ViewId]>, message: SendMessage) -> Result<(), ProxyError> {
        match self.sender.send(QueuedViewMessage {
            path: path.as_ref().to_vec(),
            message,
        }) {
            Ok(()) => {
                if let Ok(proxy) = self.event_loop_proxy.lock()
                    && let Some(proxy) = proxy.as_ref()
                {
                    let _ = proxy.send_event(WinitUserEvent::WakeUp);
                }
                Ok(())
            }
            Err(err) => Err(ProxyError::DriverFinished(err.0.message)),
        }
    }

    fn dyn_debug(&self) -> &dyn Debug {
        self
    }
}

type RuntimeViewState = <UiAnyView as View<(), (), ViewCtx>>::ViewState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ImeWindowSignal {
    Start,
    End,
    Move(Vec2),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RedrawSignal {
    Redraw,
    AnimFrame,
}

/// Snapshot of content/scheduling stickies for restore on failed present.
///
/// Sticky content flags are cleared **only after successful present** so a
/// Retry / Failed / missing-surface / throttle skip cannot drop resize,
/// retry, or theme dirt (G5 defense-in-depth).
#[derive(Debug, Clone, Copy, Default)]
struct StickySnapshot {
    needs_redraw: bool,
    needs_anim_frame: bool,
    resize_dirty: bool,
    retry_dirty: bool,
    theme_or_font_dirty: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerTraceEvent {
    Move,
    Leave,
    Down,
    Up,
    Scroll,
}

/// Per-window retained Masonry runtime state.
///
/// One [`WindowRuntime`] exists for each Bevy window entity attached to the
/// UI runtime. It owns the retained `RenderRoot`, view state, pointer/keyboard
/// state, IME channel, wgpu surface, and Vello renderer for that window.
pub struct WindowRuntime {
    pub root_widget_id: WidgetId,
    pub render_root: RenderRoot,
    view_ctx: ViewCtx,
    pub widget_id_to_entity: HashMap<WidgetId, u64>,
    view_state: RuntimeViewState,
    current_view: UiView,
    window_entity: Entity,
    /// App-owned action sink shared by every window of the same Bevy `App`.
    action_sink: InternalUiActionSink,
    window_scale_factor: f64,
    window_transparent: bool,
    pointer_info: PointerInfo,
    pointer_state: PointerState,
    keyboard_modifiers: Modifiers,
    ime_signal_receiver: mpsc::Receiver<ImeWindowSignal>,
    action_signal_receiver: mpsc::Receiver<(ErasedAction, WidgetId)>,
    view_message_receiver: mpsc::Receiver<QueuedViewMessage>,
    redraw_signal_receiver: mpsc::Receiver<RedrawSignal>,
    needs_redraw: bool,
    needs_anim_frame: bool,
    has_painted_once: bool,
    /// Resize / scale metrics dirty — never anim-throttled (G5).
    resize_dirty: bool,
    /// Surface returned [`RenderFrameResult::Retry`] — never anim-throttled (G5).
    retry_dirty: bool,
    /// Theme/font registration dirty.
    theme_or_font_dirty: bool,
    /// Base CachedScene may be stale (rewrite completed during anim / geom move).
    /// Sticky until a **full-path** present succeeds — not anim-throttlable (Issue 10).
    base_invalidated: bool,
    /// Per-window frame scheduler (decision table + optional anim present throttle).
    frame_driver: FrameDriver,
    /// Painter-order compositor plan + anim host (P2b infrastructure).
    layer_registry: LayerRegistry,
    viewport_width: f64,
    viewport_height: f64,
    window_surface: Option<ExternalWindowSurface>,
    renderer: Renderer,
    registered_font_fingerprints: HashSet<u64>,
    #[cfg(test)]
    rebuild_count: usize,
    #[cfg(test)]
    pointer_trace: Vec<PointerTraceEvent>,
}

impl WindowRuntime {
    /// Install this window's app action sink as the active thread-local target
    /// for retained widget emissions.
    pub(crate) fn install_action_sink(&self) {
        install_app_ui_action_sink(self.action_sink.clone());
    }

    fn new(
        window_entity: Entity,
        initial_view: UiView,
        event_loop_proxy: Arc<Mutex<Option<EventLoopProxy<WinitUserEvent>>>>,
        action_sink: InternalUiActionSink,
    ) -> Self {
        // Ensure retained widgets built during construction write to this app.
        install_app_ui_action_sink(action_sink.clone());

        let (view_message_sender, view_message_receiver) = mpsc::channel::<QueuedViewMessage>();
        let mut view_ctx = ViewCtx::new(
            Arc::new(ChannelProxy {
                sender: view_message_sender,
                event_loop_proxy,
            }),
            Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime should initialize")),
        );
        let (ime_signal_sender, ime_signal_receiver) = mpsc::channel::<ImeWindowSignal>();
        let (action_signal_sender, action_signal_receiver) =
            mpsc::channel::<(ErasedAction, WidgetId)>();
        let (redraw_signal_sender, redraw_signal_receiver) = mpsc::channel::<RedrawSignal>();

        let (initial_root_widget, view_state) = <UiAnyView as View<(), (), ViewCtx>>::build(
            initial_view.as_ref(),
            &mut view_ctx,
            &mut (),
        );

        let options = RenderRootOptions {
            default_properties: Arc::new(picus_default_properties()),
            use_system_fonts: true,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(1024, 768),
            scale_factor: 1.0,
            test_font: None,
        };
        let initial_viewport = (options.size.width as f64, options.size.height as f64);

        let mut render_root = RenderRoot::new(
            initial_root_widget.new_widget.erased(),
            move |signal| match signal {
                RenderRootSignal::StartIme => {
                    let _ = ime_signal_sender.send(ImeWindowSignal::Start);
                }
                RenderRootSignal::EndIme => {
                    let _ = ime_signal_sender.send(ImeWindowSignal::End);
                }
                RenderRootSignal::ImeMoved(position, _size) => {
                    let _ = ime_signal_sender.send(ImeWindowSignal::Move(Vec2::new(
                        position.x as f32,
                        position.y as f32,
                    )));
                }
                RenderRootSignal::Action(action, source) => {
                    let _ = action_signal_sender.send((action, source));
                }
                RenderRootSignal::RequestRedraw => {
                    let _ = redraw_signal_sender.send(RedrawSignal::Redraw);
                }
                RenderRootSignal::RequestAnimFrame => {
                    let _ = redraw_signal_sender.send(RedrawSignal::AnimFrame);
                }
                _ => {}
            },
            options,
        );

        if let Some(fallback) = focus_fallback_widget(&render_root) {
            let _ = render_root.set_focus_fallback(Some(fallback));
        }

        let root_widget_id = render_root.get_layer_root(0).id();

        Self {
            root_widget_id,
            render_root,
            view_ctx,
            widget_id_to_entity: HashMap::new(),
            view_state,
            current_view: initial_view,
            window_entity,
            action_sink,
            window_scale_factor: 1.0,
            window_transparent: false,
            pointer_info: PointerInfo {
                pointer_id: Some(PointerId::new(1).expect("pointer id 1 should be valid")),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            pointer_state: PointerState::default(),
            keyboard_modifiers: Modifiers::empty(),
            ime_signal_receiver,
            action_signal_receiver,
            view_message_receiver,
            redraw_signal_receiver,
            needs_redraw: true,
            needs_anim_frame: true,
            has_painted_once: false,
            resize_dirty: false,
            retry_dirty: false,
            theme_or_font_dirty: false,
            base_invalidated: false,
            frame_driver: FrameDriver::new(),
            layer_registry: LayerRegistry::default(),
            viewport_width: initial_viewport.0,
            viewport_height: initial_viewport.1,
            window_surface: None,
            renderer: Renderer::new(),
            registered_font_fingerprints: HashSet::new(),
            #[cfg(test)]
            rebuild_count: 0,
            #[cfg(test)]
            pointer_trace: Vec::new(),
        }
    }

    fn initial_placeholder_view() -> UiView {
        Arc::new(label("picus_core: waiting for synthesized root"))
    }

    #[must_use]
    pub fn window_entity(&self) -> Entity {
        self.window_entity
    }

    #[must_use]
    pub fn is_attached_to_window(&self, window: Entity) -> bool {
        self.window_entity == window
    }

    pub fn attach_to_window(&mut self, metrics: ExistingWindowMetrics) {
        self.sync_window_metrics(metrics);
    }

    #[must_use]
    pub fn viewport_size(&self) -> (f64, f64) {
        (self.viewport_width.max(1.0), self.viewport_height.max(1.0))
    }

    #[must_use]
    pub fn get_hit_path(
        &self,
        physical_pos: crate::masonry_core::kurbo::Point,
    ) -> Vec<crate::masonry_core::core::WidgetId> {
        let target = self
            .render_root
            .pointer_capture_target()
            .filter(|widget_id| self.render_root.has_widget(*widget_id))
            .or_else(|| {
                let scale_factor = self.window_scale_factor.max(f64::EPSILON);
                let logical_pos = crate::masonry_core::kurbo::Point::new(
                    physical_pos.x / scale_factor,
                    physical_pos.y / scale_factor,
                );

                self.render_root
                    .get_layer_root(0)
                    .find_widget_under_pointer(logical_pos)
                    .map(|widget| widget.id())
            });

        let Some(target) = target else {
            return Vec::new();
        };

        let mut path = Vec::new();
        if build_widget_path(self.render_root.get_layer_root(0), target, &mut path) {
            path
        } else {
            Vec::new()
        }
    }

    #[must_use]
    pub fn find_widget_id_for_entity_bits(
        &self,
        entity_bits: u64,
        prefer_opaque_hitbox: bool,
    ) -> Option<WidgetId> {
        fn walk(
            widget: WidgetRef<'_, dyn Widget>,
            entity_bits: u64,
            prefer_opaque_hitbox: bool,
        ) -> Option<WidgetId> {
            if widget.ctx().is_stashed() {
                return None;
            }

            if let Some(debug) = widget.get_debug_text()
                && let Some((bits, is_opaque_hitbox)) = parse_entity_debug_binding(&debug)
                && bits == entity_bits
                && (!prefer_opaque_hitbox || is_opaque_hitbox)
            {
                return Some(widget.id());
            }

            for child in widget.children() {
                if let Some(id) = walk(child, entity_bits, prefer_opaque_hitbox) {
                    return Some(id);
                }
            }

            None
        }

        let root = self.render_root.get_layer_root(0);
        walk(root, entity_bits, prefer_opaque_hitbox)
    }

    #[must_use]
    pub fn find_widget_ids_for_entity_bits(&self, entity_bits: u64) -> Vec<WidgetId> {
        fn walk(widget: WidgetRef<'_, dyn Widget>, entity_bits: u64, matches: &mut Vec<WidgetId>) {
            if widget.ctx().is_stashed() {
                return;
            }

            for child in widget.children() {
                walk(child, entity_bits, matches);
            }

            let Some(debug) = widget.get_debug_text() else {
                return;
            };

            let Some((bits, _is_opaque_hitbox)) = parse_entity_debug_binding(&debug) else {
                return;
            };

            if bits == entity_bits {
                matches.push(widget.id());
            }
        }

        let root = self.render_root.get_layer_root(0);
        let mut matches = Vec::new();
        walk(root, entity_bits, &mut matches);
        matches
    }

    /// Returns `(bevy_window_scale_factor, masonry_global_scale_factor)` for diagnostics.
    #[must_use]
    pub fn masonry_scale_factors(&self) -> (f64, f64) {
        (self.window_scale_factor, self.window_scale_factor)
    }

    /// Walk the widget tree and rebuild the WidgetId → entity_bits reverse map.
    pub fn populate_entity_map(&mut self) {
        self.widget_id_to_entity.clear();
        fn walk(widget: WidgetRef<'_, dyn Widget>, map: &mut HashMap<WidgetId, u64>) {
            if widget.ctx().is_stashed() {
                return;
            }
            let _ = widget
                .get_debug_text()
                .and_then(|d| parse_entity_debug_binding(&d))
                .map(|(bits, _)| map.insert(widget.id(), bits));
            for child in widget.children() {
                walk(child, map);
            }
        }
        let root = self.render_root.get_layer_root(0);
        walk(root, &mut self.widget_id_to_entity);
    }

    /// Returns the bounding box of a widget by its id, for diagnostics.
    #[must_use]
    pub fn get_widget_bounding_box(
        &self,
        id: crate::masonry_core::core::WidgetId,
    ) -> Option<crate::masonry_core::kurbo::Rect> {
        self.render_root
            .get_widget(id)
            .map(|w| w.ctx().bounding_box())
    }

    /// Returns all layer-0 widget IDs that are direct children of the overlay-root zstack,
    /// for diagnostics. Returns (widget_id, bounding_box) pairs.
    #[must_use]
    pub fn get_overlay_subtree_info(
        &self,
        overlay_widget_id: crate::masonry_core::core::WidgetId,
    ) -> Vec<(
        crate::masonry_core::core::WidgetId,
        crate::masonry_core::kurbo::Rect,
        bool,
    )> {
        let Some(esw) = self.render_root.get_widget(overlay_widget_id) else {
            return vec![];
        };
        let mut result = vec![(
            overlay_widget_id,
            esw.ctx().bounding_box(),
            esw.ctx().is_stashed(),
        )];
        for child in esw.children() {
            result.push((
                child.id(),
                child.ctx().bounding_box(),
                child.ctx().is_stashed(),
            ));
        }
        result
    }

    #[cfg(test)]
    pub(crate) fn pointer_position_for_tests(&self) -> Vec2 {
        Vec2::new(
            self.pointer_state.position.x as f32,
            self.pointer_state.position.y as f32,
        )
    }

    #[cfg(test)]
    pub(crate) fn pointer_trace_for_tests(&self) -> &[PointerTraceEvent] {
        &self.pointer_trace
    }

    #[cfg(test)]
    pub(crate) fn rebuild_count_for_tests(&self) -> usize {
        self.rebuild_count
    }

    #[cfg(test)]
    pub(crate) fn clear_pointer_trace_for_tests(&mut self) {
        self.pointer_trace.clear();
    }

    pub fn rebuild_root_view(&mut self, next_view: UiView) {
        #[cfg(test)]
        {
            self.rebuild_count += 1;
        }

        self.render_root.edit_base_layer(|mut root| {
            let mut root = root.downcast::<Passthrough>();
            <UiAnyView as View<(), (), ViewCtx>>::rebuild(
                next_view.as_ref(),
                self.current_view.as_ref(),
                &mut self.view_state,
                &mut self.view_ctx,
                root.reborrow_mut(),
                &mut (),
            );
            self.root_widget_id = root.widget.inner_id();
        });

        self.current_view = next_view;
        self.needs_redraw = true;

        if let Some(fallback) = focus_fallback_widget(&self.render_root) {
            let _ = self.render_root.set_focus_fallback(Some(fallback));
        }
    }

    /// Route widget actions and async proxy messages emitted since the last
    /// call back to their source view's [`View::message`] handler.
    ///
    /// Masonry widgets submit actions during the rewrite passes (triggered by
    /// input injection). Actions not consumed by ancestor widgets' `on_action`
    /// are emitted as [`RenderRootSignal::Action`], which the per-window signal
    /// sink captures into `action_signal_receiver`. Xilem tasks and other
    /// proxy-backed views submit messages through `view_message_receiver`.
    /// This method drains both queues and dispatches each message to the
    /// corresponding view, so callback-based views such as `text_input` can
    /// fire their `on_changed`/`on_enter` callbacks and async task views can
    /// deliver their output.
    ///
    /// Must be called after input injection and before the ECS action-drain
    /// systems run, so that callback-emitted UI actions are visible in the same
    /// frame.
    pub fn route_pending_view_messages(&mut self) {
        let view_messages: Vec<QueuedViewMessage> = self.view_message_receiver.try_iter().collect();
        let actions: Vec<(ErasedAction, WidgetId)> =
            self.action_signal_receiver.try_iter().collect();
        if view_messages.is_empty() && actions.is_empty() {
            return;
        }

        for queued in view_messages {
            self.route_view_message_at_path(queued.path, queued.message);
        }

        for (action, source) in actions {
            self.route_view_message(action, source);
        }
    }

    fn route_view_message(&mut self, action: ErasedAction, source: WidgetId) -> bool {
        let Some(path) = self.view_ctx.get_id_path(source).cloned() else {
            tracing::debug!(
                "route_pending_view_messages: no view path for widget {:?}, dropping {:?}",
                source,
                action.type_name()
            );
            return false;
        };

        self.route_view_message_at_path(path, SendMessage(action))
    }

    fn route_view_message_at_path(&mut self, path: Vec<ViewId>, message: SendMessage) -> bool {
        let env = std::mem::take(self.view_ctx.environment());
        let message = DynMessage::from(message);
        let mut ctx = MessageCtx::new(env, path, message);

        let result: MessageResult<()> = self.render_root.edit_base_layer(|mut root| {
            let mut root = root.downcast::<Passthrough>();
            <UiAnyView as View<(), (), ViewCtx>>::message(
                self.current_view.as_ref(),
                &mut self.view_state,
                &mut ctx,
                root.reborrow_mut(),
                &mut (),
            )
        });

        let (env, _, _) = ctx.finish();
        *self.view_ctx.environment() = env;
        if matches!(result, MessageResult::RequestRebuild) {
            self.needs_redraw = true;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn route_test_view_message(
        &mut self,
        action: ErasedAction,
        source: WidgetId,
    ) -> bool {
        self.route_view_message(action, source)
    }

    pub fn handle_cursor_moved(&mut self, x: f32, y: f32) -> Handled {
        self.pointer_state.position = PhysicalPosition {
            x: x as f64,
            y: y as f64,
        };

        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Move);

        self.render_root
            .handle_pointer_event(PointerEvent::Move(PointerUpdate {
                pointer: self.pointer_info,
                current: self.pointer_state.clone(),
                coalesced: vec![],
                predicted: vec![],
            }))
    }

    pub fn handle_cursor_left(&mut self) -> Handled {
        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Leave);

        self.render_root
            .handle_pointer_event(PointerEvent::Leave(self.pointer_info))
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, state: ButtonState) -> Handled {
        let Some(button) = map_mouse_button(button) else {
            return Handled::No;
        };

        match state {
            ButtonState::Pressed => {
                self.pointer_state.buttons.insert(button);
                #[cfg(test)]
                self.pointer_trace.push(PointerTraceEvent::Down);
                self.render_root
                    .handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
                        pointer: self.pointer_info,
                        button: Some(button),
                        state: self.pointer_state.clone(),
                    }))
            }
            ButtonState::Released => {
                self.pointer_state.buttons.remove(button);
                #[cfg(test)]
                self.pointer_trace.push(PointerTraceEvent::Up);
                self.render_root
                    .handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
                        pointer: self.pointer_info,
                        button: Some(button),
                        state: self.pointer_state.clone(),
                    }))
            }
        }
    }

    pub fn handle_mouse_wheel(&mut self, unit: MouseScrollUnit, x: f32, y: f32) -> Handled {
        let factor = if unit == MouseScrollUnit::Line {
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            1.0
        };

        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Scroll);

        self.render_root
            .handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
                pointer: self.pointer_info,
                delta: ScrollDelta::PixelDelta(PhysicalPosition {
                    x: (x * factor) as f64,
                    y: (y * factor) as f64,
                }),
                state: self.pointer_state.clone(),
            }))
    }

    pub fn handle_text_event(&mut self, event: TextEvent) -> Handled {
        self.render_root.handle_text_event(event)
    }

    pub fn handle_window_resized(&mut self, width: f32, height: f32) -> Handled {
        self.viewport_width = width.max(1.0) as f64;
        self.viewport_height = height.max(1.0) as f64;
        self.needs_redraw = true;
        self.resize_dirty = true;

        let scale = self.window_scale_factor.max(f64::EPSILON);
        let physical_width = (self.viewport_width * scale).round().max(1.0) as u32;
        let physical_height = (self.viewport_height * scale).round().max(1.0) as u32;

        self.render_root
            .handle_window_event(WindowEvent::Resize(PhysicalSize::new(
                physical_width,
                physical_height,
            )))
    }

    pub fn handle_window_scale_factor_changed(&mut self, scale_factor: f64) -> Handled {
        self.window_scale_factor = scale_factor.max(f64::EPSILON);
        self.needs_redraw = true;
        self.resize_dirty = true;
        let _ = self
            .render_root
            .handle_window_event(WindowEvent::Rescale(self.window_scale_factor));

        let physical_width = (self.viewport_width * self.window_scale_factor)
            .round()
            .max(1.0) as u32;
        let physical_height = (self.viewport_height * self.window_scale_factor)
            .round()
            .max(1.0) as u32;

        self.render_root
            .handle_window_event(WindowEvent::Resize(PhysicalSize::new(
                physical_width,
                physical_height,
            )))
    }

    pub fn ensure_external_surface(
        &mut self,
        window: &WindowWrapper<crate::xilem::winit::window::Window>,
        metrics: ExistingWindowMetrics,
    ) -> bool {
        if let Some(surface) = self.window_surface.as_mut() {
            if surface.sync_window_metrics(metrics) {
                self.needs_redraw = true;
                self.resize_dirty = true;
            }
            return true;
        }

        let raw_handle = match RawHandleWrapper::new(window) {
            Ok(raw_handle) => raw_handle,
            Err(error) => {
                tracing::error!("failed to create raw window handle for Masonry surface: {error}");
                return false;
            }
        };

        match ExternalWindowSurface::new_from_bevy_raw_handle_with_policy(
            raw_handle,
            metrics,
            PresentPolicy::default_ui(),
        ) {
            Ok(surface) => {
                tracing::debug!(
                    capability = ?surface.negotiated_present().capability,
                    mode = ?surface.negotiated_present().mode,
                    "Masonry surface present policy ready"
                );
                self.window_surface = Some(surface);
                self.needs_redraw = true;
                self.has_painted_once = false;
                true
            }
            Err(error) => {
                tracing::error!("failed to initialize external Masonry surface: {error}");
                false
            }
        }
    }

    fn drain_redraw_signals(&mut self) {
        for signal in self.redraw_signal_receiver.try_iter() {
            match signal {
                RedrawSignal::Redraw => self.needs_redraw = true,
                RedrawSignal::AnimFrame => self.needs_anim_frame = true,
            }
        }
    }

    /// Collect dirty reasons from sticky flags + Masonry signals + layer registry.
    fn collect_dirty_budget(&self) -> DirtyBudget {
        let mut dirty = DirtyBudget::new();
        if !self.has_painted_once {
            dirty.insert(DirtyReason::FirstPaint);
        }
        if self.resize_dirty {
            dirty.insert(DirtyReason::ResizeMetrics);
        }
        if self.retry_dirty {
            dirty.insert(DirtyReason::RetrySurface);
        }
        if self.theme_or_font_dirty {
            dirty.insert(DirtyReason::ThemeOrFont);
        }
        // Sticky base invalidation is content dirt (G5 unthrottled via InputOrRebuild).
        if self.base_invalidated || self.needs_redraw {
            // Generic content/input redraw when not already tagged finer.
            dirty.insert(DirtyReason::InputOrRebuild);
        }
        if self.needs_anim_frame || self.render_root.needs_anim() {
            dirty.insert(DirtyReason::AnimTick);
        }
        if self.render_root.needs_rewrite_passes() {
            dirty.insert(DirtyReason::LayoutRewrite);
        }
        if self.layer_registry.plan_changed() {
            dirty.insert(DirtyReason::CompositorPlan);
        }
        for layer in self.layer_registry.host().dirty_anim_paint_layers() {
            dirty.insert(DirtyReason::AnimPaint { layer });
        }
        dirty
    }

    fn take_sticky_snapshot(&self) -> StickySnapshot {
        StickySnapshot {
            needs_redraw: self.needs_redraw,
            needs_anim_frame: self.needs_anim_frame,
            resize_dirty: self.resize_dirty,
            retry_dirty: self.retry_dirty,
            theme_or_font_dirty: self.theme_or_font_dirty,
        }
    }

    /// Re-arm stickies that motivated this frame after a non-successful present.
    ///
    /// OR-merge so signals drained mid-frame are not clobbered.
    fn restore_sticky_snapshot(&mut self, snap: StickySnapshot) {
        self.needs_redraw |= snap.needs_redraw;
        self.needs_anim_frame |= snap.needs_anim_frame;
        self.resize_dirty |= snap.resize_dirty;
        self.retry_dirty |= snap.retry_dirty;
        self.theme_or_font_dirty |= snap.theme_or_font_dirty;
    }

    /// Clear only the stickies that this successful present fulfilled.
    fn clear_sticky_snapshot(&mut self, snap: StickySnapshot) {
        // New dirt raised after the snapshot (e.g. mid-frame resize signal) is
        // kept; only bits that were set when we started are cleared.
        if snap.needs_redraw {
            self.needs_redraw = false;
        }
        if snap.needs_anim_frame {
            // Anim may re-request during the tick; keep if still needed.
            if !self.render_root.needs_anim() {
                self.needs_anim_frame = false;
            }
        }
        if snap.resize_dirty {
            self.resize_dirty = false;
        }
        if snap.retry_dirty {
            self.retry_dirty = false;
        }
        if snap.theme_or_font_dirty {
            self.theme_or_font_dirty = false;
        }
    }

    /// Structured Bevy wake demand after work (Phase 1b).
    ///
    /// Splits anim-clock scheduling from content encode/present so callers can
    /// measure and document why `RequestRedraw` is written. Both still collapse
    /// to one Bevy `RequestRedraw` (reactive mode has no paint-only schedule).
    ///
    /// `need_content_present` here is **host sticky/rewrite wake** — not
    /// [`DirtyBudget::needs_content_present`] (decision-table / encode path).
    fn redraw_demand_after_work(&self) -> RedrawDemand {
        let need_anim_tick = self.needs_anim_frame || self.render_root.needs_anim();
        let need_content_present = self.needs_redraw
            || self.resize_dirty
            || self.retry_dirty
            || self.theme_or_font_dirty
            || self.base_invalidated
            || self.render_root.needs_rewrite_passes();
        RedrawDemand {
            need_anim_tick,
            need_content_present,
        }
    }

    /// Independent anim clock only — used after permanent present failure so
    /// retained content stickies do not spin a Bevy redraw loop (Issue 12).
    fn redraw_demand_anim_clock_only(&self) -> RedrawDemand {
        RedrawDemand {
            need_anim_tick: self.needs_anim_frame || self.render_root.needs_anim(),
            need_content_present: false,
        }
    }

    /// G5 content-path wake: after arming stickies, force `need_content_present`
    /// so Retry / missing-surface / missing-root cannot silently drop content
    /// demand if `redraw_demand_after_work` classification changes later.
    fn redraw_demand_force_content_present(&self) -> RedrawDemand {
        let mut demand = self.redraw_demand_after_work();
        demand.need_content_present = true;
        demand
    }

    #[inline]
    fn wants_redraw_after_work(&self) -> bool {
        self.redraw_demand_after_work().any()
    }

    #[inline]
    fn wants_redraw_anim_clock_only(&self) -> bool {
        self.redraw_demand_anim_clock_only().any()
    }

    /// After skipping encode (pure AnimTick or optional anim present throttle):
    /// restore content stickies, but **drop residual post-tick AnimPaint**
    /// `needs_redraw` so the next frame is not G5-classified as `InputOrRebuild`
    /// (Issue 11). Keep `needs_anim_frame` when throttled so the anim clock continues.
    fn finish_encode_skip(
        &mut self,
        snap: StickySnapshot,
        anim_raised_redraw: bool,
        throttled: bool,
    ) {
        self.restore_sticky_snapshot(snap);
        // Deferred AnimPaint is re-derived on the next AnimFrame; leaving
        // needs_redraw armed would map to InputOrRebuild and bypass any
        // diagnostic anim present throttle.
        if anim_raised_redraw && !snap.needs_redraw {
            self.needs_redraw = false;
        }
        // Base invalidation must survive throttle skips until a full present (Issue 10).
        if self.base_invalidated {
            self.needs_redraw = true;
        }
        if throttled {
            self.needs_anim_frame = true;
        }
    }

    /// Apply sticky / redraw policy after a present attempt.
    ///
    /// Returns structured [`RedrawDemand`] (Phase 1b) — Bevy still receives a
    /// single `RequestRedraw` when [`RedrawDemand::any`] is true.
    /// - [`RenderFrameResult::Presented`]: clear fulfilled stickies.
    /// - [`RenderFrameResult::Retry`]: restore stickies + force content present
    ///   (transient surface; G5 unthrottled).
    /// - [`RenderFrameResult::Failed`]: restore stickies for a later external wake
    ///   but **do not** force content present / permanent redraw loop (Issue 12).
    fn finish_present_attempt(
        &mut self,
        result: RenderFrameResult,
        snap: StickySnapshot,
        anim_driven_present: bool,
        now: std::time::Instant,
    ) -> RedrawDemand {
        match result {
            RenderFrameResult::Presented => {
                self.has_painted_once = true;
                self.clear_sticky_snapshot(snap);
                self.needs_redraw = false;
                if anim_driven_present {
                    self.frame_driver.note_anim_present(now);
                }
                self.redraw_demand_after_work()
            }
            RenderFrameResult::Retry => {
                self.restore_sticky_snapshot(snap);
                self.needs_redraw = true;
                self.retry_dirty = true;
                // Explicit content force (defense-in-depth; matches throttled
                // encode-skip's explicit need_anim_tick force).
                self.redraw_demand_force_content_present()
            }
            RenderFrameResult::Failed => {
                // Retain content stickies for bookkeeping / later external wake
                // (input, resize). Do not set needs_redraw or spin content RequestRedraw.
                self.restore_sticky_snapshot(snap);
                self.redraw_demand_anim_clock_only()
            }
        }
    }

    /// Frame scheduling spine: [`FrameDriver::decide_entry`] /
    /// [`FrameDriver::decide_present`] decide; this method executes.
    ///
    /// Phase 1 execution split is **anim-tick vs encode/present** (full-window
    /// rewrite+encode+present stay coupled when content present is required).
    /// `FrameDecision::{do_rewrite,do_encode,do_present}` record intent; the
    /// host only branches on `do_anim_tick` and `do_encode` until Phase 2 layers.
    ///
    /// Called from [`paint_masonry_ui`] as `window_runtime.step_frame(delta)`.
    fn step_frame(&mut self, delta: std::time::Duration) -> FrameStepResult {
        self.drain_redraw_signals();

        let pre_dirty = self.collect_dirty_budget();
        let entry = FrameDriver::decide_entry(&pre_dirty);
        if !entry.enter_work {
            return FrameStepResult::skipped();
        }

        let mut paint_reasons = FrameDriver::paint_reasons_mask(&pre_dirty);
        if self.render_root.needs_anim() {
            paint_reasons |= crate::perf::PaintReason::RenderRootNeedsAnim as u32;
        }
        let mut phases = crate::perf::PaintPhaseTimings::default();

        // Snapshot stickies for decision + failed-path restore. Content stickies
        // clear only after successful present (Issue 1).
        let snap = self.take_sticky_snapshot();
        let first_paint = !self.has_painted_once;
        let had_unthrottled = pre_dirty.requires_unthrottled_present();

        // Consume anim-frame request for this attempt; content stickies stay.
        self.needs_anim_frame = false;

        // Temporarily clear redraw so post-tick AnimPaint is distinguishable from
        // pre-tick InputOrRebuild. Content dirt is remembered in `snap`.
        self.needs_redraw = false;

        // AnimFrame may complete rewrite that was already pending — base textures
        // would go stale if we took the pure-anim selective path afterward (Issue 1).
        let rewrite_pending_before_anim = self.render_root.needs_rewrite_passes();

        if entry.do_anim_tick {
            // AnimFrame may run rewrite and emit RequestRedraw when pixels change.
            // That rewrite cost is attributed to `phases.anim_tick` (perf honesty).
            let anim_started = std::time::Instant::now();
            let _ = self
                .render_root
                .handle_window_event(WindowEvent::AnimFrame(delta));
            phases.anim_tick = anim_started.elapsed();
            self.drain_redraw_signals();
        }

        let anim_raised_redraw = self.needs_redraw;
        // Keep content sticky armed until present succeeds or restore on failure.
        if snap.needs_redraw {
            self.needs_redraw = true;
        }

        // Post-tick dirty budget for encode/present decision.
        let mut post_dirty = DirtyBudget::new();
        if first_paint {
            post_dirty.insert(DirtyReason::FirstPaint);
        }
        if snap.resize_dirty {
            post_dirty.insert(DirtyReason::ResizeMetrics);
        }
        if snap.retry_dirty {
            post_dirty.insert(DirtyReason::RetrySurface);
        }
        if snap.theme_or_font_dirty {
            post_dirty.insert(DirtyReason::ThemeOrFont);
        }
        if snap.needs_redraw || self.base_invalidated {
            // Pre-tick redraw / sticky base invalidation is content (unthrottled).
            post_dirty.insert(DirtyReason::InputOrRebuild);
        }
        // Always re-merge retained host dirty into post_dirty (Issue 13).
        // After Failed selective present, phase may already be unacked or acked;
        // host.dirty must still force AnimPaint so encode/present is retried.
        // (pre_dirty already includes host dirty; post_dirty is rebuilt from scratch.)
        let mut any_host_dirty = false;
        for layer in self.layer_registry.host().dirty_anim_paint_layers() {
            post_dirty.insert(DirtyReason::AnimPaint { layer });
            any_host_dirty = true;
        }
        if anim_raised_redraw {
            // Raised during AnimFrame (e.g. Spinner phase change → request_paint_only).
            if !any_host_dirty {
                post_dirty.insert(DirtyReason::AnimPaint { layer: 0 });
            }
            self.needs_redraw = true;
        }
        // Outstanding rewrite, or rewrite that completed during this AnimFrame
        // (flag cleared) — both require full base reassembly, not selective anim.
        // Sticky `base_invalidated` so anim throttle cannot drop the only frame
        // that knew rewrite completed (Issue 10).
        let rewrite_pending_after_anim = self.render_root.needs_rewrite_passes();
        let rewrite_completed_during_anim =
            rewrite_pending_before_anim && !rewrite_pending_after_anim;
        if rewrite_pending_after_anim || rewrite_completed_during_anim {
            post_dirty.insert(DirtyReason::LayoutRewrite);
            self.base_invalidated = true;
            self.needs_redraw = true;
            // Promote to unthrottled content dirt so present cannot be skipped.
            post_dirty.insert(DirtyReason::InputOrRebuild);
        }
        if self.needs_anim_frame || self.render_root.needs_anim() {
            post_dirty.insert(DirtyReason::AnimTick);
        }

        let now = std::time::Instant::now();
        let present_decision =
            self.frame_driver
                .decide_present(&post_dirty, anim_present_min_interval(), now);

        if !present_decision.do_encode {
            paint_reasons |= crate::perf::PaintReason::AnimTickNoPresent as u32;
            // Restore G5 stickies (defense-in-depth) but clear residual AnimPaint
            // redraw so the next frame stays anim-throttlable (Issues 1 + 11).
            self.finish_encode_skip(
                snap,
                anim_raised_redraw,
                present_decision.throttled_anim_present,
            );
            // Throttled AnimPaint is still anim scheduling (timeline B), not content.
            let mut redraw_demand = self.redraw_demand_after_work();
            if present_decision.throttled_anim_present {
                redraw_demand.need_anim_tick = true;
            }
            return FrameStepResult {
                painted: false,
                redraw_demand,
                anim_tick_only: true,
                paint_reasons,
                phases,
                decision: present_decision,
            };
        }

        let logical_size = self.render_root.size();
        let window_bounds = crate::masonry_core::kurbo::Rect::new(
            0.0,
            0.0,
            logical_size.width.max(1) as f64,
            logical_size.height.max(1) as f64,
        );

        // Take surface so we can borrow registry/renderer disjointly (P2.6 path).
        let Some(mut surface) = self.window_surface.take() else {
            self.restore_sticky_snapshot(snap);
            self.needs_redraw = true;
            self.retry_dirty = true;
            // Missing surface: explicitly force content present (G5 path).
            return FrameStepResult {
                painted: false,
                redraw_demand: self.redraw_demand_force_content_present(),
                anim_tick_only: false,
                paint_reasons,
                phases,
                decision: present_decision,
            };
        };

        // Single source of truth for layer target size: surface physical config
        // (from Bevy metrics), not a recomputed logical*scale (Issue 8).
        // **Must run before selective decision**: metrics invalidation dirties all
        // entries; selective with visual=None must not encode empty base (Issue 2).
        let (physical_w, physical_h) = surface.physical_size();
        self.layer_registry
            .notify_metrics_changed(physical_w, physical_h);
        if first_paint {
            self.layer_registry.notify_first_paint();
        }

        // P2c / G2: pure AnimPaint with a stable ordered plan → skip full-tree
        // redraw and base reassembly; only host anim scenes + dirty encode.
        // Guards (Issues 1–2):
        // - LayoutRewrite in post_dirty if rewrite pending/completed during anim
        // - No CachedScene/Overlay/External needs_encode after metrics notify
        // - Prefer full path whenever non-anim structure/content is dirty
        let selective_anim = post_dirty.is_selective_anim_encode()
            && self.has_painted_once
            && !self.base_invalidated
            && self.layer_registry.has_anim_entries()
            && self.layer_registry.prefers_ordered_composite()
            && !self.render_root.needs_rewrite_passes()
            && !self.layer_registry.non_anim_needs_encode();

        let background = if self.window_transparent {
            Color::TRANSPARENT
        } else {
            Color::BLACK
        };
        let frame_w = logical_size.width.max(1);
        let frame_h = logical_size.height.max(1);
        let scale = self.window_scale_factor;

        // True when this present attempt used (or fell back to) a full base rebuild.
        let mut used_full_base_path = !selective_anim;

        let (render_result, surface_timings) = if selective_anim {
            // Steady Spinner anim: no base scene rebuild, no full redraw.
            let anim_started = std::time::Instant::now();
            let sync_summary = self
                .layer_registry
                .sync_anim_entries_from_widgets(&self.render_root);
            phases.scene_build_anim = anim_started.elapsed();
            phases.scene_build_base = std::time::Duration::ZERO;

            // Geometry change under selective path implies layout may have moved
            // base content (Issue 11 partial detection) — force full reassembly.
            if sync_summary.any_geometry_changed {
                self.base_invalidated = true;
            }

            // Defense: if sync/propagate somehow dirtied base, or geom moved, full path.
            if self.layer_registry.non_anim_needs_encode() || sync_summary.any_geometry_changed {
                used_full_base_path = true;
                // Fall through by rebuilding with a full redraw instead of empty base.
                let bound_ids: Vec<_> = self.layer_registry.host().widget_ids().collect();
                for id in bound_ids {
                    if !self.render_root.has_widget(id) {
                        let _ = self.layer_registry.host_mut().remove_widget(id);
                        continue;
                    }
                    self.render_root.edit_widget(id, |mut w| {
                        w.ctx.request_paint_only();
                    });
                }
                let scene_started = std::time::Instant::now();
                let (visual_layers, _tree_update) = self.render_root.redraw();
                phases.scene_build_base = scene_started.elapsed();
                self.layer_registry
                    .register_external_widgets_from_visual(&visual_layers, &self.render_root);
                self.layer_registry
                    .rebuild_from_visual_plan(&visual_layers, window_bounds);
                self.layer_registry.mark_non_anim_content_dirty();
                let _ = self
                    .layer_registry
                    .sync_anim_entries_from_widgets(&self.render_root);
                surface.sync_layer_metrics_generation(LayerMetricsGeneration(
                    self.layer_registry.metrics_generation(),
                ));
                surface.retain_layer_targets(&self.layer_registry.live_layer_id_raws());
                encode_ordered_composite(
                    &self.layer_registry,
                    &mut self.renderer,
                    &mut surface,
                    Some(&visual_layers),
                    background,
                    frame_w,
                    frame_h,
                    scale,
                )
            } else {
                surface.sync_layer_metrics_generation(LayerMetricsGeneration(
                    self.layer_registry.metrics_generation(),
                ));
                surface.retain_layer_targets(&self.layer_registry.live_layer_id_raws());

                encode_ordered_composite(
                    &self.layer_registry,
                    &mut self.renderer,
                    &mut surface,
                    None,
                    background,
                    frame_w,
                    frame_h,
                    scale,
                )
            }
        } else {
            // Content / first-paint / structure path: full Masonry redraw.
            // Re-request paint on bound External widgets so mode is re-set
            // (paint_layer_mode is not sticky across clean widgets).
            let bound_ids: Vec<_> = self.layer_registry.host().widget_ids().collect();
            for id in bound_ids {
                if !self.render_root.has_widget(id) {
                    let _ = self.layer_registry.host_mut().remove_widget(id);
                    continue;
                }
                self.render_root.edit_widget(id, |mut w| {
                    w.ctx.request_paint_only();
                });
            }

            let scene_started = std::time::Instant::now();
            let (visual_layers, _tree_update) = self.render_root.redraw();
            phases.scene_build_base = scene_started.elapsed();

            // Bind Spinner External slots → Anim (type-gated host painter).
            self.layer_registry
                .register_external_widgets_from_visual(&visual_layers, &self.render_root);
            self.layer_registry
                .rebuild_from_visual_plan(&visual_layers, window_bounds);
            if self.layer_registry.plan_changed() {
                post_dirty.insert(DirtyReason::CompositorPlan);
            }

            // Issue 2: ordinary content dirt must bump CachedScene/Overlay versions.
            // Pure AnimPaint alone leaves base clean for G2 selective encode.
            let non_anim_content_dirt = post_dirty.iter().any(|r| {
                matches!(
                    r,
                    DirtyReason::FirstPaint
                        | DirtyReason::InputOrRebuild
                        | DirtyReason::LayoutRewrite
                        | DirtyReason::ResizeMetrics
                        | DirtyReason::ThemeOrFont
                        | DirtyReason::RetrySurface
                        | DirtyReason::CompositorPlan
                )
            });
            if non_anim_content_dirt {
                self.layer_registry.mark_non_anim_content_dirty();
            }

            let anim_started = std::time::Instant::now();
            let _ = self
                .layer_registry
                .sync_anim_entries_from_widgets(&self.render_root);
            phases.scene_build_anim = anim_started.elapsed();

            surface.sync_layer_metrics_generation(LayerMetricsGeneration(
                self.layer_registry.metrics_generation(),
            ));
            surface.retain_layer_targets(&self.layer_registry.live_layer_id_raws());

            let prefer_ordered = self.layer_registry.prefers_ordered_composite();
            if prefer_ordered {
                encode_ordered_composite(
                    &self.layer_registry,
                    &mut self.renderer,
                    &mut surface,
                    Some(&visual_layers),
                    background,
                    frame_w,
                    frame_h,
                    scale,
                )
            } else {
                // Single CachedScene path: existing full-window flatten encode.
                let overlays = visual_layers
                    .overlay_layers()
                    .map(|layer| {
                        let VisualLayerKind::Scene(scene) = &layer.kind else {
                            unreachable!("overlay_layers only returns scene layers");
                        };
                        ImagingLayer {
                            scene,
                            transform: layer.transform,
                        }
                    })
                    .collect::<Vec<_>>();
                let Some(root_layer) = visual_layers.root_layer() else {
                    self.window_surface = Some(surface);
                    self.restore_sticky_snapshot(snap);
                    self.needs_redraw = true;
                    self.retry_dirty = true;
                    return FrameStepResult {
                        painted: false,
                        redraw_demand: self.redraw_demand_force_content_present(),
                        anim_tick_only: false,
                        paint_reasons,
                        phases,
                        decision: present_decision,
                    };
                };
                let VisualLayerKind::Scene(root_scene) = &root_layer.kind else {
                    unreachable!("root_layer always returns a scene layer");
                };
                let frame =
                    PreparedFrame::new(frame_w, frame_h, scale, background, root_scene, &overlays);
                surface.render_frame(&mut self.renderer, frame)
            }
        };

        self.window_surface = Some(surface);

        phases.surface_acquire = surface_timings.surface_acquire;
        phases.encode_base = surface_timings.encode;
        phases.encode_anim = surface_timings.encode_anim;
        phases.composite = surface_timings.composite;
        phases.present_submit = surface_timings.present_submit;

        let painted = matches!(render_result, RenderFrameResult::Presented);
        if painted {
            // Sticky: clear entry/host dirty only after successful present.
            self.layer_registry.clear_dirty_after_successful_present();
            // Ack Spinner / ProgressBar phases only after present succeeds (Issue 13).
            // Selective path never paints; Failed must leave phase unacked so
            // request_paint_only continues until encode/present can retry.
            self.layer_registry
                .ack_anim_phases_after_present(&self.render_root);
            // Base invalidation clears only after a full-path present (Issue 10).
            if used_full_base_path {
                self.base_invalidated = false;
            }
        } else {
            self.layer_registry.retain_dirty_after_failed_present();
        }

        let anim_driven_present = !had_unthrottled
            && (entry.do_anim_tick
                || post_dirty
                    .iter()
                    .any(|r| matches!(r, DirtyReason::AnimPaint { .. })));
        let redraw_demand =
            self.finish_present_attempt(render_result, snap, anim_driven_present, now);
        self.drain_redraw_signals();

        let decision = FrameDecision {
            do_anim_tick: entry.do_anim_tick,
            // Phase 1: rewrite+encode+present remain coupled when we enter this path.
            do_rewrite: present_decision.do_rewrite,
            do_encode: present_decision.do_encode,
            do_present: painted,
            anim_tick_only: false,
            enter_work: true,
            throttled_anim_present: false,
        };

        FrameStepResult {
            painted,
            redraw_demand,
            anim_tick_only: false,
            paint_reasons,
            phases,
            decision,
        }
    }

    pub(crate) fn take_pending_ime_signals(&mut self) -> Vec<ImeWindowSignal> {
        self.ime_signal_receiver.try_iter().collect()
    }

    fn sync_window_metrics(&mut self, metrics: ExistingWindowMetrics) {
        let next_scale = metrics.scale_factor.max(f64::EPSILON);
        let next_viewport_width = metrics.logical_width.max(1.0);
        let next_viewport_height = metrics.logical_height.max(1.0);
        let transparency_changed = self.window_transparent != metrics.transparent;
        let needs_rescale = (self.window_scale_factor - next_scale).abs() > f64::EPSILON;
        let needs_resize = (self.viewport_width - next_viewport_width).abs() > f64::EPSILON
            || (self.viewport_height - next_viewport_height).abs() > f64::EPSILON;

        self.window_scale_factor = next_scale;
        self.window_transparent = metrics.transparent;
        self.viewport_width = next_viewport_width;
        self.viewport_height = next_viewport_height;

        if needs_rescale {
            let _ = self
                .render_root
                .handle_window_event(WindowEvent::Rescale(self.window_scale_factor));
        }

        if needs_resize || needs_rescale {
            let _ = self
                .render_root
                .handle_window_event(WindowEvent::Resize(PhysicalSize::new(
                    metrics.physical_width.max(1),
                    metrics.physical_height.max(1),
                )));
            self.needs_redraw = true;
            self.resize_dirty = true;
            // P2.6: atomic metrics generation — drop old-size textures on next paint.
            self.layer_registry.notify_metrics_changed(
                metrics.physical_width.max(1),
                metrics.physical_height.max(1),
            );
        }

        if transparency_changed {
            self.needs_redraw = true;
        }
    }

    /// Register a batch of font bytes into this window's retained font database.
    pub fn register_fonts(&mut self, font_bytes: Vec<u8>) -> bool {
        if font_bytes.is_empty() {
            return false;
        }

        let fingerprint = font_bytes_fingerprint(&font_bytes);
        if !self.registered_font_fingerprints.insert(fingerprint) {
            return false;
        }

        self.render_root
            .register_fonts(crate::masonry_core::peniko::Blob::new(Arc::new(font_bytes)));
        self.theme_or_font_dirty = true;
        self.needs_redraw = true;
        true
    }
}

/// True when coalesced visual runs align with the compositor plan length (Issue 10).
///
/// Empty visual plan may still produce a single fallback CachedScene entry.
#[inline]
fn visual_runs_match_plan_len(runs_len: usize, plan_len: usize) -> bool {
    (runs_len == 0 && plan_len <= 1) || runs_len == plan_len
}

/// Encode dirty painter-order entries and composite (multi-entry path, P2.3–P2.4 + P2c).
///
/// `visual_layers` is `None` on the pure-anim path (G2): only Anim entries may
/// need encode, filled from host scenes; base textures are reused.
fn encode_ordered_composite(
    registry: &LayerRegistry,
    renderer: &mut Renderer,
    surface: &mut ExternalWindowSurface,
    visual_layers: Option<&crate::masonry_core::app::VisualLayerPlan>,
    background: Color,
    frame_w: u32,
    frame_h: u32,
    scale: f64,
) -> (RenderFrameResult, picus_surface::RenderFrameTimings) {
    use crate::masonry_core::imaging::record::Scene;

    // Same run coalescing as LayerRegistry::rebuild_from_visual_plan (Issue 3).
    let runs = visual_layers
        .map(coalesce_visual_runs)
        .unwrap_or_default();
    if let Some(visual) = visual_layers {
        debug_assert!(
            visual_runs_match_plan_len(runs.len(), registry.plan().len()),
            "compositor plan / visual runs length mismatch: runs={} plan={}",
            runs.len(),
            registry.plan().len()
        );
        let _ = visual;
    }

    #[derive(Clone, Copy)]
    struct EntryMeta {
        id: u64,
        kind: CompositorEntryKind,
        needs_encode: bool,
    }
    let metas: Vec<EntryMeta> = registry
        .plan()
        .entries()
        .iter()
        .map(|e| EntryMeta {
            id: e.id.raw(),
            kind: e.kind,
            needs_encode: e.needs_encode(),
        })
        .collect();

    let empty_scene = Scene::new();
    let mut overlay_storage: Vec<Vec<ImagingLayer<'_>>> = Vec::with_capacity(metas.len());
    let mut prepared: Vec<Option<PreparedFrame<'_>>> = Vec::with_capacity(metas.len());

    // Host anim scenes (window space) — stable refs for PreparedFrame borrows.
    let anim_scenes: Vec<Option<&Scene>> = registry
        .plan()
        .entries()
        .iter()
        .map(|e| {
            e.anim_id
                .and_then(|id| registry.host().get(id))
                .map(|he| &he.scene as &Scene)
        })
        .collect();

    for (i, meta) in metas.iter().enumerate() {
        if !meta.needs_encode {
            overlay_storage.push(Vec::new());
            prepared.push(None);
            continue;
        }
        match meta.kind {
            CompositorEntryKind::Anim | CompositorEntryKind::External => {
                overlay_storage.push(Vec::new());
                let scene = anim_scenes[i].unwrap_or(&empty_scene);
                prepared.push(Some(PreparedFrame::new(
                    frame_w,
                    frame_h,
                    scale,
                    Color::TRANSPARENT,
                    scene,
                    &[],
                )));
            }
            CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay => {
                let mut overlays = Vec::new();
                if let Some(visual) = visual_layers
                    && let Some(VisualRun::Scenes(indices)) = runs.get(i)
                {
                    for &idx in indices.iter().skip(1) {
                        let layer = &visual.layers[idx];
                        if let VisualLayerKind::Scene(scene) = &layer.kind {
                            overlays.push(ImagingLayer {
                                scene,
                                transform: layer.transform,
                            });
                        }
                    }
                }
                overlay_storage.push(overlays);
                prepared.push(None);
            }
        }
    }

    // Second pass: CachedScene / Overlay base scenes from visual plan.
    for (i, meta) in metas.iter().enumerate() {
        if !meta.needs_encode
            || !matches!(
                meta.kind,
                CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay
            )
        {
            continue;
        }
        if prepared[i].is_some() {
            continue;
        }
        let bg = if i == 0 {
            background
        } else {
            Color::TRANSPARENT
        };
        let Some(visual) = visual_layers else {
            // Selective path must never encode CachedScene without a visual plan
            // (would wipe base with empty content in release). Caller should fall
            // back to full redraw; fail closed here.
            tracing::error!(
                "CachedScene/Overlay needs_encode without visual plan; refusing empty base encode"
            );
            return (RenderFrameResult::Failed, picus_surface::RenderFrameTimings::default());
        };
        match runs.get(i) {
            Some(VisualRun::Scenes(indices)) if !indices.is_empty() => {
                let base_layer = &visual.layers[indices[0]];
                let VisualLayerKind::Scene(base_scene) = &base_layer.kind else {
                    prepared[i] = Some(PreparedFrame::new(
                        frame_w,
                        frame_h,
                        scale,
                        bg,
                        &empty_scene,
                        &[],
                    ));
                    continue;
                };
                prepared[i] = Some(PreparedFrame::new(
                    frame_w,
                    frame_h,
                    scale,
                    bg,
                    base_scene,
                    &overlay_storage[i],
                ));
            }
            _ => {
                prepared[i] = Some(PreparedFrame::new(
                    frame_w,
                    frame_h,
                    scale,
                    bg,
                    &empty_scene,
                    &[],
                ));
            }
        }
    }

    let ordered: Vec<OrderedLayerEncode<'_>> = metas
        .iter()
        .zip(prepared.iter())
        .map(|(meta, frame)| OrderedLayerEncode {
            layer_id: meta.id,
            kind: match meta.kind {
                CompositorEntryKind::Anim => OrderedEntryKind::Anim,
                CompositorEntryKind::External => OrderedEntryKind::External,
                CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay => {
                    OrderedEntryKind::Cached
                }
            },
            frame: *frame,
        })
        .collect();

    surface.render_ordered_frame(renderer, &ordered)
}

fn focus_fallback_widget(render_root: &RenderRoot) -> Option<WidgetId> {
    render_root
        .get_layer_root(0)
        .downcast::<Passthrough>()
        .map(|root| root.inner().inner_id())
}

fn existing_window_metrics(
    window: &XilemWinitWindow,
    transparent: bool,
    composite_alpha_mode: CompositeAlphaMode,
) -> ExistingWindowMetrics {
    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);

    ExistingWindowMetrics {
        physical_width: physical_size.width,
        physical_height: physical_size.height,
        logical_width: logical_size.width,
        logical_height: logical_size.height,
        scale_factor,
        transparent,
        composite_alpha_mode,
    }
}

fn build_widget_path(
    widget: WidgetRef<'_, dyn Widget>,
    target: WidgetId,
    path: &mut Vec<WidgetId>,
) -> bool {
    if widget.ctx().is_stashed() {
        return false;
    }

    path.push(widget.id());
    if widget.id() == target {
        return true;
    }

    for child in widget.children() {
        if build_widget_path(child, target, path) {
            return true;
        }
    }

    path.pop();
    false
}

fn parse_entity_debug_binding(debug: &str) -> Option<(u64, bool)> {
    if let Some(bits) = debug.strip_prefix("opaque_hitbox_entity=") {
        return Some((bits.parse::<u64>().ok()?, true));
    }

    if let Some(bits) = debug.strip_prefix("entity_scope=") {
        return Some((bits.parse::<u64>().ok()?, false));
    }

    if let Some(bits) = debug.strip_prefix("entity=") {
        return Some((bits.parse::<u64>().ok()?, false));
    }

    None
}

fn picus_default_properties() -> DefaultProperties {
    let mut properties = DefaultProperties::new();
    let transparent = Color::TRANSPARENT;

    properties.insert::<WidgetLabel, _>(ContentColor::new(transparent));
    properties.insert::<TextArea<true>, _>(ContentColor::new(transparent));
    properties.insert::<TextArea<false>, _>(ContentColor::new(transparent));
    properties.insert::<WidgetTextInput, _>(PlaceholderColor::new(transparent));
    properties.insert::<Divider, _>(ContentColor::new(transparent));
    properties.insert::<Spinner, _>(ContentColor::new(transparent));

    properties
}

/// Headless Masonry runtime owned by Bevy, keyed by window entity.
///
/// This runtime keeps ownership of one [`WindowRuntime`] per attached Bevy
/// window and drives each via explicit Bevy-system input injection +
/// synthesis-time per-window rebuilds.
///
/// Use [`Self::ensure_window`] to create a runtime for a window entity,
/// [`Self::window`] / [`Self::window_mut`] to access a specific window, and
/// [`Self::primary`] / [`Self::primary_mut`] to access the primary window's
/// runtime (the window marked with Bevy's [`PrimaryWindow`] component, or the
/// first attached window when no primary is present).
pub struct MasonryRuntime {
    pub windows: HashMap<Entity, WindowRuntime>,
    primary_window: Option<Entity>,
    event_loop_proxy: Arc<Mutex<Option<EventLoopProxy<WinitUserEvent>>>>,
    /// Shared write sink for every window attached to this app.
    action_sink: InternalUiActionSink,
}

impl FromWorld for MasonryRuntime {
    fn from_world(world: &mut World) -> Self {
        world.init_resource::<InternalUiEventQueue>();
        let action_sink = world.resource::<InternalUiEventQueue>().sink();
        install_app_ui_action_sink(action_sink.clone());
        // Compatibility install for call sites still holding a raw SegQueue handle.
        install_global_ui_event_queue(action_sink.shared_queue());

        Self {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink,
        }
    }
}

impl MasonryRuntime {
    /// Returns the entity of the primary window, if any.
    ///
    /// The primary window is the one marked with Bevy's [`PrimaryWindow`]
    /// component when it was attached, or the first attached window otherwise.
    #[must_use]
    pub fn primary_window(&self) -> Option<Entity> {
        self.primary_window
            .or_else(|| self.windows.keys().next().copied())
    }

    /// Borrow the primary window's runtime.
    #[must_use]
    pub fn primary(&self) -> Option<&WindowRuntime> {
        self.primary_window()
            .and_then(|entity| self.windows.get(&entity))
    }

    /// Mutably borrow the primary window's runtime.
    #[must_use]
    pub fn primary_mut(&mut self) -> Option<&mut WindowRuntime> {
        self.primary_window()
            .and_then(|entity| self.windows.get_mut(&entity))
    }

    /// Borrow a specific window's runtime.
    #[must_use]
    pub fn window(&self, entity: Entity) -> Option<&WindowRuntime> {
        self.windows.get(&entity)
    }

    /// Mutably borrow a specific window's runtime.
    #[must_use]
    pub fn window_mut(&mut self, entity: Entity) -> Option<&mut WindowRuntime> {
        self.windows.get_mut(&entity)
    }

    /// Install this app's action sink as the active thread-local target.
    ///
    /// Call before retained input/rebuild work so multi-app hosts cannot leak
    /// emissions across Bevy `App` instances on the same thread.
    pub(crate) fn install_action_sink(&self) {
        install_app_ui_action_sink(self.action_sink.clone());
    }

    /// Create a window runtime for `entity` if one does not already exist.
    ///
    /// If `is_primary` is `true`, the entity is recorded as the primary window.
    /// Returns the new (or existing) window runtime.
    pub fn ensure_window(&mut self, entity: Entity, is_primary: bool) -> &mut WindowRuntime {
        if is_primary && self.primary_window.is_none() {
            self.primary_window = Some(entity);
        }

        let action_sink = self.action_sink.clone();
        let event_loop_proxy = Arc::clone(&self.event_loop_proxy);
        self.windows.entry(entity).or_insert_with(|| {
            WindowRuntime::new(
                entity,
                WindowRuntime::initial_placeholder_view(),
                event_loop_proxy,
                action_sink,
            )
        })
    }

    /// Update the Bevy winit event-loop proxy used by async Xilem view tasks.
    pub fn set_event_loop_proxy(&mut self, proxy: Option<EventLoopProxy<WinitUserEvent>>) {
        if let Ok(mut target) = self.event_loop_proxy.lock() {
            *target = proxy;
        }
    }

    /// Mark `entity` as the primary window if no primary is set yet.
    pub fn set_primary_if_unset(&mut self, entity: Entity) {
        if self.primary_window.is_none() {
            self.primary_window = Some(entity);
        }
    }

    /// Remove a window's runtime when the window is destroyed.
    pub fn remove_window(&mut self, entity: Entity) {
        self.windows.remove(&entity);
        if self.primary_window == Some(entity) {
            self.primary_window = None;
        }
    }

    /// Iterate over all attached window entities.
    pub fn window_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.windows.keys().copied()
    }

    /// Returns `true` if a runtime exists for `entity`.
    #[must_use]
    pub fn has_window(&self, entity: Entity) -> bool {
        self.windows.contains_key(&entity)
    }

    /// Register font bytes into every attached window's retained font database.
    pub fn register_fonts_all(&mut self, font_bytes: Vec<u8>) {
        for window in self.windows.values_mut() {
            window.register_fonts(font_bytes.clone());
        }
    }
}

pub fn compose_runtime_root(roots: &[UiView]) -> UiView {
    fn viewport_child(root: UiView) -> UiView {
        Arc::new(sized_box(root).dims(Dimensions::STRETCH))
    }

    match roots {
        [] => Arc::new(label("picus_core: no synthesized root")),
        [root] => root.clone(),
        _ => Arc::new(
            zstack(
                roots
                    .iter()
                    .enumerate()
                    .map(|(index, root)| {
                        if index + 1 == roots.len() {
                            root.clone()
                        } else {
                            viewport_child(root.clone())
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .alignment(UnitPoint::TOP_LEFT)
            .dims(Dimensions::STRETCH),
        ),
    }
}

/// Sync pending IME signals from every window runtime into the corresponding
/// Bevy window's `ime_enabled` / `ime_position` state.
pub fn sync_masonry_ime_state_to_bevy_window(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    primary_window_query: Query<Entity, With<PrimaryWindow>>,
    mut window_query: Query<&mut Window>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let window_entities: Vec<Entity> = runtime.window_entities().collect();
    let primary_entity = primary_window_query.iter().next();

    for window_entity in window_entities {
        let Some(window_runtime) = runtime.window_mut(window_entity) else {
            continue;
        };
        let pending = window_runtime.take_pending_ime_signals();
        if pending.is_empty() {
            continue;
        }

        let target_window = if runtime.has_window(window_entity) {
            Some(window_entity)
        } else {
            primary_entity
        };
        let Some(target_window) = target_window else {
            continue;
        };

        let Ok(mut window) = window_query.get_mut(target_window) else {
            continue;
        };

        for signal in pending {
            match signal {
                ImeWindowSignal::Start => {
                    if !window.ime_enabled {
                        window.ime_enabled = true;
                    }
                }
                ImeWindowSignal::End => {
                    if window.ime_enabled {
                        window.ime_enabled = false;
                    }
                }
                ImeWindowSignal::Move(position) => {
                    if window.ime_position != position {
                        window.ime_position = position;
                    }
                }
            }
        }
    }
}

fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Auxiliary),
        MouseButton::Back => Some(PointerButton::X1),
        MouseButton::Forward => Some(PointerButton::X2),
        MouseButton::Other(_) => None,
    }
}

fn map_button_state_to_key_state(state: ButtonState) -> KeyState {
    match state {
        ButtonState::Pressed => KeyState::Down,
        ButtonState::Released => KeyState::Up,
    }
}

fn map_named_key_from_key_code(key_code: KeyCode) -> Option<NamedKey> {
    match key_code {
        KeyCode::Backspace => Some(NamedKey::Backspace),
        KeyCode::Delete => Some(NamedKey::Delete),
        KeyCode::Tab => Some(NamedKey::Tab),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(NamedKey::Enter),
        KeyCode::Escape => Some(NamedKey::Escape),
        KeyCode::ArrowLeft => Some(NamedKey::ArrowLeft),
        KeyCode::ArrowRight => Some(NamedKey::ArrowRight),
        KeyCode::ArrowUp => Some(NamedKey::ArrowUp),
        KeyCode::ArrowDown => Some(NamedKey::ArrowDown),
        KeyCode::Home => Some(NamedKey::Home),
        KeyCode::End => Some(NamedKey::End),
        KeyCode::PageUp => Some(NamedKey::PageUp),
        KeyCode::PageDown => Some(NamedKey::PageDown),
        _ => None,
    }
}

fn map_text_key_from_logical_key(key: &BevyKey) -> Option<Key> {
    match key {
        BevyKey::Character(text) => Some(Key::Character(text.clone().into())),
        BevyKey::Space => Some(Key::Character(" ".into())),
        _ => None,
    }
}

fn modifier_from_logical_key(key: &BevyKey) -> Option<Modifiers> {
    match key {
        BevyKey::Alt | BevyKey::AltGraph => Some(Modifiers::ALT),
        BevyKey::Control => Some(Modifiers::CONTROL),
        BevyKey::Shift => Some(Modifiers::SHIFT),
        BevyKey::Meta | BevyKey::Super => Some(Modifiers::META),
        _ => None,
    }
}

fn update_modifiers_from_logical_key(modifiers: &mut Modifiers, key: &BevyKey, state: ButtonState) {
    let Some(next) = modifier_from_logical_key(key) else {
        return;
    };

    match state {
        ButtonState::Pressed => modifiers.insert(next),
        ButtonState::Released => modifiers.remove(next),
    }
}

/// PreUpdate input bridge: consume Bevy window/input messages and inject them
/// into the matching per-window Masonry runtime.
///
/// Events are routed to the window runtime identified by their `window` field.
/// Events for windows without an attached runtime are ignored.
#[expect(
    clippy::too_many_arguments,
    reason = "Bevy system functions naturally take multiple queries and readers"
)]
pub fn inject_bevy_input_into_masonry(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    mut overlay_routing: ResMut<OverlayPointerRoutingState>,
    mut frame_timing: ResMut<crate::perf::FrameTiming>,
    window_query: Query<&Window>,
    primary_window_entity_query: Query<Entity, With<PrimaryWindow>>,
    mut keyboard_input: MessageReader<KeyboardInput>,
    mut ime_events: MessageReader<BevyIme>,
    mut window_focused: MessageReader<WindowFocused>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut cursor_left: MessageReader<CursorLeft>,
    mut mouse_button_input: MessageReader<MouseButtonInput>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut window_resized: MessageReader<WindowResized>,
    mut window_scale_factor_changed: MessageReader<WindowScaleFactorChanged>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };
    let input_phase = crate::perf::PhaseTimer::start();
    runtime.install_action_sink();

    let primary_window_entity = primary_window_entity_query.iter().next();

    for event in cursor_moved.read() {
        let target = event.window;
        let Some(window) = window_query.get(target).ok() else {
            continue;
        };
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };

        let Some(pointer_position) = window.physical_cursor_position() else {
            continue;
        };

        window_runtime.handle_cursor_moved(pointer_position.x, pointer_position.y);
        tracing::trace!(
            "Input Injection - Bevy Physical Cursor Moved: ({}, {}). Injected into Masonry window {:?}.",
            pointer_position.x,
            pointer_position.y,
            target
        );
    }

    for event in cursor_left.read() {
        let target = event.window;
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };
        window_runtime.handle_cursor_left();
    }

    for event in window_focused.read() {
        let target = event.window;
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };
        window_runtime.handle_text_event(TextEvent::WindowFocusChange(event.focused));
    }

    for event in ime_events.read() {
        let (window, text_event) = match event {
            BevyIme::Preedit {
                window,
                value,
                cursor,
            } => (
                *window,
                TextEvent::Ime(crate::masonry_core::core::Ime::Preedit(
                    value.clone(),
                    *cursor,
                )),
            ),
            BevyIme::Commit { window, value } => (
                *window,
                TextEvent::Ime(crate::masonry_core::core::Ime::Commit(value.clone())),
            ),
            BevyIme::Enabled { window } => (
                *window,
                TextEvent::Ime(crate::masonry_core::core::Ime::Enabled),
            ),
            BevyIme::Disabled { window } => (
                *window,
                TextEvent::Ime(crate::masonry_core::core::Ime::Disabled),
            ),
        };

        let Some(window_runtime) = runtime.window_mut(window) else {
            continue;
        };
        window_runtime.handle_text_event(text_event);
    }

    for event in keyboard_input.read() {
        let target = event.window;
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };

        update_modifiers_from_logical_key(
            &mut window_runtime.keyboard_modifiers,
            &event.logical_key,
            event.state,
        );

        if let Some(key) = map_named_key_from_key_code(event.key_code)
            .map(Key::Named)
            .or_else(|| map_text_key_from_logical_key(&event.logical_key))
        {
            let keyboard_modifiers = window_runtime.keyboard_modifiers;
            window_runtime.handle_text_event(TextEvent::Keyboard(
                crate::masonry_core::core::KeyboardEvent {
                    state: map_button_state_to_key_state(event.state),
                    key,
                    repeat: event.repeat,
                    modifiers: keyboard_modifiers,
                    ..Default::default()
                },
            ));
            continue;
        }

        if event.state == ButtonState::Pressed
            && let Some(text) = event.text.as_ref()
            && !text.is_empty()
        {
            window_runtime.handle_text_event(TextEvent::Ime(
                crate::masonry_core::core::Ime::Commit(text.to_string()),
            ));
        }
    }

    for event in mouse_button_input.read() {
        let target = event.window;
        let Some(window) = window_query.get(target).ok() else {
            continue;
        };

        let suppressed = match event.state {
            ButtonState::Pressed => overlay_routing.take_suppressed_press(target, event.button),
            ButtonState::Released => overlay_routing.take_suppressed_release(target, event.button),
        };

        if suppressed {
            continue;
        }

        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };

        let Some(pointer_position) = window.physical_cursor_position() else {
            tracing::debug!(
                "skipping mouse button input because cursor is outside window {:?}",
                target
            );
            continue;
        };

        window_runtime.handle_cursor_moved(pointer_position.x, pointer_position.y);
        window_runtime.handle_mouse_button(event.button, event.state);
        tracing::trace!(
            "Input Injection - Mouse Button: {:?} {:?} at Physical ({}, {}) window {:?}",
            event.button,
            event.state,
            pointer_position.x,
            pointer_position.y,
            target
        );
    }

    for event in mouse_wheel.read() {
        let target = event.window;
        let Some(window) = window_query.get(target).ok() else {
            continue;
        };
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };

        let Some(pointer_position) = window.physical_cursor_position() else {
            tracing::debug!(
                "skipping mouse wheel input because cursor is outside window {:?}",
                target
            );
            continue;
        };

        window_runtime.handle_cursor_moved(pointer_position.x, pointer_position.y);
        window_runtime.handle_mouse_wheel(event.unit, event.x, event.y);
        tracing::trace!(
            "Input Injection - Mouse Wheel: {:?} ({}, {}) at Physical cursor ({}, {}) window {:?}",
            event.unit,
            event.x,
            event.y,
            pointer_position.x,
            pointer_position.y,
            target
        );
    }

    for event in window_resized.read() {
        let target = event.window;
        let Some(window) = window_query.get(target).ok() else {
            continue;
        };
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };
        window_runtime.handle_window_resized(window.width(), window.height());
        tracing::trace!(
            "Window Resize - Bevy Logical Size: {}x{}, window {:?}.",
            window.width(),
            window.height(),
            target
        );
    }

    for event in window_scale_factor_changed.read() {
        let target = event.window;
        let Some(window) = window_query.get(target).ok() else {
            continue;
        };
        let Some(window_runtime) = runtime.window_mut(target) else {
            continue;
        };
        window_runtime.handle_window_scale_factor_changed(window.scale_factor() as f64);
        tracing::trace!(
            "Window Scale Factor - Bevy Scale: {}, window {:?}.",
            window.scale_factor(),
            target
        );
    }

    let _ = primary_window_entity;
    frame_timing.record_input_dispatch(input_phase.elapsed());
}

/// Attach a Masonry window runtime to each Bevy window once it appears.
///
/// The primary window (marked with [`PrimaryWindow`]) is auto-attached. Other
/// windows are auto-attached as secondary windows.
pub fn initialize_masonry_runtime_from_windows(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    bridge: Option<Res<XilemFontBridge>>,
    event_loop_proxy: Option<Res<EventLoopProxyWrapper>>,
    added_window_query: Query<(Entity, &Window, Option<&PrimaryWindow>), Added<Window>>,
    window_query: Query<(Entity, &Window, Option<&PrimaryWindow>), With<Window>>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    runtime.set_event_loop_proxy(event_loop_proxy.as_deref().map(|proxy| (**proxy).clone()));

    // Gather candidate windows: newly-added windows, or any existing window if
    // the runtime currently has none attached.
    let candidates: Vec<(Entity, bool, bool, CompositeAlphaMode)> = if runtime.windows.is_empty() {
        window_query
            .iter()
            .map(|(entity, window, primary)| {
                (
                    entity,
                    primary.is_some(),
                    window.transparent,
                    window.composite_alpha_mode,
                )
            })
            .collect()
    } else {
        added_window_query
            .iter()
            .map(|(entity, window, primary)| {
                (
                    entity,
                    primary.is_some(),
                    window.transparent,
                    window.composite_alpha_mode,
                )
            })
            .collect()
    };

    for (window_entity, is_primary, transparent, composite_alpha_mode) in candidates {
        if runtime.has_window(window_entity) {
            if is_primary {
                runtime.set_primary_if_unset(window_entity);
            }
            continue;
        }

        let metrics = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
            let winit_windows = winit_windows.borrow();
            winit_windows
                .get_window(window_entity)
                .map(|window| existing_window_metrics(window, transparent, composite_alpha_mode))
        });

        let window_runtime = runtime.ensure_window(window_entity, is_primary);
        if let Some(bridge) = bridge.as_deref() {
            for font_bytes in bridge.registered_font_bytes() {
                window_runtime.register_fonts(font_bytes.to_vec());
            }
        }

        if let Some(metrics) = metrics {
            window_runtime.attach_to_window(metrics);

            tracing::trace!(
                "Runtime Init - Window {:?} ({}) Logic Size: {}x{}, Scale: {}",
                window_entity,
                if is_primary { "primary" } else { "secondary" },
                metrics.logical_width,
                metrics.logical_height,
                metrics.scale_factor
            );

            // Prime Masonry's layout root with an explicit initial logical resize so hit-testing
            // never starts from a zero-sized root, even before the first window-resize message.
            window_runtime
                .handle_window_resized(metrics.logical_width as f32, metrics.logical_height as f32);
        } else {
            // No winit handle available (e.g. headless tests). Still create the
            // runtime so synthesis/rebuild/hit-testing work without a real window.
            let fallback = ExistingWindowMetrics {
                physical_width: 1024,
                physical_height: 768,
                logical_width: 1024.0,
                logical_height: 768.0,
                scale_factor: 1.0,
                transparent,
                composite_alpha_mode,
            };
            window_runtime.attach_to_window(fallback);
            window_runtime.handle_window_resized(1024.0, 768.0);
            tracing::trace!(
                "Runtime Init - Window {:?} ({}) created without winit handle (fallback 1024x768)",
                window_entity,
                if is_primary { "primary" } else { "secondary" }
            );
        }
    }
}

/// Drop retained UI and surface state for Bevy windows that are already closing
/// or whose [`Window`] component has been removed.
pub fn sync_masonry_window_lifecycle(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    window_query: Query<(), With<Window>>,
    closing_window_query: Query<(), With<ClosingWindow>>,
    mut synthesized: ResMut<SynthesizedUiViews>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let stale_windows = runtime
        .window_entities()
        .filter(|entity| !window_query.contains(*entity) || closing_window_query.contains(*entity))
        .collect::<Vec<_>>();

    for window_entity in stale_windows {
        runtime.remove_window(window_entity);
        synthesized.remove_window(window_entity);
    }
}

/// PostUpdate rebuild step: diff each window's synthesized root against its
/// retained Masonry tree.
pub fn rebuild_masonry_runtime(world: &mut World) {
    if !world.contains_non_send::<MasonryRuntime>() {
        return;
    }

    let phase = crate::perf::PhaseTimer::start();
    let _span =
        tracing::trace_span!(target: "picus_core::perf", "rebuild_masonry_runtime").entered();

    let window_views: Vec<(Entity, UiView)> = world
        .get_resource_mut::<SynthesizedUiViews>()
        .map(|mut views| {
            let dirty_windows = views.dirty_windows.drain().collect::<Vec<_>>();
            dirty_windows
                .into_iter()
                .filter_map(|window| {
                    views
                        .windows
                        .get(&window)
                        .cloned()
                        .map(|view| (window, view))
                })
                .collect()
        })
        .unwrap_or_default();

    if window_views.is_empty() {
        if let Some(mut timing) = world.get_resource_mut::<crate::perf::FrameTiming>() {
            timing.record_rebuild(phase.elapsed());
        }
        return;
    }

    let mut runtime = world.non_send_mut::<MasonryRuntime>();

    for (window_entity, window_view) in window_views {
        let Some(window_runtime) = runtime.window_mut(window_entity) else {
            continue;
        };
        window_runtime.install_action_sink();
        window_runtime.rebuild_root_view(window_view);
    }

    drop(runtime);
    if let Some(mut timing) = world.get_resource_mut::<crate::perf::FrameTiming>() {
        timing.record_rebuild(phase.elapsed());
    }
}

/// PreUpdate step: route widget actions emitted during input injection and
/// async Xilem proxy messages back to their source view's `message` handler.
/// Callback-based views (such as `text_input`) fire their `on_changed` /
/// `on_enter` callbacks, and task views can deliver background output into the
/// global UI event queue before the ECS action-drain systems run.
pub fn route_masonry_view_messages(runtime: Option<NonSendMut<MasonryRuntime>>) {
    let Some(mut runtime) = runtime else {
        return;
    };
    let entities: Vec<Entity> = runtime.window_entities().collect();
    for entity in entities {
        if let Some(window_runtime) = runtime.window_mut(entity) {
            window_runtime.install_action_sink();
            window_runtime.route_pending_view_messages();
        }
    }
}

/// Last-stage paint pass: submit each window's Masonry scene through Vello and
/// present to the corresponding Bevy window.
///
/// # Bevy wake (Phase 1b)
///
/// Each window returns a structured anim-tick vs content-present demand
/// (internal). Demands are OR-merged across windows; a single
/// [`RequestRedraw`] is written when either flag is set. Bevy's reactive
/// `WinitSettings` still runs a full schedule on that wake — see
/// `docs/architecture/runtime.md` (redraw semantics).
pub fn paint_masonry_ui(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    active_window_query: Query<&Window, Without<ClosingWindow>>,
    time: Res<Time>,
    mut redraw_requests: MessageWriter<RequestRedraw>,
    mut frame_timing: ResMut<crate::perf::FrameTiming>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let _span = tracing::trace_span!(target: "picus_core::perf", "paint_masonry_ui").entered();

    let window_entities: Vec<Entity> = runtime.window_entities().collect();
    let mut redraw_demand = RedrawDemand::none();

    for window_entity in window_entities {
        let Ok(bevy_window) = active_window_query.get(window_entity) else {
            runtime.remove_window(window_entity);
            continue;
        };
        let transparent = bevy_window.transparent;
        let composite_alpha_mode = bevy_window.composite_alpha_mode;

        let Some(metrics) = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
            let winit_windows = winit_windows.borrow();
            winit_windows
                .get_window(window_entity)
                .map(|window| existing_window_metrics(window, transparent, composite_alpha_mode))
        }) else {
            continue;
        };

        let has_surface = {
            let Some(window_runtime) = runtime.window_mut(window_entity) else {
                continue;
            };
            window_runtime.attach_to_window(metrics);
            bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
                let winit_windows = winit_windows.borrow();
                let Some(window) = winit_windows.get_window(window_entity) else {
                    return false;
                };
                window_runtime.ensure_external_surface(window, metrics)
            })
        };

        if !has_surface {
            continue;
        }

        let Some(window_runtime) = runtime.window_mut(window_entity) else {
            continue;
        };
        // FrameDriver decision+execution spine (Phase 1).
        let result = window_runtime.step_frame(time.delta());
        redraw_demand.merge(result.redraw_demand);
        // Skip pure idle (Skipped reason with no work) from frame_id accounting.
        let entered_work = result.paint_reasons & crate::perf::PaintReason::Skipped as u32 == 0;
        if entered_work {
            frame_timing.record_window_paint(
                window_entity,
                result.phases,
                result.painted,
                result.anim_tick_only,
                result.paint_reasons,
            );
        }
        if result.painted {
            tracing::trace!("painted Masonry frame for window {:?}", window_entity);
        }
    }

    // Write RequestRedraw only when ContentPresent or AnimTick scheduling needs it.
    if redraw_demand.any() {
        tracing::trace!(
            target: "picus_core::perf",
            need_anim_tick = redraw_demand.need_anim_tick,
            need_content_present = redraw_demand.need_content_present,
            "paint_masonry_ui RequestRedraw"
        );
        redraw_requests.write(RequestRedraw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use crate::{
        AdvancedAppPicusExt, AppPicusExt, InteractionState, NavigationViewItem, PicusPlugin,
        ProjectionCtx, UiAction, UiComponentTemplate, UiNavigationItem, UiNavigationView,
        UiProjectorRegistry, UiRoot, UiView, emit_ui_action,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_ecs::message::MessageReader;
    use bevy_ecs::prelude::{Component, Resource};
    use bevy_input::touch::TouchPhase;
    use picus_view::picus_widget::widgets::TextAction;
    use picus_view::{core::fork, view::task};

    #[derive(Resource, Default)]
    struct ProjectionTestResource {
        text: String,
    }

    #[derive(Component, Default, Clone)]
    struct ResourceBackedLabel;

    impl UiComponentTemplate for ResourceBackedLabel {
        fn project(_component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
            Arc::new(label(
                ctx.world.resource::<ProjectionTestResource>().text.clone(),
            ))
        }

        fn register_projection_dependencies(registry: &mut UiProjectorRegistry) {
            registry.register_resource_dependency::<ProjectionTestResource>();
        }
    }

    #[derive(Component, Default, Clone)]
    struct ProxyTaskRoot;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ProxyTaskAction;

    impl UiComponentTemplate for ProxyTaskRoot {
        fn project(_component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
            let root = ctx.entity;
            let heartbeat = task(
                |proxy, _: &mut ()| async move {
                    let _ = proxy.message(());
                },
                move |_: &mut (), ()| {
                    emit_ui_action(root, ProxyTaskAction);
                },
            );

            Arc::new(fork(Arc::new(label("task")) as UiView, Some(heartbeat)))
        }
    }

    #[test]
    fn logical_character_keys_map_to_text_keys() {
        assert!(matches!(
            map_text_key_from_logical_key(&BevyKey::Character("骨".into())),
            Some(Key::Character(text)) if text.as_str() == "骨"
        ));
        assert!(matches!(
            map_text_key_from_logical_key(&BevyKey::Space),
            Some(Key::Character(text)) if text.as_str() == " "
        ));
    }

    #[test]
    fn modifier_tracking_maps_super_to_meta() {
        let mut modifiers = Modifiers::empty();

        update_modifiers_from_logical_key(&mut modifiers, &BevyKey::Super, ButtonState::Pressed);
        assert!(modifiers.meta());

        update_modifiers_from_logical_key(&mut modifiers, &BevyKey::Super, ButtonState::Released);
        assert!(!modifiers.meta());
    }

    #[test]
    fn multi_window_runtime_isolates_windows() {
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };

        let a = Entity::from_bits(1);
        let b = Entity::from_bits(2);

        let wa = runtime.ensure_window(a, true);
        assert_eq!(wa.window_entity(), a);

        let wb = runtime.ensure_window(b, false);
        assert_eq!(wb.window_entity(), b);

        assert_eq!(runtime.primary_window(), Some(a));
        assert!(runtime.has_window(a));
        assert!(runtime.has_window(b));

        assert_eq!(runtime.windows.len(), 2);

        runtime.remove_window(a);
        assert!(!runtime.has_window(a));
        // `primary_window` field is cleared, but `primary_window()` falls back
        // to the first remaining window when no explicit primary is set.
        assert_eq!(runtime.primary_window(), Some(b));
    }

    #[test]
    fn window_runtime_deduplicates_font_registration() {
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let window = Entity::from_bits(7);

        let window_runtime = runtime.ensure_window(window, true);

        assert!(window_runtime.register_fonts(b"font-data".to_vec()));
        assert!(!window_runtime.register_fonts(b"font-data".to_vec()));
        assert_eq!(window_runtime.registered_font_fingerprints.len(), 1);
    }

    #[test]
    fn window_lifecycle_sync_removes_closing_and_removed_windows() {
        let mut app = App::new();
        app.init_resource::<SynthesizedUiViews>();
        app.init_non_send::<MasonryRuntime>();
        app.add_systems(Update, sync_masonry_window_lifecycle);

        let active = app.world_mut().spawn(Window::default()).id();
        let closing = app
            .world_mut()
            .spawn((Window::default(), ClosingWindow))
            .id();
        let removed = app.world_mut().spawn(Window::default()).id();

        {
            let mut runtime = app.world_mut().non_send_mut::<MasonryRuntime>();
            runtime.ensure_window(active, false);
            runtime.ensure_window(closing, false);
            runtime.ensure_window(removed, false);
        }

        {
            let mut synthesized = app.world_mut().resource_mut::<SynthesizedUiViews>();
            synthesized
                .windows
                .insert(active, Arc::new(label("active")));
            synthesized
                .windows
                .insert(closing, Arc::new(label("closing")));
            synthesized
                .windows
                .insert(removed, Arc::new(label("removed")));
        }

        app.world_mut().entity_mut(removed).despawn();
        app.update();

        let runtime = app.world().non_send::<MasonryRuntime>();
        assert!(runtime.has_window(active));
        assert!(!runtime.has_window(closing));
        assert!(!runtime.has_window(removed));

        let synthesized = app.world().resource::<SynthesizedUiViews>();
        assert!(synthesized.windows.contains_key(&active));
        assert!(!synthesized.windows.contains_key(&closing));
        assert!(!synthesized.windows.contains_key(&removed));
    }

    #[test]
    fn idle_update_does_not_rebuild_retained_tree_again() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));

        app.world_mut()
            .spawn((UiRoot, crate::UiLabel::new("stable")));

        app.update();
        let first_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();

        app.update();
        let second_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();

        assert_eq!(first_rebuild_count, second_rebuild_count);
    }

    #[test]
    fn changed_ui_component_rebuilds_once_then_returns_to_idle() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let label = app
            .world_mut()
            .spawn((UiRoot, crate::UiLabel::new("before")))
            .id();

        app.update();
        let initial_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();

        app.world_mut()
            .get_mut::<crate::UiLabel>(label)
            .expect("label should exist")
            .text = "after".to_string();

        app.update();
        let changed_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(changed_rebuild_count, initial_rebuild_count + 1);

        app.update();
        let idle_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(idle_rebuild_count, changed_rebuild_count);
    }

    #[test]
    fn changed_projection_resource_rebuilds_once_then_returns_to_idle() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .insert_resource(ProjectionTestResource {
                text: "before".to_string(),
            })
            .register_ui_component::<ResourceBackedLabel>();

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));
        app.world_mut().spawn((UiRoot, ResourceBackedLabel));

        app.update();
        let initial_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();

        app.world_mut()
            .resource_mut::<ProjectionTestResource>()
            .text = "after".to_string();

        app.update();
        let changed_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(changed_rebuild_count, initial_rebuild_count + 1);

        app.update();
        let idle_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(idle_rebuild_count, changed_rebuild_count);
    }

    /// Same contract as `#[ui_component(resources(T))]` / macro support:
    /// `register_projection_resource` must dirty synthesis on resource change.
    #[test]
    fn register_projection_resource_path_rebuilds_on_change() {
        #[derive(Component, Default, Clone)]
        struct MacroStyleLabel;

        impl UiComponentTemplate for MacroStyleLabel {
            fn project(_component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
                Arc::new(label(
                    ctx.world.resource::<ProjectionTestResource>().text.clone(),
                ))
            }
            // Intentionally no register_projection_dependencies — resource
            // tracking comes only from AppPicusExt::register_projection_resource.
        }

        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .insert_resource(ProjectionTestResource {
                text: "before".to_string(),
            })
            .register_ui_component::<MacroStyleLabel>()
            .register_projection_resource::<ProjectionTestResource>();

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));
        app.world_mut().spawn((UiRoot, MacroStyleLabel));

        app.update();
        let initial = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary")
            .rebuild_count_for_tests();

        app.world_mut()
            .resource_mut::<ProjectionTestResource>()
            .text = "after".to_string();
        app.update();
        let after = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary")
            .rebuild_count_for_tests();
        assert_eq!(
            after,
            initial + 1,
            "register_projection_resource (macro resources attr path) must dirty synthesis"
        );

        let debug = app.world().resource::<crate::UiProjectionDirtyDebug>();
        assert!(
            debug
                .last_reasons
                .iter()
                .any(|r| matches!(r, crate::UiDirtyReason::TrackedProjectionResource)),
            "dirty debug should record tracked resource reason: {:?}",
            debug.last_reasons
        );
        assert!(
            !debug.last_dirty_windows.is_empty(),
            "dirty debug should list rebuilt windows"
        );
    }

    #[test]
    fn navigation_item_interaction_state_rebuilds_hover_once_then_returns_to_idle() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));
        app.world_mut().spawn((
            UiRoot,
            UiNavigationView::new([
                NavigationViewItem::new("One"),
                NavigationViewItem::new("Two"),
            ])
            .with_settings_visible(false),
        ));

        app.update();
        let initial_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();

        let item = {
            let mut query = app
                .world_mut()
                .query_filtered::<Entity, With<UiNavigationItem>>();
            query
                .iter(app.world())
                .next()
                .expect("navigation item should be expanded")
        };
        app.world_mut().entity_mut(item).insert(InteractionState {
            hovered: true,
            ..InteractionState::default()
        });

        app.update();
        let changed_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(changed_rebuild_count, initial_rebuild_count + 1);

        app.update();
        let idle_rebuild_count = app
            .world()
            .non_send::<crate::MasonryRuntime>()
            .primary()
            .expect("primary window runtime should exist")
            .rebuild_count_for_tests();
        assert_eq!(idle_rebuild_count, changed_rebuild_count);
    }

    #[test]
    fn xilem_task_proxy_messages_are_routed_back_to_view_handler() {
        #[derive(Resource, Default)]
        struct Routed(bool);

        #[derive(Resource)]
        struct ExpectedRoot(Entity);

        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .register_ui_component::<ProxyTaskRoot>()
            .add_ui_action::<ProxyTaskAction>()
            .insert_resource(Routed(false))
            .add_systems(
                Update,
                |mut reader: MessageReader<UiAction<ProxyTaskAction>>,
                 expected: Res<ExpectedRoot>,
                 mut routed: ResMut<Routed>| {
                    for UiAction { source, action } in reader.read() {
                        if *source == expected.0 && *action == ProxyTaskAction {
                            routed.0 = true;
                        }
                    }
                },
            );

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));
        let task_root = app.world_mut().spawn((UiRoot, ProxyTaskRoot)).id();
        app.insert_resource(ExpectedRoot(task_root));

        app.update();

        let mut routed = false;
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.update();
            if app.world().resource::<Routed>().0 {
                routed = true;
                break;
            }
        }

        assert!(
            routed,
            "task proxy message should be queued, dispatched as UiAction, and visible to MessageReader"
        );
    }

    #[test]
    fn input_bridge_uses_primary_window_cursor_for_click_and_emits_move_before_down_up() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        window.set_cursor_position(Some(Vec2::new(320.0, 180.0)));
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        app.update();

        // CursorMoved payload is intentionally different from Window::cursor_position().
        // The bridge should trust Window state.
        app.world_mut().write_message(CursorMoved {
            window: window_entity,
            position: Vec2::new(12.0, 24.0),
            delta: None,
        });
        app.update();

        {
            let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
            runtime
                .primary_mut()
                .unwrap()
                .clear_pointer_trace_for_tests();
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

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        assert_eq!(
            runtime.primary().unwrap().pointer_position_for_tests(),
            Vec2::new(320.0, 180.0)
        );
        assert_eq!(
            runtime.primary().unwrap().pointer_trace_for_tests(),
            &[
                crate::runtime::PointerTraceEvent::Move,
                crate::runtime::PointerTraceEvent::Down,
                crate::runtime::PointerTraceEvent::Move,
                crate::runtime::PointerTraceEvent::Up,
            ]
        );
    }

    #[test]
    fn input_bridge_uses_primary_window_cursor_for_mouse_wheel_events() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        window.set_cursor_position(Some(Vec2::new(144.0, 96.0)));
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        app.update();

        app.world_mut().write_message(CursorMoved {
            window: window_entity,
            position: Vec2::new(8.0, 8.0),
            delta: None,
        });
        app.update();

        {
            let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
            runtime
                .primary_mut()
                .unwrap()
                .clear_pointer_trace_for_tests();
        }

        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -1.0,
            window: window_entity,
            phase: TouchPhase::Moved,
        });

        app.update();

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        assert_eq!(
            runtime.primary().unwrap().pointer_position_for_tests(),
            Vec2::new(144.0, 96.0)
        );
        assert_eq!(
            runtime.primary().unwrap().pointer_trace_for_tests(),
            &[
                crate::runtime::PointerTraceEvent::Move,
                crate::runtime::PointerTraceEvent::Scroll,
            ]
        );
    }

    #[test]
    fn input_bridge_uses_primary_window_logical_size_for_resize_events() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        app.update();

        {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
            let mut primary_window = query
                .single_mut(world)
                .expect("primary window should exist");
            primary_window.resolution.set(1280.0, 720.0);
        }

        // Event payload is intentionally stale/incorrect; bridge should trust Window state.
        app.world_mut().write_message(WindowResized {
            window: window_entity,
            width: 1.0,
            height: 1.0,
        });

        app.update();

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        assert_eq!(runtime.primary().unwrap().viewport_size(), (1280.0, 720.0));
    }

    #[test]
    fn clicking_text_input_enables_window_ime() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let input = app
            .world_mut()
            .spawn((
                crate::UiTextInput::new("").with_placeholder("Type here"),
                ChildOf(root),
            ))
            .id();

        app.update();
        app.update();

        assert!(
            !app.world()
                .get::<Window>(window_entity)
                .expect("primary window should exist")
                .ime_enabled
        );

        let input_center = widget_center_for_entity(&app, input);
        send_primary_click(&mut app, window_entity, input_center);

        assert!(
            app.world()
                .get::<Window>(window_entity)
                .expect("primary window should exist")
                .ime_enabled
        );
    }

    #[test]
    fn navigation_view_tracks_flex_column_window_height() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        let _window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let nav = spawn_navigation_height_probe(&mut app);

        app.update();

        resize_masonry_runtime(&mut app, 480, 320);
        let short_height = widget_height_for_entity(&app, nav);

        resize_masonry_runtime(&mut app, 480, 640);
        let tall_height = widget_height_for_entity(&app, nav);

        assert!(
            (short_height - 320.0).abs() <= 1.0,
            "nav height should match short viewport, got {short_height}"
        );
        assert!(
            (tall_height - 640.0).abs() <= 1.0,
            "nav height should match tall viewport, got {tall_height}"
        );
    }

    #[test]
    fn navigation_view_tracks_invisible_primary_window_resizes() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let nav = spawn_navigation_height_probe(&mut app);

        app.update();

        resize_primary_window(&mut app, window_entity, 480.0, 320.0);
        let short_height = widget_height_for_entity(&app, nav);

        resize_primary_window(&mut app, window_entity, 480.0, 640.0);
        let tall_height = widget_height_for_entity(&app, nav);

        assert!(
            !app.world()
                .get::<Window>(window_entity)
                .expect("primary window should exist")
                .visible
        );
        assert!(
            (short_height - 320.0).abs() <= 1.0,
            "nav height should match invisible window's short size, got {short_height}"
        );
        assert!(
            (tall_height - 640.0).abs() <= 1.0,
            "nav height should match invisible window's tall size, got {tall_height}"
        );
    }

    #[test]
    fn navigation_view_sidebar_items_are_ecs_interactive_entities() {
        fn token_color(world: &World, name: &str) -> crate::xilem::Color {
            match world
                .resource::<crate::StyleSheet>()
                .tokens
                .get(name)
                .unwrap_or_else(|| panic!("missing color token `{name}`"))
            {
                crate::TokenValue::Color(color) => *color,
                other => panic!("token `{name}` should be a color, got {other:?}"),
            }
        }

        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let nav = spawn_navigation_height_probe(&mut app);
        app.update();

        let items = {
            let mut query = app
                .world_mut()
                .query::<(Entity, &crate::UiNavigationItem)>();
            let mut items = query
                .iter(app.world())
                .filter(|(_, item)| item.nav == nav)
                .map(|(entity, item)| (entity, item.index))
                .collect::<Vec<_>>();
            items.sort_by_key(|(_, index)| *index);
            items
        };
        assert_eq!(items.len(), 2);

        let active_item = items[0].0;
        let inactive_item = items[1].0;

        app.world_mut()
            .entity_mut(active_item)
            .insert(InteractionState {
                hovered: true,
                pressed: false,
                focused: false,
            });
        app.world_mut()
            .entity_mut(inactive_item)
            .insert(InteractionState {
                hovered: true,
                pressed: false,
                focused: false,
            });

        let active_hover = crate::resolve_style_for_entity_classes(
            app.world(),
            active_item,
            ["nav.item", "nav.item.active"],
        );
        let inactive_hover =
            crate::resolve_style_for_entity_classes(app.world(), inactive_item, ["nav.item"]);
        let indicator = crate::resolve_style_for_classes(app.world(), ["nav.item.indicator"]);

        // Selected: WinUI SubtleFill secondary; hover steps to tertiary.
        // Accent lives on the left indicator.
        assert_eq!(
            active_hover.colors.bg,
            Some(token_color(app.world(), "fill-subtle-tertiary"))
        );
        assert_eq!(
            inactive_hover.colors.bg,
            Some(token_color(app.world(), "fill-subtle-secondary"))
        );
        assert_eq!(
            indicator.colors.bg,
            Some(token_color(app.world(), "surface-accent"))
        );

        {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary()
                .expect("primary window runtime should exist");
            let debug_text = format!("entity={}", active_item.to_bits());
            first_widget_by_short_name_and_debug_text(
                window_runtime.render_root.get_layer_root(0),
                "ActionButtonWithChildWidget",
                &debug_text,
            )
            .expect("navigation view should project sidebar items as action buttons");
        }
    }

    #[test]
    fn navigation_view_clips_content_to_container_not_window() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(480.0, 360.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let nav = spawn_navigation_clipping_probe(&mut app);

        app.update();

        resize_primary_window(&mut app, window_entity, 480.0, 360.0);

        let nav_rect = widget_rect_for_entity(&app, nav);
        let nav_subtree = widget_ids_for_entity_subtree(&app, nav);
        let portal_rects = portal_rects_for_entity(&app, nav);

        assert!(
            portal_rects.len() >= 3,
            "navigation view should wrap its root, sidebar, and content in portals, got {portal_rects:?}"
        );
        assert!(
            portal_rects.iter().all(
                |rect| rect.min.y >= nav_rect.min.y - 1.0 && rect.max.y <= nav_rect.max.y + 1.0
            ),
            "portal viewports should stay inside nav rect {nav_rect:?}, got {portal_rects:?}"
        );
        assert!(
            nav_rect.max.y + 4.0 < 360.0,
            "test setup should leave window space below the nav, got nav rect {nav_rect:?}"
        );

        let outside_nav_position = Vec2::new(
            (nav_rect.min.x + nav_rect.width() * 0.5).max(1.0),
            nav_rect.max.y + 4.0,
        );
        let hit_path = hit_path_for_position(&mut app, window_entity, outside_nav_position);

        assert!(
            hit_path
                .iter()
                .all(|widget_id| !nav_subtree.contains(widget_id)),
            "nav content should be clipped by the nav container before window clipping; hit path outside nav at {outside_nav_position:?} still included nav subtree: {hit_path:?}"
        );
    }

    #[test]
    /// Verifies that text input `on_changed` callbacks from the masonry widget tree
    /// are routed to the ECS `UiEventQueue` as `WidgetUiAction::SetTextInput` events.
    ///
    /// The retained `TextInput` widget produces a `TextAction::Changed` when the user
    /// types. This action is sent through `RenderRootSignal::Action`, which the window
    /// runtime catches and re-dispatches through
    /// `route_masonry_view_messages`, so `on_changed` reaches the ECS action path.
    ///
    /// Before the routing system was added, the `RenderRootSignal::Action` was
    /// dropped by the per-window signal sink, so `on_changed`/`on_enter` callbacks
    /// never fired and the composer draft stayed empty (see picuscode issue 4).
    fn route_masonry_view_messages_dispatches_text_input_on_changed() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let input = app
            .world_mut()
            .spawn((
                crate::UiTextInput::new("").with_placeholder("Type here"),
                ChildOf(root),
            ))
            .id();

        // Two updates so synthesis builds the retained tree and the widget map.
        app.update();
        app.update();

        let text_area_id = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary()
                .expect("primary window runtime should exist");
            first_widget_id_by_short_name(window_runtime.render_root.get_layer_root(0), "TextArea")
                .expect("text input should build an inner TextArea widget")
        };

        let routed = {
            let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary_mut()
                .expect("primary window runtime should exist");
            window_runtime.route_test_view_message(
                Box::new(TextAction::Changed("h".to_string())),
                text_area_id,
            )
        };
        assert!(routed, "text input should register a view action source");

        let changed: Vec<_> = app
            .world_mut()
            .resource_mut::<crate::events::InternalUiEventQueue>()
            .drain_actions::<crate::WidgetUiAction>();
        assert!(
            changed.iter().any(|event| {
                match &event.action {
                    crate::WidgetUiAction::SetTextInput {
                        input: changed_input,
                        value,
                    } => *changed_input == input && value == "h",
                    _ => false,
                }
            }),
            "text_input on_changed should route through route_masonry_view_messages, got: {changed:?}"
        );
    }

    #[test]
    fn route_masonry_view_messages_dispatches_search_on_changed() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(480.0, 320.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let search = app
            .world_mut()
            .spawn((crate::UiSearch::new("Find"), ChildOf(root)))
            .id();

        app.update();
        app.update();

        let text_area_id = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary()
                .expect("primary window runtime should exist");
            first_widget_id_by_short_name(window_runtime.render_root.get_layer_root(0), "TextArea")
                .expect("search should build an inner TextArea widget")
        };

        let routed = {
            let mut runtime = app.world_mut().non_send_mut::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary_mut()
                .expect("primary window runtime should exist");
            window_runtime.route_test_view_message(
                Box::new(TextAction::Changed("button".to_string())),
                text_area_id,
            )
        };
        assert!(routed, "search should register a view action source");

        let changed: Vec<_> = app
            .world_mut()
            .resource_mut::<crate::events::InternalUiEventQueue>()
            .drain_actions::<crate::WidgetUiAction>();
        assert!(
            changed.iter().any(|event| {
                match &event.action {
                    crate::WidgetUiAction::SetSearch {
                        search: changed_search,
                        value,
                    } => *changed_search == search && value == "button",
                    _ => false,
                }
            }),
            "search on_changed should route through route_masonry_view_messages, got: {changed:?}"
        );
    }

    #[test]
    fn retry_dirty_appears_in_dirty_budget_as_unthrottled() {
        // P1.6: RetrySurface is retained on WindowRuntime until successful present;
        // while set it must force unthrottled present (G5).
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(99);
        let window = runtime.ensure_window(entity, true);
        window.retry_dirty = true;
        window.needs_redraw = true;
        let dirty = window.collect_dirty_budget();
        assert!(
            dirty.has(frame_driver::DirtyReason::RetrySurface),
            "retry_dirty must appear in DirtyBudget for unthrottled present"
        );
        assert!(dirty.requires_unthrottled_present());

        // After a failed present path, flags stay so the next frame still presents.
        let mut driver = FrameDriver::new();
        let t0 = std::time::Instant::now();
        driver.note_anim_present(t0);
        let decision =
            driver.decide_present(&dirty, Some(std::time::Duration::from_millis(33)), t0);
        assert!(
            decision.do_present,
            "RetrySurface must not be blocked by anim throttle"
        );
    }

    #[test]
    fn visual_runs_match_plan_len_accepts_empty_fallback_and_equal_counts() {
        // Issue 10: empty runs with single fallback plan entry is OK; empty runs
        // with multi-entry plan is not; equal non-zero counts match.
        assert!(visual_runs_match_plan_len(0, 0));
        assert!(visual_runs_match_plan_len(0, 1));
        assert!(!visual_runs_match_plan_len(0, 2));
        assert!(visual_runs_match_plan_len(3, 3));
        assert!(!visual_runs_match_plan_len(2, 3));
    }

    #[test]
    fn step_frame_missing_surface_restores_sticky_dirt() {
        // Paint-path wiring: without a surface, step_frame must not drop
        // resize/retry stickies and must keep content present demand (Issues 1–2 / 6).
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(100);
        let window = runtime.ensure_window(entity, true);
        window.has_painted_once = true;
        window.resize_dirty = true;
        window.retry_dirty = true;
        window.needs_redraw = true;
        window.frame_driver.note_anim_present(std::time::Instant::now());

        let result = window.step_frame(std::time::Duration::from_millis(16));
        assert!(!result.painted, "no surface ⇒ cannot paint");
        assert!(result.wants_redraw(), "must request another frame");
        assert!(
            result.redraw_demand.need_content_present,
            "missing surface is content present demand: {:?}",
            result.redraw_demand
        );
        assert!(
            window.resize_dirty,
            "resize_dirty must survive missing-surface early return"
        );
        assert!(
            window.retry_dirty,
            "retry_dirty must survive missing-surface early return"
        );
        assert!(
            window.needs_redraw,
            "needs_redraw must survive missing-surface early return"
        );
        // G5: decision still wanted encode/present (not throttled away).
        assert!(
            result.decision.do_encode || !result.anim_tick_only,
            "content dirt must not be treated as pure anim skip: {result:?}"
        );
    }

    #[test]
    fn throttled_anim_paint_does_not_leave_input_or_rebuild() {
        // Issue 11: residual post-tick AnimPaint must not reclassify as G5
        // InputOrRebuild on the next collect_dirty_budget.
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(101);
        let window = runtime.ensure_window(entity, true);
        window.has_painted_once = true;
        window.needs_redraw = false;
        window.needs_anim_frame = false;
        window.resize_dirty = false;
        window.retry_dirty = false;
        let snap = window.take_sticky_snapshot();
        assert!(!snap.needs_redraw);

        // Simulate residual redraw from AnimFrame + throttle skip cleanup.
        window.needs_redraw = true;
        window.finish_encode_skip(snap, /*anim_raised_redraw=*/ true, /*throttled=*/ true);

        assert!(
            !window.needs_redraw,
            "throttled AnimPaint must clear needs_redraw"
        );
        assert!(
            window.needs_anim_frame,
            "anim clock must stay scheduled under throttle"
        );

        let dirty = window.collect_dirty_budget();
        assert!(
            !dirty.has(frame_driver::DirtyReason::InputOrRebuild),
            "next frame must not G5-promote residual AnimPaint: {dirty:?}"
        );
        assert!(
            dirty.has(frame_driver::DirtyReason::AnimTick),
            "anim tick should remain scheduled: {dirty:?}"
        );

        // Second decide_present under interval pressure stays throttled for AnimPaint.
        let mut post = DirtyBudget::new();
        post.insert(DirtyReason::AnimTick);
        post.insert(DirtyReason::AnimPaint { layer: 0 });
        window
            .frame_driver
            .note_anim_present(std::time::Instant::now());
        let decision = window.frame_driver.decide_present(
            &post,
            Some(std::time::Duration::from_millis(33)),
            std::time::Instant::now(),
        );
        assert!(
            decision.throttled_anim_present && !decision.do_present,
            "AnimPaint-only must remain throttleable: {decision:?}"
        );
    }

    #[test]
    fn failed_present_does_not_force_redraw_loop() {
        // Issue 12: Failed retains stickies but must not force needs_redraw /
        // permanent RequestRedraw the way Retry does (Phase 0 / surface contract).
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(102);
        let window = runtime.ensure_window(entity, true);
        window.has_painted_once = true;
        // Content dirt without pre-tick needs_redraw — old bug still forced
        // needs_redraw = true on Failed and spun forever.
        window.needs_redraw = false;
        window.resize_dirty = true;
        window.retry_dirty = false;
        window.needs_anim_frame = false;
        let snap = window.take_sticky_snapshot();

        let demand = window.finish_present_attempt(
            RenderFrameResult::Failed,
            snap,
            /*anim_driven_present=*/ false,
            std::time::Instant::now(),
        );
        assert!(
            !window.needs_redraw,
            "Failed must not force needs_redraw (Phase 0 contract)"
        );
        assert!(
            !window.retry_dirty,
            "Failed must not promote retry_dirty"
        );
        assert!(
            window.resize_dirty,
            "content stickies retained for a later external wake"
        );
        // Redraw demand only from independent anim clock, not content stickies.
        assert_eq!(
            demand,
            window.redraw_demand_anim_clock_only(),
            "Failed redraw demand must ignore content stickies"
        );
        assert!(
            !demand.need_content_present,
            "Failed must not set need_content_present"
        );
        // Even with resize_dirty held, after_work would be true — Failed must not
        // use that path when the anim clock is quiet.
        if !window.wants_redraw_anim_clock_only() {
            assert!(
                !demand.any() && window.wants_redraw_after_work(),
                "content stickies alone must not drive Failed wants_redraw"
            );
        }

        // Retry still forces content present + retry sticky.
        window.needs_redraw = false;
        window.retry_dirty = false;
        window.resize_dirty = true;
        let snap = window.take_sticky_snapshot();
        let demand = window.finish_present_attempt(
            RenderFrameResult::Retry,
            snap,
            false,
            std::time::Instant::now(),
        );
        assert!(demand.any(), "Retry must request another frame");
        assert!(
            demand.need_content_present,
            "Retry is content present demand: {demand:?}"
        );
        assert!(window.retry_dirty);
        assert!(window.needs_redraw);
    }

    #[test]
    fn redraw_demand_classifies_anim_vs_content() {
        // P1b.1: NeedAnimTick vs NeedContentPresent as structured flags.
        // Note: `render_root.needs_anim()` may already be true on a fresh window;
        // classify relative to stickies, not absolute silence.
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(103);
        let window = runtime.ensure_window(entity, true);
        window.has_painted_once = true;
        window.needs_redraw = false;
        window.needs_anim_frame = false;
        window.resize_dirty = false;
        window.retry_dirty = false;
        window.theme_or_font_dirty = false;

        // No content stickies ⇒ never content present.
        let baseline = window.redraw_demand_after_work();
        assert!(
            !baseline.need_content_present,
            "no content stickies: {baseline:?}"
        );

        // Anim sticky forces anim demand (content still clear).
        window.needs_anim_frame = true;
        let anim = window.redraw_demand_after_work();
        assert!(anim.need_anim_tick, "anim sticky: {anim:?}");
        assert!(!anim.need_content_present, "anim sticky is not content: {anim:?}");
        assert!(anim.is_anim_only());

        // Content alone (resize is G5 content present). Clear anim sticky;
        // root may still report needs_anim — content flag is what we assert.
        window.needs_anim_frame = false;
        window.resize_dirty = true;
        let content = window.redraw_demand_after_work();
        assert!(
            content.need_content_present,
            "resize must set need_content_present: {content:?}"
        );

        // Both stickies.
        window.needs_anim_frame = true;
        let both = window.redraw_demand_after_work();
        assert!(both.need_anim_tick && both.need_content_present, "{both:?}");

        // Failed path ignores content stickies even when resize is armed.
        window.needs_anim_frame = false;
        let failed_demand = window.redraw_demand_anim_clock_only();
        assert!(
            !failed_demand.need_content_present,
            "Failed path must not demand content: {failed_demand:?}"
        );
        assert_eq!(
            failed_demand.need_anim_tick,
            window.render_root.needs_anim(),
            "Failed anim demand follows root/sticky anim only"
        );
        assert!(
            window.redraw_demand_after_work().need_content_present,
            "after_work still sees resize"
        );
    }

    #[test]
    fn multi_window_host_demand_merge_mirrors_paint_path() {
        // Two host classification outcomes OR-merged like paint_masonry_ui.
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };

        // Window A: Failed path → anim clock only (content stickies ignored).
        let demand_a = {
            let a = runtime.ensure_window(Entity::from_bits(201), true);
            a.has_painted_once = true;
            a.needs_redraw = false;
            a.needs_anim_frame = true;
            a.resize_dirty = true;
            a.retry_dirty = false;
            a.theme_or_font_dirty = false;
            let d = a.redraw_demand_anim_clock_only();
            assert!(!d.need_content_present);
            d
        };

        // Window B: content present forced (Retry / resize path).
        let demand_b = {
            let b = runtime.ensure_window(Entity::from_bits(202), true);
            b.has_painted_once = true;
            b.needs_redraw = false;
            b.needs_anim_frame = false;
            b.resize_dirty = true;
            b.retry_dirty = false;
            b.theme_or_font_dirty = false;
            let d = b.redraw_demand_force_content_present();
            assert!(d.need_content_present);
            d
        };

        let mut merged = RedrawDemand::none();
        merged.merge(demand_a);
        merged.merge(demand_b);
        assert!(
            merged.need_content_present,
            "content window must survive merge: {merged:?}"
        );
        assert_eq!(
            merged.need_anim_tick,
            demand_a.need_anim_tick || demand_b.need_anim_tick,
            "anim flag OR-merged: a={demand_a:?} b={demand_b:?} merged={merged:?}"
        );
        assert!(merged.any(), "single RequestRedraw should be written");
    }

    #[test]
    fn retry_force_content_present_is_explicit() {
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let window = runtime.ensure_window(Entity::from_bits(203), true);
        window.has_painted_once = true;
        window.needs_redraw = false;
        window.needs_anim_frame = false;
        window.resize_dirty = false;
        window.retry_dirty = false;
        let snap = window.take_sticky_snapshot();
        let demand = window.finish_present_attempt(
            RenderFrameResult::Retry,
            snap,
            false,
            std::time::Instant::now(),
        );
        assert!(
            demand.need_content_present,
            "Retry must force content present: {demand:?}"
        );
        assert!(window.retry_dirty && window.needs_redraw);
    }

    #[test]
    fn throttled_anim_encode_skip_is_anim_only_demand() {
        // P1b: after finish_encode_skip (pure AnimTick / throttled AnimPaint),
        // redraw demand is anim-only — never content present (Issue 11 + P1b.1).
        // Drive the host helper directly so pending Masonry redraw signals from
        // construction cannot promote InputOrRebuild the way a full step_frame
        // without a surface would.
        let mut runtime = MasonryRuntime {
            windows: HashMap::new(),
            primary_window: None,
            event_loop_proxy: Arc::new(Mutex::new(None)),
            action_sink: InternalUiActionSink::default(),
        };
        let entity = Entity::from_bits(104);
        let window = runtime.ensure_window(entity, true);
        window.has_painted_once = true;
        window.needs_redraw = false;
        window.needs_anim_frame = false;
        window.resize_dirty = false;
        window.retry_dirty = false;
        window.theme_or_font_dirty = false;
        let snap = window.take_sticky_snapshot();
        assert!(!snap.needs_redraw);

        // Residual AnimPaint + throttle skip cleanup (same as encode-skip path).
        window.needs_redraw = true;
        window.finish_encode_skip(snap, /*anim_raised_redraw=*/ true, /*throttled=*/ true);

        assert!(!window.needs_redraw, "residual AnimPaint cleared");
        assert!(window.needs_anim_frame, "throttle keeps anim scheduled");

        let demand = window.redraw_demand_after_work();
        assert!(
            demand.need_anim_tick,
            "anim clock must stay scheduled: {demand:?}"
        );
        assert!(
            !demand.need_content_present,
            "throttled anim must not demand content present: {demand:?}"
        );
        assert!(demand.is_anim_only(), "{demand:?}");
    }
}

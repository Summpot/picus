use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, mpsc},
};

use crate::xilem::style::Style as _;
use crate::xilem::winit::window::Window as XilemWinitWindow;
use bevy_ecs::{
    entity::Entity,
    message::MessageReader,
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
    ClosingWindow, CursorLeft, CursorMoved, Ime as BevyIme, PrimaryWindow, RawHandleWrapper, Window,
    WindowFocused, WindowResized, WindowScaleFactorChanged, WindowWrapper,
};
use masonry_core::{
    app::{RenderRoot, RenderRootOptions, RenderRootSignal, VisualLayerKind, WindowSizePolicy},
    core::{
        DefaultProperties, Handled, PointerButton, PointerButtonEvent, PointerEvent, PointerId,
        PointerInfo, PointerScrollEvent, PointerState, PointerType, PointerUpdate, ScrollDelta,
        TextEvent, Widget, WidgetId, WidgetRef, WindowEvent,
        keyboard::{Key, KeyState, Modifiers, NamedKey},
    },
    dpi::{PhysicalPosition, PhysicalSize},
    layout::UnitPoint,
    peniko::Color,
    properties::Dimensions,
};
use masonry_imaging::{Layer as ImagingLayer, PreparedFrame, texture_render::Renderer};
use picus_surface::{ExistingWindowMetrics, ExternalWindowSurface};
use picus_view::{
    ViewCtx,
    picus_widget::widgets::Passthrough,
    view::{label, sized_box, zstack},
};
use wgpu::PresentMode;
use xilem_core::{ProxyError, RawProxy, SendMessage, View, ViewId};

use crate::{
    events::{UiEventQueue, install_global_ui_event_queue},
    overlay::OverlayPointerRoutingState,
    projection::{UiAnyView, UiView},
    synthesize::SynthesizedUiViews,
};

#[derive(Debug)]
struct NoopProxy;

impl RawProxy for NoopProxy {
    fn send_message(&self, _path: Arc<[ViewId]>, message: SendMessage) -> Result<(), ProxyError> {
        Err(ProxyError::DriverFinished(message))
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
    window_scale_factor: f64,
    pointer_info: PointerInfo,
    pointer_state: PointerState,
    keyboard_modifiers: Modifiers,
    ime_signal_receiver: mpsc::Receiver<ImeWindowSignal>,
    viewport_width: f64,
    viewport_height: f64,
    window_surface: Option<ExternalWindowSurface>,
    renderer: Renderer,
    #[cfg(test)]
    pointer_trace: Vec<PointerTraceEvent>,
}

impl WindowRuntime {
    fn new(window_entity: Entity, initial_view: UiView) -> Self {
        let mut view_ctx = ViewCtx::new(
            Arc::new(NoopProxy),
            Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime should initialize")),
        );
        let (ime_signal_sender, ime_signal_receiver) = mpsc::channel::<ImeWindowSignal>();

        let (initial_root_widget, view_state) = <UiAnyView as View<(), (), ViewCtx>>::build(
            initial_view.as_ref(),
            &mut view_ctx,
            &mut (),
        );

        let options = RenderRootOptions {
            default_properties: Arc::new(DefaultProperties::new()),
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
            window_scale_factor: 1.0,
            pointer_info: PointerInfo {
                pointer_id: Some(PointerId::new(1).expect("pointer id 1 should be valid")),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            pointer_state: PointerState::default(),
            keyboard_modifiers: Modifiers::empty(),
            ime_signal_receiver,
            viewport_width: initial_viewport.0,
            viewport_height: initial_viewport.1,
            window_surface: None,
            renderer: Renderer::new(),
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
        physical_pos: masonry_core::kurbo::Point,
    ) -> Vec<masonry_core::core::WidgetId> {
        let target = self
            .render_root
            .pointer_capture_target()
            .filter(|widget_id| self.render_root.has_widget(*widget_id))
            .or_else(|| {
                let scale_factor = self.window_scale_factor.max(f64::EPSILON);
                let logical_pos = masonry_core::kurbo::Point::new(
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
        id: masonry_core::core::WidgetId,
    ) -> Option<masonry_core::kurbo::Rect> {
        self.render_root
            .get_widget(id)
            .map(|w| w.ctx().bounding_box())
    }

    /// Returns all layer-0 widget IDs that are direct children of the overlay-root zstack,
    /// for diagnostics. Returns (widget_id, bounding_box) pairs.
    #[must_use]
    pub fn get_overlay_subtree_info(
        &self,
        overlay_widget_id: masonry_core::core::WidgetId,
    ) -> Vec<(
        masonry_core::core::WidgetId,
        masonry_core::kurbo::Rect,
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
    pub(crate) fn clear_pointer_trace_for_tests(&mut self) {
        self.pointer_trace.clear();
    }

    pub fn rebuild_root_view(&mut self, next_view: UiView) {
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

        if let Some(fallback) = focus_fallback_widget(&self.render_root) {
            let _ = self.render_root.set_focus_fallback(Some(fallback));
        }
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
            surface.sync_window_metrics(metrics);
            return true;
        }

        let raw_handle = match RawHandleWrapper::new(window) {
            Ok(raw_handle) => raw_handle,
            Err(error) => {
                tracing::error!("failed to create raw window handle for Masonry surface: {error}");
                return false;
            }
        };

        match ExternalWindowSurface::new_from_bevy_raw_handle(
            raw_handle,
            metrics,
            PresentMode::AutoVsync,
        ) {
            Ok(surface) => {
                self.window_surface = Some(surface);
                true
            }
            Err(error) => {
                tracing::error!("failed to initialize external Masonry surface: {error}");
                false
            }
        }
    }

    pub fn paint_frame(&mut self, delta: std::time::Duration) {
        let _ = self
            .render_root
            .handle_window_event(WindowEvent::AnimFrame(delta));
        let logical_size = self.render_root.size();
        let (visual_layers, _tree_update) = self.render_root.redraw();

        let Some(surface) = self.window_surface.as_mut() else {
            return;
        };

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
            return;
        };
        let VisualLayerKind::Scene(root_scene) = &root_layer.kind else {
            unreachable!("root_layer always returns a scene layer");
        };
        let frame = PreparedFrame::new(
            logical_size.width.max(1),
            logical_size.height.max(1),
            self.window_scale_factor,
            Color::BLACK,
            root_scene,
            &overlays,
        );

        surface.render_frame(&mut self.renderer, frame);
    }

    pub(crate) fn take_pending_ime_signals(&mut self) -> Vec<ImeWindowSignal> {
        self.ime_signal_receiver.try_iter().collect()
    }

    fn sync_window_metrics(&mut self, metrics: ExistingWindowMetrics) {
        let next_scale = metrics.scale_factor.max(f64::EPSILON);
        let next_viewport_width = metrics.logical_width.max(1.0);
        let next_viewport_height = metrics.logical_height.max(1.0);
        let needs_rescale = (self.window_scale_factor - next_scale).abs() > f64::EPSILON;
        let needs_resize = (self.viewport_width - next_viewport_width).abs() > f64::EPSILON
            || (self.viewport_height - next_viewport_height).abs() > f64::EPSILON;

        self.window_scale_factor = next_scale;
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
        }
    }

    /// Register a batch of font bytes into this window's retained font database.
    pub fn register_fonts(&mut self, font_bytes: Vec<u8>) {
        self.render_root
            .register_fonts(masonry_core::peniko::Blob::new(Arc::new(font_bytes)));
    }
}

fn focus_fallback_widget(render_root: &RenderRoot) -> Option<WidgetId> {
    render_root
        .get_layer_root(0)
        .downcast::<Passthrough>()
        .map(|root| root.inner().inner_id())
}

fn existing_window_metrics(window: &XilemWinitWindow) -> ExistingWindowMetrics {
    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);

    ExistingWindowMetrics {
        physical_width: physical_size.width,
        physical_height: physical_size.height,
        logical_width: logical_size.width,
        logical_height: logical_size.height,
        scale_factor,
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

    None
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
}

impl FromWorld for MasonryRuntime {
    fn from_world(world: &mut World) -> Self {
        world.init_resource::<UiEventQueue>();
        let queue = world.resource::<UiEventQueue>().shared_queue();
        install_global_ui_event_queue(queue);

        Self {
            windows: HashMap::new(),
            primary_window: None,
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

    /// Create a window runtime for `entity` if one does not already exist.
    ///
    /// If `is_primary` is `true`, the entity is recorded as the primary window.
    /// Returns the new (or existing) window runtime.
    pub fn ensure_window(&mut self, entity: Entity, is_primary: bool) -> &mut WindowRuntime {
        if is_primary && self.primary_window.is_none() {
            self.primary_window = Some(entity);
        }

        self.windows.entry(entity).or_insert_with(|| {
            WindowRuntime::new(entity, WindowRuntime::initial_placeholder_view())
        })
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
                TextEvent::Ime(masonry_core::core::Ime::Preedit(value.clone(), *cursor)),
            ),
            BevyIme::Commit { window, value } => (
                *window,
                TextEvent::Ime(masonry_core::core::Ime::Commit(value.clone())),
            ),
            BevyIme::Enabled { window } => {
                (*window, TextEvent::Ime(masonry_core::core::Ime::Enabled))
            }
            BevyIme::Disabled { window } => {
                (*window, TextEvent::Ime(masonry_core::core::Ime::Disabled))
            }
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
                masonry_core::core::KeyboardEvent {
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
            window_runtime.handle_text_event(TextEvent::Ime(masonry_core::core::Ime::Commit(
                text.to_string(),
            )));
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
}

/// Attach a Masonry window runtime to each Bevy window once it appears.
///
/// The primary window (marked with [`PrimaryWindow`]) is auto-attached. Other
/// windows are auto-attached as secondary windows.
pub fn initialize_masonry_runtime_from_windows(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    added_window_query: Query<(Entity, Option<&PrimaryWindow>), Added<Window>>,
    window_query: Query<(Entity, Option<&PrimaryWindow>), With<Window>>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    // Gather candidate windows: newly-added windows, or any existing window if
    // the runtime currently has none attached.
    let candidates: Vec<(Entity, bool)> = if runtime.windows.is_empty() {
        window_query
            .iter()
            .map(|(entity, primary)| (entity, primary.is_some()))
            .collect()
    } else {
        added_window_query
            .iter()
            .map(|(entity, primary)| (entity, primary.is_some()))
            .collect()
    };

    for (window_entity, is_primary) in candidates {
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
                .map(|window| existing_window_metrics(window))
        });

        let window_runtime = runtime.ensure_window(window_entity, is_primary);

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
        synthesized.windows.remove(&window_entity);
    }
}

/// PostUpdate rebuild step: diff each window's synthesized root against its
/// retained Masonry tree.
pub fn rebuild_masonry_runtime(world: &mut World) {
    let window_views: Vec<(Entity, UiView)> = world
        .get_resource::<SynthesizedUiViews>()
        .map(|views| views.windows.iter().map(|(e, v)| (*e, v.clone())).collect())
        .unwrap_or_default();

    let Some(mut runtime) = world.get_non_send_mut::<MasonryRuntime>() else {
        return;
    };

    for (window_entity, window_view) in window_views {
        let Some(window_runtime) = runtime.window_mut(window_entity) else {
            continue;
        };
        window_runtime.rebuild_root_view(window_view);
    }
}

/// Last-stage paint pass: submit each window's Masonry scene through Vello and
/// present to the corresponding Bevy window.
pub fn paint_masonry_ui(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    active_window_query: Query<(), (With<Window>, Without<ClosingWindow>)>,
    time: Res<Time>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let window_entities: Vec<Entity> = runtime.window_entities().collect();

    for window_entity in window_entities {
        if !active_window_query.contains(window_entity) {
            runtime.remove_window(window_entity);
            continue;
        }

        let Some(metrics) = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
            let winit_windows = winit_windows.borrow();
            winit_windows
                .get_window(window_entity)
                .map(|window| existing_window_metrics(window))
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
        window_runtime.paint_frame(time.delta());

        bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
            let winit_windows = winit_windows.borrow();
            if let Some(window) = winit_windows.get_window(window_entity) {
                window.request_redraw();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};

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
            synthesized.windows.insert(active, Arc::new(label("active")));
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
}

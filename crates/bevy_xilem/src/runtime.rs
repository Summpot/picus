use std::{fmt::Debug, sync::Arc};

use bevy_ecs::{
    entity::Entity,
    message::MessageReader,
    prelude::{Added, FromWorld, NonSendMut, Query, Res, ResMut, With, World},
};
use bevy_input::{
    ButtonState,
    keyboard::{Key as BevyKey, KeyCode, KeyboardInput},
    mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel},
};
#[cfg(test)]
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_window::{
    CursorLeft, CursorMoved, Ime as BevyIme, PrimaryWindow, RawHandleWrapper, Window,
    WindowFocused, WindowResized, WindowScaleFactorChanged, WindowWrapper,
};
use bevy_xilem_surface::{ExistingWindowMetrics, ExternalWindowSurface};
use masonry::layout::{Dim, UnitPoint};
use masonry::{
    app::{RenderRoot, RenderRootOptions, WindowSizePolicy},
    core::{
        Handled, PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo,
        PointerScrollEvent, PointerState, PointerType, PointerUpdate, ScrollDelta, TextEvent,
        Widget, WidgetId, WidgetRef, WindowEvent,
        keyboard::{Key, KeyState, Modifiers, NamedKey},
    },
    dpi::{PhysicalPosition, PhysicalSize},
    peniko::Color,
    theme::default_property_set,
    vello::{Renderer, wgpu},
    widgets::Passthrough,
};
use xilem::style::Style as _;
use xilem::winit::window::Window as XilemWinitWindow;
use xilem_core::{ProxyError, RawProxy, SendMessage, View, ViewId};
use xilem_masonry::{
    ViewCtx,
    view::{label, zstack},
};

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

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerTraceEvent {
    Move,
    Leave,
    Down,
    Up,
    Scroll,
}

/// Headless Masonry runtime owned by Bevy.
///
/// This runtime keeps ownership of the retained Masonry tree and drives it via
/// explicit Bevy-system input injection + synthesis-time rebuilds.
pub struct MasonryRuntime {
    pub root_widget_id: WidgetId,
    pub render_root: RenderRoot,
    view_ctx: ViewCtx,
    view_state: RuntimeViewState,
    current_view: UiView,
    active_window: Option<Entity>,
    window_scale_factor: f64,
    pointer_info: PointerInfo,
    pointer_state: PointerState,
    keyboard_modifiers: Modifiers,
    viewport_width: f64,
    viewport_height: f64,
    window_surface: Option<ExternalWindowSurface>,
    renderer: Option<Renderer>,
    #[cfg(test)]
    pointer_trace: Vec<PointerTraceEvent>,
}

impl FromWorld for MasonryRuntime {
    fn from_world(world: &mut World) -> Self {
        world.init_resource::<UiEventQueue>();
        let queue = world.resource::<UiEventQueue>().shared_queue();
        install_global_ui_event_queue(queue);

        let mut view_ctx = ViewCtx::new(
            Arc::new(NoopProxy),
            Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime should initialize")),
        );

        let initial_view: UiView = Arc::new(label("bevy_xilem: waiting for synthesized root"));
        let (initial_root_widget, view_state) = <UiAnyView as View<(), (), ViewCtx>>::build(
            initial_view.as_ref(),
            &mut view_ctx,
            &mut (),
        );

        let options = RenderRootOptions {
            default_properties: Arc::new(default_property_set()),
            use_system_fonts: true,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(1024, 768),
            scale_factor: 1.0,
            test_font: None,
        };
        let initial_viewport = (options.size.width as f64, options.size.height as f64);

        let mut render_root =
            RenderRoot::new(initial_root_widget.new_widget.erased(), |_| {}, options);

        if let Some(fallback) = focus_fallback_widget(&render_root) {
            let _ = render_root.set_focus_fallback(Some(fallback));
        }

        let root_widget_id = render_root.get_layer_root(0).id();

        Self {
            root_widget_id,
            render_root,
            view_ctx,
            view_state,
            current_view: initial_view,
            active_window: None,
            window_scale_factor: 1.0,
            pointer_info: PointerInfo {
                pointer_id: Some(PointerId::new(1).expect("pointer id 1 should be valid")),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            pointer_state: PointerState::default(),
            keyboard_modifiers: Modifiers::empty(),
            viewport_width: initial_viewport.0,
            viewport_height: initial_viewport.1,
            window_surface: None,
            renderer: None,
            #[cfg(test)]
            pointer_trace: Vec::new(),
        }
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

impl MasonryRuntime {
    #[must_use]
    pub fn is_attached_to_window(&self, window: Entity) -> bool {
        self.active_window == Some(window)
    }

    pub fn attach_to_window(&mut self, window: Entity, metrics: ExistingWindowMetrics) {
        self.sync_window_metrics(window, metrics);
    }

    #[must_use]
    pub fn viewport_size(&self) -> (f64, f64) {
        (self.viewport_width.max(1.0), self.viewport_height.max(1.0))
    }

    #[must_use]
    pub fn get_hit_path(
        &self,
        physical_pos: masonry::kurbo::Point,
    ) -> Vec<masonry::core::WidgetId> {
        let target = self
            .render_root
            .pointer_capture_target()
            .filter(|widget_id| self.render_root.has_widget(*widget_id))
            .or_else(|| {
                let scale_factor = self.window_scale_factor.max(f64::EPSILON);
                let logical_pos = masonry::kurbo::Point::new(
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

    /// Returns the bounding box of a widget by its id, for diagnostics.
    #[must_use]
    pub fn get_widget_bounding_box(
        &self,
        id: masonry::core::WidgetId,
    ) -> Option<masonry::kurbo::Rect> {
        self.render_root
            .get_widget(id)
            .map(|w| w.ctx().bounding_box())
    }

    /// Returns all layer-0 widget IDs that are direct children of the overlay-root zstack,
    /// for diagnostics. Returns (widget_id, bounding_box) pairs.
    #[must_use]
    pub fn get_overlay_subtree_info(
        &self,
        overlay_widget_id: masonry::core::WidgetId,
    ) -> Vec<(masonry::core::WidgetId, masonry::kurbo::Rect, bool)> {
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

    fn accepts_window(&mut self, window: Entity) -> bool {
        match self.active_window {
            Some(active) => active == window,
            None => {
                self.active_window = Some(window);
                true
            }
        }
    }

    pub fn handle_cursor_moved(&mut self, window: Entity, x: f32, y: f32) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

        self.pointer_state.position = PhysicalPosition {
            x: x as f64,
            y: y as f64,
        };

        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Move);

        self.render_root
            .handle_pointer_event(PointerEvent::Move(PointerUpdate {
                pointer: self.pointer_info.clone(),
                current: self.pointer_state.clone(),
                coalesced: vec![],
                predicted: vec![],
            }))
    }

    pub fn handle_cursor_left(&mut self, window: Entity) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Leave);

        self.render_root
            .handle_pointer_event(PointerEvent::Leave(self.pointer_info.clone()))
    }

    pub fn handle_mouse_button(
        &mut self,
        window: Entity,
        button: MouseButton,
        state: ButtonState,
    ) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

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
                        pointer: self.pointer_info.clone(),
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
                        pointer: self.pointer_info.clone(),
                        button: Some(button),
                        state: self.pointer_state.clone(),
                    }))
            }
        }
    }

    pub fn handle_mouse_wheel(
        &mut self,
        window: Entity,
        unit: MouseScrollUnit,
        x: f32,
        y: f32,
    ) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

        let factor = if unit == MouseScrollUnit::Line {
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            1.0
        };

        #[cfg(test)]
        self.pointer_trace.push(PointerTraceEvent::Scroll);

        self.render_root
            .handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
                pointer: self.pointer_info.clone(),
                delta: ScrollDelta::PixelDelta(PhysicalPosition {
                    x: (x * factor) as f64,
                    y: (y * factor) as f64,
                }),
                state: self.pointer_state.clone(),
            }))
    }

    pub fn handle_text_event(&mut self, window: Entity, event: TextEvent) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

        self.render_root.handle_text_event(event)
    }

    pub fn handle_window_resized(&mut self, window: Entity, width: f32, height: f32) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

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

    pub fn handle_window_scale_factor_changed(
        &mut self,
        window: Entity,
        scale_factor: f64,
    ) -> Handled {
        if !self.accepts_window(window) {
            return Handled::No;
        }

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
        window: &WindowWrapper<xilem::winit::window::Window>,
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
            wgpu::PresentMode::AutoVsync,
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
        let (paint_result, _tree_update) = self.render_root.redraw();

        let Some(surface) = self.window_surface.as_mut() else {
            return;
        };
        surface.render_scene(
            &mut self.renderer,
            paint_result.composite(),
            logical_size.width.max(1),
            logical_size.height.max(1),
            Color::BLACK,
        );
    }

    fn sync_window_metrics(&mut self, window: Entity, metrics: ExistingWindowMetrics) {
        let window_changed = self.active_window != Some(window);
        if window_changed {
            self.active_window = Some(window);
            self.window_surface = None;
            self.renderer = None;
        }

        let next_scale = metrics.scale_factor.max(f64::EPSILON);
        let next_viewport_width = metrics.logical_width.max(1.0);
        let next_viewport_height = metrics.logical_height.max(1.0);
        let needs_rescale = (self.window_scale_factor - next_scale).abs() > f64::EPSILON;
        let needs_resize = (self.viewport_width - next_viewport_width).abs() > f64::EPSILON
            || (self.viewport_height - next_viewport_height).abs() > f64::EPSILON;

        self.window_scale_factor = next_scale;
        self.viewport_width = next_viewport_width;
        self.viewport_height = next_viewport_height;

        if window_changed || needs_rescale {
            let _ = self
                .render_root
                .handle_window_event(WindowEvent::Rescale(self.window_scale_factor));
        }

        if window_changed || needs_resize || needs_rescale {
            let _ = self
                .render_root
                .handle_window_event(WindowEvent::Resize(PhysicalSize::new(
                    metrics.physical_width.max(1),
                    metrics.physical_height.max(1),
                )));
        }
    }
}

fn compose_runtime_root(roots: &[UiView]) -> UiView {
    match roots {
        [] => Arc::new(label("bevy_xilem: no synthesized root")),
        [root] => root.clone(),
        _ => Arc::new(
            zstack(roots.to_vec())
                .alignment(UnitPoint::TOP_LEFT)
                .width(Dim::Stretch)
                .height(Dim::Stretch),
        ),
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

/// PreUpdate input bridge: consume Bevy window/input messages and inject them into Masonry.
pub fn inject_bevy_input_into_masonry(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    mut overlay_routing: ResMut<OverlayPointerRoutingState>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
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

    let Some(primary_window_entity) = primary_window_entity_query.iter().next() else {
        return;
    };

    let Ok(primary_window) = primary_window_query.get(primary_window_entity) else {
        return;
    };

    for event in cursor_moved.read() {
        if event.window != primary_window_entity {
            continue;
        }

        let Some(pointer_position) = primary_window.physical_cursor_position() else {
            continue;
        };

        runtime.handle_cursor_moved(
            primary_window_entity,
            pointer_position.x,
            pointer_position.y,
        );
        tracing::trace!(
            "Input Injection - Bevy Physical Cursor Moved: ({}, {}). Injected into Masonry.",
            pointer_position.x,
            pointer_position.y
        );
    }

    for event in cursor_left.read() {
        if event.window != primary_window_entity {
            continue;
        }

        runtime.handle_cursor_left(primary_window_entity);
    }

    for event in window_focused.read() {
        if event.window != primary_window_entity {
            continue;
        }

        runtime.handle_text_event(
            primary_window_entity,
            TextEvent::WindowFocusChange(event.focused),
        );
    }

    for event in ime_events.read() {
        let (window, text_event) = match event {
            BevyIme::Preedit {
                window,
                value,
                cursor,
            } => (
                *window,
                TextEvent::Ime(masonry::core::Ime::Preedit(value.clone(), *cursor)),
            ),
            BevyIme::Commit { window, value } => (
                *window,
                TextEvent::Ime(masonry::core::Ime::Commit(value.clone())),
            ),
            BevyIme::Enabled { window } => (*window, TextEvent::Ime(masonry::core::Ime::Enabled)),
            BevyIme::Disabled { window } => (*window, TextEvent::Ime(masonry::core::Ime::Disabled)),
        };

        if window != primary_window_entity {
            continue;
        }

        runtime.handle_text_event(primary_window_entity, text_event);
    }

    for event in keyboard_input.read() {
        if event.window != primary_window_entity {
            continue;
        }

        update_modifiers_from_logical_key(
            &mut runtime.keyboard_modifiers,
            &event.logical_key,
            event.state,
        );

        if let Some(key) = map_named_key_from_key_code(event.key_code)
            .map(Key::Named)
            .or_else(|| map_text_key_from_logical_key(&event.logical_key))
        {
            let keyboard_modifiers = runtime.keyboard_modifiers;
            runtime.handle_text_event(
                primary_window_entity,
                TextEvent::Keyboard(masonry::core::KeyboardEvent {
                    state: map_button_state_to_key_state(event.state),
                    key,
                    repeat: event.repeat,
                    modifiers: keyboard_modifiers,
                    ..Default::default()
                }),
            );
            continue;
        }

        if event.state == ButtonState::Pressed
            && let Some(text) = event.text.as_ref()
            && !text.is_empty()
        {
            runtime.handle_text_event(
                primary_window_entity,
                TextEvent::Ime(masonry::core::Ime::Commit(text.to_string())),
            );
        }
    }

    for event in mouse_button_input.read() {
        if event.window != primary_window_entity {
            continue;
        }

        let suppressed = match event.state {
            ButtonState::Pressed => {
                overlay_routing.take_suppressed_press(primary_window_entity, event.button)
            }
            ButtonState::Released => {
                overlay_routing.take_suppressed_release(primary_window_entity, event.button)
            }
        };

        if suppressed {
            continue;
        }

        let Some(pointer_position) = primary_window.physical_cursor_position() else {
            tracing::debug!(
                "skipping mouse button input because primary cursor is outside window {:?}",
                primary_window_entity
            );
            continue;
        };

        runtime.handle_cursor_moved(
            primary_window_entity,
            pointer_position.x,
            pointer_position.y,
        );

        runtime.handle_mouse_button(primary_window_entity, event.button, event.state);
        tracing::trace!(
            "Input Injection - Mouse Button: {:?} {:?} at Physical ({}, {})",
            event.button,
            event.state,
            pointer_position.x,
            pointer_position.y
        );
    }

    for event in mouse_wheel.read() {
        if event.window != primary_window_entity {
            continue;
        }

        let Some(pointer_position) = primary_window.physical_cursor_position() else {
            tracing::debug!(
                "skipping mouse wheel input because primary cursor is outside window {:?}",
                primary_window_entity
            );
            continue;
        };

        runtime.handle_cursor_moved(
            primary_window_entity,
            pointer_position.x,
            pointer_position.y,
        );
        runtime.handle_mouse_wheel(primary_window_entity, event.unit, event.x, event.y);
        tracing::trace!(
            "Input Injection - Mouse Wheel: {:?} ({}, {}) at Physical cursor ({}, {})",
            event.unit,
            event.x,
            event.y,
            pointer_position.x,
            pointer_position.y
        );
    }

    for event in window_resized.read() {
        if event.window != primary_window_entity {
            continue;
        }

        runtime.handle_window_resized(
            primary_window_entity,
            primary_window.width(),
            primary_window.height(),
        );
        tracing::trace!(
            "Window Resize - Bevy Logical Size: {}x{}, Injected into Masonry.",
            primary_window.width(),
            primary_window.height()
        );
    }

    for event in window_scale_factor_changed.read() {
        if event.window != primary_window_entity {
            continue;
        }

        runtime.handle_window_scale_factor_changed(
            primary_window_entity,
            primary_window.scale_factor() as f64,
        );
        tracing::trace!(
            "Window Scale Factor - Bevy Scale: {}, Injected into Masonry.",
            primary_window.scale_factor()
        );
    }
}

/// Attach Masonry runtime viewport state to the primary Bevy winit window once available.
pub fn initialize_masonry_runtime_from_primary_window(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    added_primary_window_query: Query<Entity, (With<PrimaryWindow>, Added<PrimaryWindow>)>,
    primary_window_query: Query<Entity, With<PrimaryWindow>>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let primary_window_entity = added_primary_window_query
        .iter()
        .next()
        .or_else(|| primary_window_query.iter().next());

    let Some(primary_window_entity) = primary_window_entity else {
        return;
    };

    if runtime.is_attached_to_window(primary_window_entity) {
        return;
    }

    let Some(metrics) = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
        let winit_windows = winit_windows.borrow();
        winit_windows
            .get_window(primary_window_entity)
            .map(|window| existing_window_metrics(window))
    }) else {
        return;
    };

    runtime.attach_to_window(primary_window_entity, metrics);

    tracing::trace!(
        "Runtime Init - Primary Window Logic Size: {}x{}, Scale: {}",
        metrics.logical_width,
        metrics.logical_height,
        metrics.scale_factor
    );

    // Prime Masonry's layout root with an explicit initial logical resize so hit-testing
    // never starts from a zero-sized root, even before the first window-resize message.
    runtime.handle_window_resized(
        primary_window_entity,
        metrics.logical_width as f32,
        metrics.logical_height as f32,
    );
    tracing::trace!(
        "Runtime Init - Priming Masonry Resize: {}x{}",
        metrics.logical_width,
        metrics.logical_height
    );
}

/// PostUpdate rebuild step: diff synthesized root against retained Masonry tree.
pub fn rebuild_masonry_runtime(world: &mut World) {
    let Some(roots) = world
        .get_resource::<SynthesizedUiViews>()
        .map(|views| views.roots.clone())
    else {
        return;
    };

    let next_root = compose_runtime_root(&roots);

    let Some(mut runtime) = world.get_non_send_resource_mut::<MasonryRuntime>() else {
        return;
    };

    runtime.rebuild_root_view(next_root);
}

/// Last-stage paint pass: submit Masonry scene through Vello and present to the primary window.
pub fn paint_masonry_ui(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    primary_window_query: Query<Entity, With<PrimaryWindow>>,
    time: Res<Time>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };

    let Some(primary_window_entity) = primary_window_query.iter().next() else {
        return;
    };

    let Some(metrics) = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
        let winit_windows = winit_windows.borrow();
        winit_windows
            .get_window(primary_window_entity)
            .map(|window| existing_window_metrics(window))
    }) else {
        return;
    };

    runtime.attach_to_window(primary_window_entity, metrics);

    let has_surface = bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
        let winit_windows = winit_windows.borrow();
        let Some(window) = winit_windows.get_window(primary_window_entity) else {
            return false;
        };

        runtime.ensure_external_surface(window, metrics)
    });

    if !has_surface {
        return;
    }

    runtime.paint_frame(time.delta());

    bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
        let winit_windows = winit_windows.borrow();
        if let Some(window) = winit_windows.get_window(primary_window_entity) {
            window.request_redraw();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

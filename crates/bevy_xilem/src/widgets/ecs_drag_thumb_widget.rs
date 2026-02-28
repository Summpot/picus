use std::any::TypeId;

use bevy_ecs::entity::Entity;
use masonry::{
    accesskit::{Node, Role},
    core::{
        AccessCtx, AccessEvent, ChildrenIds, EventCtx, HasProperty, LayoutCtx, MeasureCtx,
        PaintCtx, PointerButton, PointerButtonEvent, PointerEvent, PointerUpdate, PropertiesMut,
        PropertiesRef, Property, RegisterCtx, TextEvent, Update, UpdateCtx, Widget, WidgetMut,
        WidgetPod,
    },
    kurbo::Size,
    layout::{LayoutSize, LenReq, SizeDef},
    properties::{Background, BorderColor, BorderWidth, ContentColor, CornerRadius, Padding},
    widgets::Label,
};
use vello::Scene;

use crate::{
    ScrollAxis, WidgetUiAction,
    events::{UiEvent, push_global_ui_event},
    styling::UiInteractionEvent,
};

/// Internal action used to force Xilem driver ticks for drag-thumb state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcsDragThumbWidgetAction {
    StateChanged,
}

/// Masonry widget that emits thumb-drag deltas into the ECS event queue.
pub struct EcsDragThumbWidget {
    entity: Entity,
    axis: ScrollAxis,
    label: WidgetPod<Label>,
    hovered: bool,
    pressed: bool,
    last_axis_position: Option<f64>,
}

impl HasProperty<ContentColor> for EcsDragThumbWidget {}

impl EcsDragThumbWidget {
    #[must_use]
    pub fn new(entity: Entity, axis: ScrollAxis, label: impl Into<masonry::core::ArcStr>) -> Self {
        Self {
            entity,
            axis,
            label: Label::new(label).with_auto_id().to_pod(),
            hovered: false,
            pressed: false,
            last_axis_position: None,
        }
    }

    pub fn set_entity(this: &mut WidgetMut<'_, Self>, entity: Entity) {
        this.widget.entity = entity;
    }

    pub fn set_axis(this: &mut WidgetMut<'_, Self>, axis: ScrollAxis) {
        this.widget.axis = axis;
    }

    pub fn set_label(this: &mut WidgetMut<'_, Self>, label: impl Into<masonry::core::ArcStr>) {
        Label::set_text(&mut this.ctx.get_mut(&mut this.widget.label), label);
    }

    fn axis_position(&self, update: &PointerUpdate) -> f64 {
        match self.axis {
            ScrollAxis::Horizontal => update.current.position.x,
            ScrollAxis::Vertical => update.current.position.y,
        }
    }

    fn axis_position_from_button_event(&self, event: &PointerButtonEvent) -> f64 {
        match self.axis {
            ScrollAxis::Horizontal => event.state.position.x,
            ScrollAxis::Vertical => event.state.position.y,
        }
    }

    fn push_interaction(&self, event: UiInteractionEvent) {
        push_global_ui_event(UiEvent::typed(self.entity, event));
    }

    fn push_drag_delta(&self, delta_pixels: f64) {
        if delta_pixels.abs() <= f64::EPSILON {
            return;
        }

        push_global_ui_event(UiEvent::typed(
            self.entity,
            WidgetUiAction::DragScrollThumb {
                thumb: self.entity,
                axis: self.axis,
                delta_pixels,
            },
        ));
    }

    fn set_hovered(&mut self, hovered: bool) -> bool {
        if self.hovered != hovered {
            self.hovered = hovered;
            self.push_interaction(if hovered {
                UiInteractionEvent::PointerEntered
            } else {
                UiInteractionEvent::PointerLeft
            });
            true
        } else {
            false
        }
    }

    fn set_pressed(&mut self, pressed: bool) -> bool {
        if self.pressed != pressed {
            self.pressed = pressed;
            self.push_interaction(if pressed {
                UiInteractionEvent::PointerPressed
            } else {
                UiInteractionEvent::PointerReleased
            });
            true
        } else {
            false
        }
    }
}

impl Widget for EcsDragThumbWidget {
    type Action = EcsDragThumbWidgetAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(PointerButtonEvent { button, .. }) => {
                if matches!(button, Some(PointerButton::Primary)) {
                    ctx.request_focus();
                    ctx.capture_pointer();
                    self.set_pressed(true);
                    ctx.request_render();
                }
            }
            PointerEvent::Move(update) => {
                if ctx.is_active() {
                    let axis_pos = self.axis_position(update);
                    if let Some(last) = self.last_axis_position {
                        self.push_drag_delta(axis_pos - last);
                        ctx.submit_action::<Self::Action>(EcsDragThumbWidgetAction::StateChanged);
                    }
                    self.last_axis_position = Some(axis_pos);
                }
            }
            PointerEvent::Up(PointerButtonEvent { button, .. }) => {
                if matches!(button, Some(PointerButton::Primary)) {
                    self.set_pressed(false);
                    self.last_axis_position = None;
                    ctx.submit_action::<Self::Action>(EcsDragThumbWidgetAction::StateChanged);
                    ctx.request_render();
                }
            }
            PointerEvent::Leave(..) => {
                self.last_axis_position = None;
            }
            _ => {}
        }

        if let PointerEvent::Down(button_event) = event {
            self.last_axis_position = Some(self.axis_position_from_button_event(button_event));
        }
    }

    fn on_text_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &TextEvent,
    ) {
    }

    fn on_access_event(
        &mut self,
        _ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &AccessEvent,
    ) {
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.label);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(hovered) => {
                if self.set_hovered(*hovered) {
                    ctx.request_render();
                }
            }
            Update::ActiveChanged(active) => {
                if self.set_pressed(*active) {
                    ctx.request_render();
                }
            }
            Update::DisabledChanged(true) => {
                let hover_changed = self.set_hovered(false);
                let pressed_changed = self.set_pressed(false);
                if hover_changed || pressed_changed {
                    ctx.request_render();
                }
                self.last_axis_position = None;
            }
            _ => {}
        }
    }

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        if Padding::matches(property_type) || BorderWidth::matches(property_type) {
            ctx.request_layout();
            ctx.request_render();
            return;
        }

        if ContentColor::matches(property_type)
            || CornerRadius::matches(property_type)
            || BorderColor::matches(property_type)
            || Background::matches(property_type)
        {
            ctx.request_render();
        }
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: masonry::kurbo::Axis,
        len_req: LenReq,
        cross_length: Option<f64>,
    ) -> f64 {
        let auto_length = len_req.into();
        let context_size = LayoutSize::maybe(axis.cross(), cross_length);

        ctx.compute_length(
            &mut self.label,
            auto_length,
            context_size,
            axis,
            cross_length,
        )
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        let child_size = ctx.compute_size(&mut self.label, SizeDef::fit(size), size.into());
        ctx.run_layout(&mut self.label, child_size);

        let child_origin = ((size - child_size).to_vec2() * 0.5).to_point();
        ctx.place_child(&mut self.label, child_origin);
        ctx.derive_baselines(&self.label);
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.label.id()])
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn get_debug_text(&self) -> Option<String> {
        Some(format!("entity={}", self.entity.to_bits()))
    }
}

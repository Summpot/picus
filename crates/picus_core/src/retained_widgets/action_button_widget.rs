use std::any::{Any, TypeId};
use std::sync::Arc;

use bevy_ecs::entity::Entity;
use crate::masonry_core::{
    accesskit::{Node, Role},
    core::keyboard::{Key, NamedKey},
    core::{
        AccessCtx, AccessEvent, ArcStr, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, NewWidget,
        PaintCtx, PointerButton, PointerButtonEvent, PointerEvent, PropertiesMut, PropertiesRef,
        Property, RegisterCtx, TextEvent, Update, UpdateCtx, UsesProperty, Widget, WidgetMut,
        WidgetPod,
    },
    imaging::Painter,
    kurbo::{Axis, Size},
    layout::{LayoutSize, LenReq, Length, SizeDef},
    properties::{Background, BorderColor, BorderWidth, CornerRadius, Padding},
};
use picus_view::picus_widget::{
    properties::{BorderBrush, ContentColor, pre_paint_brush},
    widgets::Label,
};

use crate::{
    events::{InternalUiEvent, UiEvent, push_global_ui_event},
    styling::UiInteractionEvent,
};

use super::HitTransparentWidget;

/// Internal action used to force Xilem driver ticks for ECS button state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionButtonWidgetAction {
    StateChanged,
}

/// Type-erased action payload stored on action buttons.
#[derive(Clone)]
struct StoredAction {
    type_id: TypeId,
    payload: Arc<dyn Any + Send + Sync>,
}

impl StoredAction {
    fn from_typed<A: Clone + Send + Sync + 'static>(action: A) -> Self {
        Self {
            type_id: TypeId::of::<A>(),
            payload: Arc::new(action),
        }
    }

    fn from_erased(type_id: TypeId, payload: Arc<dyn Any + Send + Sync>) -> Self {
        Self { type_id, payload }
    }

    fn push(&self, entity: Entity) {
        push_global_ui_event(InternalUiEvent::erased(
            entity,
            self.type_id,
            Arc::clone(&self.payload),
        ));
    }
}

/// Masonry widget that emits typed ECS actions without user-facing closures.
pub struct ActionButtonWidget<A> {
    entity: Entity,
    action: StoredAction,
    label: WidgetPod<HitTransparentWidget>,
    hovered: bool,
    pressed: bool,
    _marker: std::marker::PhantomData<A>,
}

impl<A> UsesProperty<ContentColor> for ActionButtonWidget<A> where A: Clone + Send + Sync + 'static {}
impl<A> UsesProperty<BorderBrush> for ActionButtonWidget<A> where A: Clone + Send + Sync + 'static {}

impl<A> ActionButtonWidget<A>
where
    A: Clone + Send + Sync + 'static,
{
    #[must_use]
    pub fn new(entity: Entity, action: A, label: impl Into<ArcStr>) -> Self {
        Self {
            entity,
            action: StoredAction::from_typed(action),
            label: NewWidget::new(HitTransparentWidget::new(Label::new(label).prepare())).to_pod(),
            hovered: false,
            pressed: false,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a button that emits a pre-erased [`crate::UiEmit`] payload.
    #[must_use]
    pub fn new_erased(
        entity: Entity,
        type_id: TypeId,
        payload: Arc<dyn Any + Send + Sync>,
        label: impl Into<ArcStr>,
    ) -> ActionButtonWidget<()> {
        ActionButtonWidget {
            entity,
            action: StoredAction::from_erased(type_id, payload),
            label: NewWidget::new(HitTransparentWidget::new(Label::new(label).prepare())).to_pod(),
            hovered: false,
            pressed: false,
            _marker: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub const fn entity(&self) -> Entity {
        self.entity
    }

    pub fn set_entity(this: &mut WidgetMut<'_, Self>, entity: Entity) {
        this.widget.entity = entity;
    }

    pub fn set_action(this: &mut WidgetMut<'_, Self>, action: A) {
        this.widget.action = StoredAction::from_typed(action);
    }

    pub fn set_label(this: &mut WidgetMut<'_, Self>, label: impl Into<ArcStr>) {
        let mut wrapper = this.ctx.get_mut(&mut this.widget.label);
        let mut child = HitTransparentWidget::child_mut(&mut wrapper);
        let mut label_widget = child.downcast::<Label>();
        Label::set_text(&mut label_widget, label);
    }

    fn push_action(&self) {
        self.action.push(self.entity);
    }

    fn push_interaction(&self, event: UiInteractionEvent) {
        push_global_ui_event(UiEvent::typed(self.entity, event));
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

impl<A> Widget for ActionButtonWidget<A>
where
    A: Clone + Send + Sync + 'static,
{
    type Action = ActionButtonWidgetAction;

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(..) => {
                ctx.request_focus();
                ctx.capture_pointer();
                ctx.request_render();
            }
            PointerEvent::Up(PointerButtonEvent { button, .. }) => {
                if matches!(button, Some(PointerButton::Primary))
                    && ctx.is_active()
                    && ctx.is_hovered()
                {
                    self.push_action();
                    ctx.submit_action::<Self::Action>(ActionButtonWidgetAction::StateChanged);
                }
                ctx.request_render();
            }
            PointerEvent::Move(..) | PointerEvent::Leave(..) => {}
            _ => {}
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(event) = event
            && event.state.is_up()
            && (matches!(&event.key, Key::Character(c) if c == " ")
                || event.key == Key::Named(NamedKey::Enter))
        {
            self.push_action();
            ctx.submit_action::<Self::Action>(ActionButtonWidgetAction::StateChanged);
            ctx.request_render();
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if matches!(event.action, crate::masonry_core::accesskit::Action::Click) {
            self.push_action();
            ctx.submit_action::<Self::Action>(ActionButtonWidgetAction::StateChanged);
            ctx.request_render();
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.label);
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(hovered) if self.set_hovered(*hovered) => {
                ctx.request_render();
            }
            Update::ActiveChanged(active) if self.set_pressed(*active) => {
                ctx.request_render();
            }
            Update::DisabledChanged(true) => {
                let hover_changed = self.set_hovered(false);
                let pressed_changed = self.set_pressed(false);
                if hover_changed || pressed_changed {
                    ctx.request_render();
                }
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
            || BorderBrush::matches(property_type)
            || Background::matches(property_type)
        {
            ctx.request_render();
        }
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        cross_length: Option<Length>,
    ) -> Length {
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

    fn pre_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        pre_paint_brush(ctx, props, painter);
    }

    fn paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        _painter: &mut Painter<'_>,
    ) {
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.add_action(crate::masonry_core::accesskit::Action::Click);
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

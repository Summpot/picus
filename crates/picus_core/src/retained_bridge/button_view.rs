use bevy_ecs::entity::Entity;
use crate::masonry_core::core::ArcStr;
use picus_view::{Pod, ViewCtx};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};

use crate::retained_widgets::{ActionButtonWidget, ActionButtonWidgetAction};

/// Picus action-dispatched view backed by Masonry's native `Button` widget.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct ButtonView<A> {
    entity: Entity,
    action: A,
    label: ArcStr,
}

pub fn button_view<A>(entity: Entity, action: A, label: impl Into<ArcStr>) -> ButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    ButtonView {
        entity,
        action,
        label: label.into(),
    }
}

impl<A> ViewMarker for ButtonView<A> where A: Clone + Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for ButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    type Element = Pod<ActionButtonWidget<A>>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut ()) -> (Self::Element, Self::ViewState) {
        (
            ctx.with_action_widget(|ctx| {
                ctx.create_pod(ActionButtonWidget::new(
                    self.entity,
                    self.action.clone(),
                    self.label.clone(),
                ))
            }),
            (),
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _app_state: &mut (),
    ) {
        if self.entity != prev.entity {
            ActionButtonWidget::set_entity(&mut element, self.entity);
        }

        ActionButtonWidget::set_action(&mut element, self.action.clone());

        if self.label != prev.label {
            ActionButtonWidget::set_label(&mut element, self.label.clone());
        }
    }

    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        _message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut (),
    ) -> MessageResult<()> {
        match _message.take_first() {
            None => match _message.take_message::<ActionButtonWidgetAction>() {
                Some(_) => MessageResult::Action(()),
                None => MessageResult::Stale,
            },
            _ => MessageResult::Stale,
        }
    }
}

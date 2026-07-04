use bevy_ecs::entity::Entity;
use picus_view::{Pod, ViewCtx, WidgetView};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};

use crate::retained_widgets::{ActionButtonWidgetAction, ActionButtonWithChildWidget, HitTransparentWidget};

/// Picus action-dispatched button view that accepts an arbitrary child widget view.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct ButtonWithChildView<A, Child> {
    entity: Entity,
    action: A,
    child: Child,
}

pub fn button_with_child_view<A, Child>(
    entity: Entity,
    action: A,
    child: Child,
) -> ButtonWithChildView<A, Child>
where
    A: Clone + Send + Sync + 'static,
    Child: WidgetView<(), ()>,
{
    ButtonWithChildView {
        entity,
        action,
        child,
    }
}

impl<A, Child> ViewMarker for ButtonWithChildView<A, Child>
where
    A: Clone + Send + Sync + 'static,
    Child: WidgetView<(), ()>,
{
}

impl<A, Child> View<(), (), ViewCtx> for ButtonWithChildView<A, Child>
where
    A: Clone + Send + Sync + 'static,
    Child: WidgetView<(), ()>,
{
    type Element = Pod<ActionButtonWithChildWidget<A>>;
    type ViewState = Child::ViewState;

    fn build(&self, ctx: &mut ViewCtx, app_state: &mut ()) -> (Self::Element, Self::ViewState) {
        let (child, child_state) = self.child.build(ctx, app_state);

        (
            ctx.with_action_widget(|ctx| {
                ctx.create_pod(ActionButtonWithChildWidget::new(
                    self.entity,
                    self.action.clone(),
                    child.new_widget,
                ))
            }),
            child_state,
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut (),
    ) {
        if self.entity != prev.entity {
            ActionButtonWithChildWidget::set_entity(&mut element, self.entity);
        }

        ActionButtonWithChildWidget::set_action(&mut element, self.action.clone());

        let mut child_wrapper = ActionButtonWithChildWidget::child_mut(&mut element);
        let mut child = HitTransparentWidget::child_mut(&mut child_wrapper);
        self.child
            .rebuild(&prev.child, view_state, ctx, child.downcast(), app_state);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
    ) {
        {
            let mut child_wrapper = ActionButtonWithChildWidget::child_mut(&mut element);
            let mut child = HitTransparentWidget::child_mut(&mut child_wrapper);
            self.child.teardown(view_state, ctx, child.downcast());
        }
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        message: &mut MessageCtx,
        mut element: Mut<'_, Self::Element>,
        app_state: &mut (),
    ) -> MessageResult<()> {
        if !message.remaining_path().is_empty() {
            let mut child_wrapper = ActionButtonWithChildWidget::child_mut(&mut element);
            let mut child = HitTransparentWidget::child_mut(&mut child_wrapper);
            return self
                .child
                .message(view_state, message, child.downcast(), app_state);
        }

        match message.take_message::<ActionButtonWidgetAction>() {
            Some(_) => MessageResult::Action(()),
            None => MessageResult::Stale,
        }
    }
}

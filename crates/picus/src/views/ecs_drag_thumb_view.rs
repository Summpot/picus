use bevy_ecs::entity::Entity;
use masonry::core::ArcStr;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::{Pod, ViewCtx};

use crate::{
    ScrollAxis,
    widgets::{EcsDragThumbWidget, EcsDragThumbWidgetAction},
};

/// ECS-dispatched view backed by a custom draggable thumb widget.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct EcsDragThumbView {
    entity: Entity,
    axis: ScrollAxis,
    label: ArcStr,
}

#[must_use]
pub fn ecs_drag_thumb(
    entity: Entity,
    axis: ScrollAxis,
    label: impl Into<ArcStr>,
) -> EcsDragThumbView {
    EcsDragThumbView {
        entity,
        axis,
        label: label.into(),
    }
}

impl ViewMarker for EcsDragThumbView {}

impl View<(), (), ViewCtx> for EcsDragThumbView {
    type Element = Pod<EcsDragThumbWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut ()) -> (Self::Element, Self::ViewState) {
        (
            ctx.with_action_widget(|ctx| {
                ctx.create_pod(EcsDragThumbWidget::new(
                    self.entity,
                    self.axis,
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
            EcsDragThumbWidget::set_entity(&mut element, self.entity);
        }

        if self.axis != prev.axis {
            EcsDragThumbWidget::set_axis(&mut element, self.axis);
        }

        if self.label != prev.label {
            EcsDragThumbWidget::set_label(&mut element, self.label.clone());
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
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut (),
    ) -> MessageResult<()> {
        match message.take_first() {
            None => match message.take_message::<EcsDragThumbWidgetAction>() {
                Some(_) => MessageResult::Action(()),
                None => MessageResult::Stale,
            },
            _ => MessageResult::Stale,
        }
    }
}

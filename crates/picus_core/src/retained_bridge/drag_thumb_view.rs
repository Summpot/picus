use bevy_ecs::entity::Entity;
use crate::masonry_core::core::ArcStr;
use picus_view::{Pod, ViewCtx};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};

use crate::{
    ScrollAxis,
    retained_widgets::{DragThumbWidget, DragThumbWidgetAction},
};

/// Picus action-dispatched view backed by a custom draggable thumb widget.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct DragThumbView {
    entity: Entity,
    axis: ScrollAxis,
    label: ArcStr,
}

pub fn drag_thumb_view(
    entity: Entity,
    axis: ScrollAxis,
    label: impl Into<ArcStr>,
) -> DragThumbView {
    DragThumbView {
        entity,
        axis,
        label: label.into(),
    }
}

impl ViewMarker for DragThumbView {}

impl View<(), (), ViewCtx> for DragThumbView {
    type Element = Pod<DragThumbWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut ()) -> (Self::Element, Self::ViewState) {
        (
            ctx.with_action_widget(|ctx| {
                ctx.create_pod(DragThumbWidget::new(
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
            DragThumbWidget::set_entity(&mut element, self.entity);
        }

        if self.axis != prev.axis {
            DragThumbWidget::set_axis(&mut element, self.axis);
        }

        if self.label != prev.label {
            DragThumbWidget::set_label(&mut element, self.label.clone());
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
            None => match message.take_message::<DragThumbWidgetAction>() {
                Some(_) => MessageResult::Action(()),
                None => MessageResult::Stale,
            },
            _ => MessageResult::Stale,
        }
    }
}

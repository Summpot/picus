use bevy_ecs::entity::Entity;
use picus_view::{
    Pod, ViewCtx,
    picus_widget::widgets::{self, ColorSpectrumChanged},
};
use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};

use crate::events::emit_ui_action;

type SpectrumCallback<A> = Box<dyn Fn(f64, f64) -> A + Send + Sync + 'static>;

/// Picus action-dispatching color spectrum view backed by Picus' retained widget backend.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct ColorSpectrumView<A> {
    entity: Entity,
    hue: f64,
    s: f64,
    v: f64,
    map_action: SpectrumCallback<A>,
}

/// Creates a color spectrum view that dispatches overlay actions when the user drags.
pub fn color_spectrum_view<A, F>(
    entity: Entity,
    hue: f64,
    s: f64,
    v: f64,
    map_action: F,
) -> ColorSpectrumView<A>
where
    A: Send + Sync + 'static,
    F: Fn(f64, f64) -> A + Send + Sync + 'static,
{
    ColorSpectrumView {
        entity,
        hue,
        s,
        v,
        map_action: Box::new(map_action),
    }
}

impl<A> ViewMarker for ColorSpectrumView<A> where A: Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for ColorSpectrumView<A>
where
    A: Send + Sync + 'static,
{
    type Element = Pod<widgets::ColorSpectrum>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let element = ctx.with_action_widget(|ctx| {
            let widget = widgets::ColorSpectrum::new(self.hue, self.s, self.v);
            ctx.create_pod(widget)
        });
        (element, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut (),
    ) {
        if prev.hue != self.hue {
            widgets::ColorSpectrum::set_hue(&mut element, self.hue);
        }
        if (prev.s - self.s).abs() > f64::EPSILON || (prev.v - self.v).abs() > f64::EPSILON {
            widgets::ColorSpectrum::set_sv(&mut element, self.s, self.v);
        }
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut (),
    ) -> MessageResult<()> {
        if message.take_first().is_some() {
            tracing::warn!("Got unexpected id path in ColorSpectrumView::message");
            return MessageResult::Stale;
        }

        match message.take_message::<ColorSpectrumChanged>() {
            Some(value) => {
                emit_ui_action(self.entity, (self.map_action)(value.s, value.v));
                MessageResult::Action(())
            }
            None => MessageResult::Stale,
        }
    }
}
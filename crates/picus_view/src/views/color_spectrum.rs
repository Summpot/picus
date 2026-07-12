use std::marker::PhantomData;

use picus_widget::widgets::{self, ColorSpectrumChanged};

use crate::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use crate::{Pod, ViewCtx, WidgetView};

/// A view that displays a [`ColorSpectrum`] widget for 2-D saturation/value selection.
pub struct ColorSpectrumView<State, Action, F> {
    hue: f64,
    s: f64,
    v: f64,
    on_change: F,
    phantom: PhantomData<fn(State) -> Action>,
}

/// Creates a color spectrum widget for selecting saturation and value at a fixed hue.
pub fn color_spectrum<
    State: 'static,
    Action,
    F: Fn(&mut State, f64, f64) -> Action + Send + Sync + 'static,
>(
    hue: f64,
    s: f64,
    v: f64,
    on_change: F,
) -> ColorSpectrumView<State, Action, F>
where
    ColorSpectrumView<State, Action, F>: WidgetView<State, Action>,
{
    ColorSpectrumView {
        hue,
        s,
        v,
        on_change,
        phantom: PhantomData,
    }
}

impl<State, Action, F> ViewMarker for ColorSpectrumView<State, Action, F> {}
impl<F, State, Action> View<State, Action, ViewCtx> for ColorSpectrumView<State, Action, F>
where
    State: 'static,
    Action: 'static,
    F: Fn(&mut State, f64, f64) -> Action + Send + Sync + 'static,
{
    type Element = Pod<widgets::ColorSpectrum>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, Self::ViewState) {
        (
            ctx.with_action_widget(|ctx| {
                let widget = widgets::ColorSpectrum::new(self.hue, self.s, self.v);
                ctx.create_pod(widget)
            }),
            (),
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut State,
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
        _: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        if message.take_first().is_some() {
            tracing::warn!("Got unexpected id path in ColorSpectrumView::message");
            return MessageResult::Stale;
        }
        match message.take_message::<ColorSpectrumChanged>() {
            Some(value) => {
                MessageResult::Action((self.on_change)(app_state, value.s, value.v))
            }
            None => {
                tracing::error!(
                    "Wrong message type in ColorSpectrumView::message: {message:?}, expected {}",
                    std::any::type_name::<ColorSpectrumChanged>(),
                );
                MessageResult::Stale
            }
        }
    }
}
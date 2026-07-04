use std::borrow::Cow;

use bevy_ecs::entity::Entity;
use masonry_core::{
    core::{ArcStr, NewWidget},
    parley::{FontFamily, StyleProperty},
    peniko::Color,
};
use picus_view::{
    Pod, ViewCtx,
    picus_widget::{
        properties::{CheckmarkColor, ContentColor},
        widgets::{self, CheckboxToggled, RadioButtonSelected, SliderMoved, SwitchToggled},
    },
    view::TextInput,
};
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};

use crate::events::emit_ui_action;
use crate::styling::DEFAULT_TEXT_SIZE;

/// Picus action-dispatching checkbox backed by Picus' retained widget backend.
pub fn checkbox_view<A, F>(
    entity: Entity,
    label: impl Into<ArcStr>,
    checked: bool,
    map_action: F,
) -> CheckboxView<A>
where
    A: Send + Sync + 'static,
    F: Fn(bool) -> A + Send + Sync + 'static,
{
    CheckboxView {
        entity,
        label: label.into(),
        checked,
        map_action: Box::new(map_action),
        text_size: DEFAULT_TEXT_SIZE,
        font: FontFamily::List(Cow::Borrowed(&[])),
        text_color: None,
        checkmark_color: None,
        disabled: false,
    }
}

type CheckboxCallback<A> = Box<dyn Fn(bool) -> A + Send + Sync + 'static>;

/// Picus action-dispatching checkbox view with label/checkmark styling support.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct CheckboxView<A> {
    entity: Entity,
    label: ArcStr,
    checked: bool,
    map_action: CheckboxCallback<A>,
    text_size: f32,
    font: FontFamily<'static>,
    text_color: Option<Color>,
    checkmark_color: Option<Color>,
    disabled: bool,
}

impl<A> CheckboxView<A>
where
    A: Send + Sync + 'static,
{
    #[doc(alias = "font_size")]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    pub fn font(mut self, font: impl Into<FontFamily<'static>>) -> Self {
        self.font = font.into();
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn checkmark_color(mut self, color: Color) -> Self {
        self.checkmark_color = Some(color);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<A> ViewMarker for CheckboxView<A> where A: Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for CheckboxView<A>
where
    A: Send + Sync + 'static,
{
    type Element = Pod<widgets::Checkbox>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let label = widgets::Label::new(self.label.clone())
            .with_style(StyleProperty::FontSize(self.text_size))
            .with_style(StyleProperty::FontFamily(self.font.clone()));

        let label = if let Some(color) = self.text_color {
            NewWidget::new(label).with_props(ContentColor::new(color))
        } else {
            NewWidget::new(label)
        };

        let element = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(widgets::Checkbox::from_label(self.checked, label));
            pod.new_widget.options.disabled = self.disabled;
            if let Some(color) = self.checkmark_color {
                pod.new_widget.properties.insert(CheckmarkColor { color });
            }
            pod
        });

        (element, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut (),
    ) {
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if prev.label != self.label {
            widgets::Checkbox::set_text(&mut element, self.label.clone());
        }
        if prev.checked != self.checked {
            widgets::Checkbox::set_checked(&mut element, self.checked);
        }

        let mut label = widgets::Checkbox::label_mut(&mut element);
        if prev.text_size != self.text_size {
            widgets::Label::insert_style(&mut label, StyleProperty::FontSize(self.text_size));
        }
        if prev.font != self.font {
            widgets::Label::insert_style(&mut label, StyleProperty::FontFamily(self.font.clone()));
        }
        if prev.text_color != self.text_color {
            if let Some(color) = self.text_color {
                label.insert_prop(ContentColor::new(color));
            } else {
                label.remove_prop::<ContentColor>();
            }
        }
        drop(label);
        if prev.checkmark_color != self.checkmark_color {
            if let Some(color) = self.checkmark_color {
                element.insert_prop(CheckmarkColor { color });
            } else {
                element.remove_prop::<CheckmarkColor>();
            }
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
        debug_assert!(
            message.remaining_path().is_empty(),
            "id path should be empty in CheckboxView::message"
        );
        match message.take_message::<CheckboxToggled>() {
            Some(checked) => {
                emit_ui_action(self.entity, (self.map_action)(checked.0));
                MessageResult::Action(())
            }
            None => MessageResult::Stale,
        }
    }
}

/// Picus action-dispatching radio button backed by Picus' retained widget backend.
pub fn radio_button_view<A>(
    entity: Entity,
    action: A,
    label: impl Into<ArcStr>,
    checked: bool,
) -> RadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    RadioButtonView {
        entity,
        action,
        label: label.into(),
        checked,
        text_size: DEFAULT_TEXT_SIZE,
        font: FontFamily::List(Cow::Borrowed(&[])),
        text_color: None,
        checkmark_color: None,
        disabled: false,
    }
}

/// Picus action-dispatching radio button view with label styling support.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct RadioButtonView<A> {
    entity: Entity,
    action: A,
    label: ArcStr,
    checked: bool,
    text_size: f32,
    font: FontFamily<'static>,
    text_color: Option<Color>,
    checkmark_color: Option<Color>,
    disabled: bool,
}

impl<A> RadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    #[doc(alias = "font_size")]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    pub fn font(mut self, font: impl Into<FontFamily<'static>>) -> Self {
        self.font = font.into();
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn checkmark_color(mut self, color: Color) -> Self {
        self.checkmark_color = Some(color);
        self
    }
}

impl<A> ViewMarker for RadioButtonView<A> where A: Clone + Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for RadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    type Element = Pod<widgets::RadioButton>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let label = widgets::Label::new(self.label.clone())
            .with_style(StyleProperty::FontSize(self.text_size))
            .with_style(StyleProperty::FontFamily(self.font.clone()));

        let label = if let Some(color) = self.text_color {
            NewWidget::new(label).with_props(ContentColor::new(color))
        } else {
            NewWidget::new(label)
        };

        let element = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(widgets::RadioButton::from_label(self.checked, label));
            pod.new_widget.options.disabled = self.disabled;
            if let Some(color) = self.checkmark_color {
                pod.new_widget.properties.insert(CheckmarkColor { color });
            }
            pod
        });
        (element, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut (),
    ) {
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if prev.label != self.label {
            widgets::RadioButton::set_text(&mut element, self.label.clone());
        }
        if prev.checked != self.checked {
            widgets::RadioButton::set_checked(&mut element, self.checked);
        }

        let mut label = widgets::RadioButton::label_mut(&mut element);
        if prev.text_size != self.text_size {
            widgets::Label::insert_style(&mut label, StyleProperty::FontSize(self.text_size));
        }
        if prev.font != self.font {
            widgets::Label::insert_style(&mut label, StyleProperty::FontFamily(self.font.clone()));
        }
        if prev.text_color != self.text_color {
            if let Some(color) = self.text_color {
                label.insert_prop(ContentColor::new(color));
            } else {
                label.remove_prop::<ContentColor>();
            }
        }
        drop(label);
        if prev.checkmark_color != self.checkmark_color {
            if let Some(color) = self.checkmark_color {
                element.insert_prop(CheckmarkColor { color });
            } else {
                element.remove_prop::<CheckmarkColor>();
            }
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
        debug_assert!(
            message.remaining_path().is_empty(),
            "id path should be empty in RadioButtonView::message"
        );
        match message.take_message::<RadioButtonSelected>() {
            Some(_) => {
                emit_ui_action(self.entity, self.action.clone());
                MessageResult::Action(())
            }
            None => MessageResult::Stale,
        }
    }
}

type SliderCallback<A> = Box<dyn Fn(f64) -> A + Send + Sync + 'static>;

/// Picus action-dispatching slider view backed by Picus' retained widget backend.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct SliderView<A> {
    entity: Entity,
    min: f64,
    max: f64,
    value: f64,
    map_action: SliderCallback<A>,
    step: Option<f64>,
    disabled: bool,
}

pub fn slider_view<A, F>(
    entity: Entity,
    min: f64,
    max: f64,
    value: f64,
    map_action: F,
) -> SliderView<A>
where
    A: Send + Sync + 'static,
    F: Fn(f64) -> A + Send + Sync + 'static,
{
    SliderView {
        entity,
        min,
        max,
        value,
        map_action: Box::new(map_action),
        step: None,
        disabled: false,
    }
}

impl<A> SliderView<A>
where
    A: Send + Sync + 'static,
{
    /// Sets the stepping interval of the slider.
    pub fn step(mut self, step: f64) -> Self {
        if step > 0.0 {
            self.step = Some(step);
        }
        self
    }

    /// Sets whether the slider is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<A> ViewMarker for SliderView<A> where A: Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for SliderView<A>
where
    A: Send + Sync + 'static,
{
    type Element = Pod<widgets::Slider>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let element = ctx.with_action_widget(|ctx| {
            let mut widget = widgets::Slider::new(self.min, self.max, self.value);
            if let Some(step) = self.step {
                widget = widget.with_step(step);
            }
            let mut pod = ctx.create_pod(widget);
            pod.new_widget.options.disabled = self.disabled;
            pod
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
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if prev.value != self.value {
            widgets::Slider::set_value(&mut element, self.value);
        }
        if prev.min != self.min || prev.max != self.max {
            widgets::Slider::set_range(&mut element, self.min, self.max);
        }
        if prev.step != self.step {
            widgets::Slider::set_step(&mut element, self.step);
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
            tracing::warn!("Got unexpected id path in SliderView::message");
            return MessageResult::Stale;
        }

        match message.take_message::<SliderMoved>() {
            Some(value) => {
                emit_ui_action(self.entity, (self.map_action)(value.value));
                MessageResult::Action(())
            }
            None => MessageResult::Stale,
        }
    }
}

type SwitchCallback<A> = Box<dyn Fn(bool) -> A + Send + Sync + 'static>;

/// Picus action-dispatching switch view backed by Picus' retained widget backend.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct SwitchView<A> {
    entity: Entity,
    on: bool,
    map_action: SwitchCallback<A>,
    disabled: bool,
}

pub fn switch_view<A, F>(
    entity: Entity,
    on: bool,
    map_action: F,
) -> SwitchView<A>
where
    A: Send + Sync + 'static,
    F: Fn(bool) -> A + Send + Sync + 'static,
{
    SwitchView {
        entity,
        on,
        map_action: Box::new(map_action),
        disabled: false,
    }
}

impl<A> SwitchView<A>
where
    A: Send + Sync + 'static,
{
    /// Sets whether the switch is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<A> ViewMarker for SwitchView<A> where A: Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for SwitchView<A>
where
    A: Send + Sync + 'static,
{
    type Element = Pod<widgets::Switch>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let element = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(widgets::Switch::new(self.on));
            pod.new_widget.options.disabled = self.disabled;
            pod
        });
        (element, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut (),
    ) {
        if prev.disabled != self.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if prev.on != self.on {
            widgets::Switch::set_on(&mut element, self.on);
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
        debug_assert!(
            message.remaining_path().is_empty(),
            "id path should be empty in SwitchView::message"
        );
        match message.take_message::<SwitchToggled>() {
            Some(switched) => {
                emit_ui_action(self.entity, (self.map_action)(switched.0));
                MessageResult::Action(())
            }
            None => MessageResult::Stale,
        }
    }
}

/// Picus action-dispatching text input backed by Picus' native `TextInput` view.
pub fn text_input_view<A, F>(entity: Entity, contents: String, map_action: F) -> TextInput<()>
where
    A: Send + Sync + 'static,
    F: Fn(String) -> A + Send + Sync + 'static,
{
    picus_view::view::text_input(contents, move |value| {
        emit_ui_action(entity, map_action(value));
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bevy_ecs::world::World;
    use masonry_core::{
        app::{RenderRoot, RenderRootOptions, WindowSizePolicy},
        core::DefaultProperties,
        dpi::PhysicalSize,
    };

    use super::*;
    use xilem_core::{ProxyError, RawProxy, SendMessage, View, ViewId};

    #[derive(Debug)]
    struct NoopProxy;

    impl RawProxy for NoopProxy {
        fn send_message(
            &self,
            _path: Arc<[ViewId]>,
            message: SendMessage,
        ) -> Result<(), ProxyError> {
            Err(ProxyError::DriverFinished(message))
        }

        fn dyn_debug(&self) -> &dyn std::fmt::Debug {
            self
        }
    }

    fn test_view_ctx() -> ViewCtx {
        ViewCtx::new(
            Arc::new(NoopProxy),
            Arc::new(tokio::runtime::Runtime::new().expect("tokio runtime should initialize")),
        )
    }

    fn test_render_root(widget: Pod<widgets::TextInput>) -> RenderRoot {
        RenderRoot::new(
            widget.new_widget.erased(),
            |_| {},
            RenderRootOptions {
                default_properties: Arc::new(DefaultProperties::new()),
                use_system_fonts: true,
                size_policy: WindowSizePolicy::User,
                size: PhysicalSize::new(320, 120),
                scale_factor: 1.0,
                test_font: None,
            },
        )
    }

    #[test]
    fn text_input_rebuild_does_not_reset_user_edit_when_bound_state_has_not_changed_yet() {
        let entity = World::new().spawn_empty().id();
        let prev = text_input_view(entity, String::new(), |_: String| ());
        let next = text_input_view(entity, String::new(), |_: String| ());
        let mut view_ctx = test_view_ctx();
        let (element, mut view_state) =
            <TextInput<()> as View<(), (), ViewCtx>>::build(&prev, &mut view_ctx, &mut ());
        let mut render_root = test_render_root(element);

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            let mut text_area = widgets::TextInput::text_mut(&mut input);
            widgets::TextArea::reset_text(&mut text_area, "typed");
        });

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            <TextInput<()> as View<(), (), ViewCtx>>::rebuild(
                &next,
                &prev,
                &mut view_state,
                &mut view_ctx,
                input.reborrow_mut(),
                &mut (),
            );

            let text_area = widgets::TextInput::text_mut(&mut input);
            assert_eq!(text_area.widget.text(), "typed");
        });
    }

    #[test]
    fn text_input_rebuild_applies_external_text_change_when_bound_state_updates() {
        let entity = World::new().spawn_empty().id();
        let prev = text_input_view(entity, String::new(), |_: String| ());
        let next = text_input_view(entity, "synced".to_string(), |_: String| ());
        let mut view_ctx = test_view_ctx();
        let (element, mut view_state) =
            <TextInput<()> as View<(), (), ViewCtx>>::build(&prev, &mut view_ctx, &mut ());
        let mut render_root = test_render_root(element);

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            let mut text_area = widgets::TextInput::text_mut(&mut input);
            widgets::TextArea::reset_text(&mut text_area, "typed");
        });

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            <TextInput<()> as View<(), (), ViewCtx>>::rebuild(
                &next,
                &prev,
                &mut view_state,
                &mut view_ctx,
                input.reborrow_mut(),
                &mut (),
            );

            let text_area = widgets::TextInput::text_mut(&mut input);
            assert_eq!(text_area.widget.text(), "synced");
        });
    }
}

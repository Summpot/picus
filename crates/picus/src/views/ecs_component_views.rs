use std::borrow::Cow;

use bevy_ecs::entity::Entity;
use masonry::{
    core::{ArcStr, NewWidget, PointerButton, PropertySet},
    parley::Alignment as TextAlign,
    parley::StyleProperty,
    parley::style::FontStack,
    properties::{ContentColor, DisabledContentColor, PlaceholderColor},
    widgets::{self, RadioButtonSelected, TextAction},
};
use vello::peniko::Color;
use xilem_core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem_masonry::view::{
    Button, Checkbox, Label, Slider, Switch, checkbox, slider, switch, text_button,
};
use xilem_masonry::{Pod, ViewCtx};

use crate::events::emit_ui_action;

/// ECS-dispatching variant of `xilem_masonry::view::text_button`.
#[must_use]
pub fn ecs_text_button<A>(
    entity: Entity,
    action: A,
    text: impl Into<ArcStr>,
) -> Button<
    (),
    (),
    impl Fn(&mut (), Option<PointerButton>) -> MessageResult<()> + Send + Sync + 'static,
    Label,
>
where
    A: Clone + Send + Sync + 'static,
{
    text_button(text, move |_| {
        emit_ui_action(entity, action.clone());
    })
}

/// ECS-dispatching variant of `xilem_masonry::view::checkbox`.
#[must_use]
pub fn ecs_checkbox<A, F>(
    entity: Entity,
    label: impl Into<ArcStr>,
    checked: bool,
    map_action: F,
) -> Checkbox<(), (), impl Fn(&mut (), bool) -> () + Send + Sync + 'static>
where
    A: Send + Sync + 'static,
    F: Fn(bool) -> A + Send + Sync + 'static,
{
    checkbox(label, checked, move |_, value| {
        emit_ui_action(entity, map_action(value));
    })
}

/// ECS-dispatching radio button backed by Masonry's native `RadioButton` widget.
#[must_use]
pub fn ecs_radio_button<A>(
    entity: Entity,
    action: A,
    label: impl Into<ArcStr>,
    checked: bool,
) -> EcsRadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    EcsRadioButtonView {
        entity,
        action,
        label: label.into(),
        checked,
        text_size: masonry::theme::TEXT_SIZE_NORMAL,
        font: FontStack::List(Cow::Borrowed(&[])),
        text_color: None,
        disabled: false,
    }
}

/// ECS-dispatching radio button view with label styling support.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct EcsRadioButtonView<A> {
    entity: Entity,
    action: A,
    label: ArcStr,
    checked: bool,
    text_size: f32,
    font: FontStack<'static>,
    text_color: Option<Color>,
    disabled: bool,
}

impl<A> EcsRadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    #[doc(alias = "font_size")]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    pub fn font(mut self, font: impl Into<FontStack<'static>>) -> Self {
        self.font = font.into();
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }
}

impl<A> ViewMarker for EcsRadioButtonView<A> where A: Clone + Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for EcsRadioButtonView<A>
where
    A: Clone + Send + Sync + 'static,
{
    type Element = Pod<widgets::RadioButton>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let label = widgets::Label::new(self.label.clone())
            .with_style(StyleProperty::FontSize(self.text_size))
            .with_style(StyleProperty::FontStack(self.font.clone()));

        let label = if let Some(color) = self.text_color {
            NewWidget::new_with_props(label, ContentColor::new(color))
        } else {
            NewWidget::new(label)
        };

        let element = ctx.with_action_widget(|ctx| {
            let mut pod = ctx.create_pod(widgets::RadioButton::from_label(self.checked, label));
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
            widgets::Label::insert_style(&mut label, StyleProperty::FontStack(self.font.clone()));
        }
        if prev.text_color != self.text_color {
            if let Some(color) = self.text_color {
                label.insert_prop(ContentColor::new(color));
            } else {
                label.remove_prop::<ContentColor>();
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
            "id path should be empty in EcsRadioButtonView::message"
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

/// ECS-dispatching variant of `xilem_masonry::view::slider`.
#[must_use]
pub fn ecs_slider<A, F>(
    entity: Entity,
    min: f64,
    max: f64,
    value: f64,
    map_action: F,
) -> Slider<(), (), impl Fn(&mut (), f64) -> () + Send + Sync + 'static>
where
    A: Send + Sync + 'static,
    F: Fn(f64) -> A + Send + Sync + 'static,
{
    slider(min, max, value, move |_, value| {
        emit_ui_action(entity, map_action(value));
    })
}

/// ECS-dispatching variant of `xilem_masonry::view::switch`.
#[must_use]
pub fn ecs_switch<A, F>(
    entity: Entity,
    on: bool,
    map_action: F,
) -> Switch<(), (), impl Fn(&mut (), bool) -> () + Send + Sync + 'static>
where
    A: Send + Sync + 'static,
    F: Fn(bool) -> A + Send + Sync + 'static,
{
    switch(on, move |_, value| {
        emit_ui_action(entity, map_action(value));
    })
}

/// ECS-dispatching variant of `xilem_masonry::view::text_input`.
#[must_use]
pub fn ecs_text_input<A, F>(entity: Entity, contents: String, map_action: F) -> EcsTextInputView<A>
where
    A: Send + Sync + 'static,
    F: Fn(String) -> A + Send + Sync + 'static,
{
    EcsTextInputView {
        entity,
        contents,
        map_action: Box::new(map_action),
        text_color: None,
        disabled_text_color: None,
        placeholder_color: None,
        placeholder: ArcStr::default(),
        text_alignment: TextAlign::default(),
        text_size: masonry::theme::TEXT_SIZE_NORMAL,
        font: FontStack::List(Cow::Borrowed(&[])),
        disabled: false,
        clip: true,
    }
}

type EcsTextInputCallback<A> = Box<dyn Fn(String) -> A + Send + Sync + 'static>;

/// ECS-dispatching text input backed by Masonry's native `TextInput` widget.
#[must_use = "View values do nothing unless returned into the synthesized UI tree."]
pub struct EcsTextInputView<A> {
    entity: Entity,
    contents: String,
    map_action: EcsTextInputCallback<A>,
    text_color: Option<Color>,
    disabled_text_color: Option<Color>,
    placeholder_color: Option<Color>,
    placeholder: ArcStr,
    text_alignment: TextAlign,
    text_size: f32,
    font: FontStack<'static>,
    disabled: bool,
    clip: bool,
}

impl<A> EcsTextInputView<A>
where
    A: Send + Sync + 'static,
{
    pub fn placeholder(mut self, placeholder_text: impl Into<ArcStr>) -> Self {
        self.placeholder = placeholder_text.into();
        self
    }

    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = Some(color);
        self
    }

    pub fn text_alignment(mut self, text_alignment: TextAlign) -> Self {
        self.text_alignment = text_alignment;
        self
    }

    #[doc(alias = "font_size")]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    pub fn font(mut self, font: impl Into<FontStack<'static>>) -> Self {
        self.font = font.into();
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn disabled_text_color(mut self, color: Color) -> Self {
        self.disabled_text_color = Some(color);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }
}

impl<A> ViewMarker for EcsTextInputView<A> where A: Send + Sync + 'static {}

impl<A> View<(), (), ViewCtx> for EcsTextInputView<A>
where
    A: Send + Sync + 'static,
{
    type Element = Pod<widgets::TextInput>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut ()) -> (Self::Element, Self::ViewState) {
        let text_area = widgets::TextArea::new_editable(&self.contents)
            .with_text_alignment(self.text_alignment)
            .with_style(StyleProperty::FontSize(self.text_size))
            .with_style(StyleProperty::FontStack(self.font.clone()));

        let mut props = PropertySet::new();
        if let Some(color) = self.text_color {
            props.insert(ContentColor { color });
        }
        if let Some(color) = self.disabled_text_color {
            props.insert(DisabledContentColor(ContentColor { color }));
        }

        let text_input =
            widgets::TextInput::from_text_area(NewWidget::new_with_props(text_area, props))
                .with_text_alignment(self.text_alignment)
                .with_clip(self.clip)
                .with_placeholder(self.placeholder.clone());

        let id = text_input.area_pod().id();
        ctx.record_action_source(id);

        let mut pod = ctx.create_pod(text_input);
        pod.new_widget.options.disabled = self.disabled;
        if let Some(color) = self.placeholder_color {
            pod.new_widget
                .properties
                .insert(PlaceholderColor::new(color));
        }
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut (),
    ) {
        if self.text_color != prev.text_color {
            if let Some(color) = self.text_color {
                element.insert_prop(ContentColor { color });
            } else {
                element.remove_prop::<ContentColor>();
            }
        }
        if self.disabled_text_color != prev.disabled_text_color {
            if let Some(color) = self.disabled_text_color {
                element.insert_prop(DisabledContentColor(ContentColor { color }));
            } else {
                element.remove_prop::<DisabledContentColor>();
            }
        }
        if self.placeholder_color != prev.placeholder_color {
            if let Some(color) = self.placeholder_color {
                element.insert_prop(PlaceholderColor::new(color));
            } else {
                element.remove_prop::<PlaceholderColor>();
            }
        }
        if self.placeholder != prev.placeholder {
            widgets::TextInput::set_placeholder(&mut element, self.placeholder.clone());
        }
        if self.disabled != prev.disabled {
            element.ctx.set_disabled(self.disabled);
        }
        if self.clip != prev.clip {
            widgets::TextInput::set_clip(&mut element, self.clip);
        }
        if self.text_alignment != prev.text_alignment {
            widgets::TextInput::set_text_alignment(&mut element, self.text_alignment);
        }

        let mut text_area = widgets::TextInput::text_mut(&mut element);

        if self.contents != prev.contents && text_area.widget.text() != &self.contents {
            widgets::TextArea::reset_text(&mut text_area, &self.contents);
        }

        if self.text_size != prev.text_size {
            widgets::TextArea::insert_style(
                &mut text_area,
                StyleProperty::FontSize(self.text_size),
            );
        }
        if self.font != prev.font {
            widgets::TextArea::insert_style(
                &mut text_area,
                StyleProperty::FontStack(self.font.clone()),
            );
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
        _: &mut (),
    ) -> MessageResult<()> {
        debug_assert!(
            message.remaining_path().is_empty(),
            "id path should be empty in EcsTextInputView::message"
        );
        match message.take_message::<TextAction>() {
            Some(action) => match *action {
                TextAction::Changed(text) => {
                    emit_ui_action(self.entity, (self.map_action)(text));
                    MessageResult::Action(())
                }
                TextAction::Entered(_) => MessageResult::Stale,
            },
            None => MessageResult::Stale,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bevy_ecs::world::World;
    use masonry::{
        app::{RenderRoot, RenderRootOptions, WindowSizePolicy},
        dpi::PhysicalSize,
        theme::default_property_set,
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
                default_properties: Arc::new(default_property_set()),
                use_system_fonts: true,
                size_policy: WindowSizePolicy::User,
                size: PhysicalSize::new(320, 120),
                scale_factor: 1.0,
                test_font: None,
            },
        )
    }

    #[test]
    fn ecs_rebuild_does_not_reset_user_edit_when_bound_state_has_not_changed_yet() {
        let entity = World::new().spawn_empty().id();
        let prev = ecs_text_input(entity, String::new(), |_: String| ());
        let next = ecs_text_input(entity, String::new(), |_: String| ());
        let mut view_ctx = test_view_ctx();
        let (element, mut view_state) =
            <EcsTextInputView<()> as View<(), (), ViewCtx>>::build(&prev, &mut view_ctx, &mut ());
        let mut render_root = test_render_root(element);

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            let mut text_area = widgets::TextInput::text_mut(&mut input);
            widgets::TextArea::reset_text(&mut text_area, "typed");
        });

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            <EcsTextInputView<()> as View<(), (), ViewCtx>>::rebuild(
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
    fn ecs_rebuild_applies_external_text_change_when_bound_state_updates() {
        let entity = World::new().spawn_empty().id();
        let prev = ecs_text_input(entity, String::new(), |_: String| ());
        let next = ecs_text_input(entity, "synced".to_string(), |_: String| ());
        let mut view_ctx = test_view_ctx();
        let (element, mut view_state) =
            <EcsTextInputView<()> as View<(), (), ViewCtx>>::build(&prev, &mut view_ctx, &mut ());
        let mut render_root = test_render_root(element);

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            let mut text_area = widgets::TextInput::text_mut(&mut input);
            widgets::TextArea::reset_text(&mut text_area, "typed");
        });

        render_root.edit_base_layer(|mut root| {
            let mut input = root.downcast::<widgets::TextInput>();
            <EcsTextInputView<()> as View<(), (), ViewCtx>>::rebuild(
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

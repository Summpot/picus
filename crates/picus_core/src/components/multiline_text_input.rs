use bevy_ecs::{entity::Entity, prelude::*};

use crate::{
    ProjectionCtx, StyleClass, UiLabel, UiView, components::UiComponentTemplate,
    templates::ensure_template_part,
};

/// Multiline editable text input.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiMultilineTextInput {
    pub value: String,
    pub placeholder: String,
    pub clip: bool,
    pub wrap: bool,
    pub read_only: bool,
    pub max_length: Option<usize>,
    pub accept_tab: bool,
}

impl UiMultilineTextInput {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            clip: false,
            wrap: true,
            read_only: false,
            max_length: None,
            accept_tab: false,
        }
    }

    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn with_clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    #[must_use]
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self.clip = !wrap;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    #[must_use]
    pub fn without_max_length(mut self) -> Self {
        self.max_length = None;
        self
    }

    #[must_use]
    pub fn accept_tab(mut self, accept_tab: bool) -> Self {
        self.accept_tab = accept_tab;
        self
    }

    #[must_use]
    pub fn clamped_value(&self, value: impl AsRef<str>) -> String {
        match self.max_length {
            Some(max_length) => value.as_ref().chars().take(max_length).collect(),
            None => value.as_ref().to_string(),
        }
    }
}

/// Emitted when [`UiMultilineTextInput`] value changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiMultilineTextInputChanged {
    pub input: Entity,
    pub value: String,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartMultilineTextInputField;

impl UiComponentTemplate for UiMultilineTextInput {
    fn expand(world: &mut World, entity: Entity) {
        let placeholder = world
            .get::<UiMultilineTextInput>(entity)
            .map(|input| input.placeholder.clone());
        let Some(placeholder) = placeholder else {
            return;
        };

        let field = ensure_template_part::<PartMultilineTextInputField, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.multiline_text_input.field".to_string()]),
            )
        });

        if let Some(mut label) = world.get_mut::<UiLabel>(field) {
            label.text = placeholder;
        }
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_multiline_text_input(component, ctx)
    }
}

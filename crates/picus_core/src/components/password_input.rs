use bevy_ecs::{entity::Entity, prelude::*};

use crate::{
    ProjectionCtx, StyleClass, UiLabel, UiView, components::UiComponentTemplate,
    templates::ensure_template_part,
};

/// Password input component.
///
/// The ECS value stores the real text. Projection masks displayed content before
/// handing it to Masonry, providing a lightweight obscured input until Masonry
/// grows a native secure text field.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiPasswordInput {
    pub value: String,
    pub placeholder: String,
    pub mask: char,
    pub read_only: bool,
    pub max_length: Option<usize>,
}

impl UiPasswordInput {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            mask: '•',
            read_only: false,
            max_length: Some(32),
        }
    }

    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn with_mask(mut self, mask: char) -> Self {
        self.mask = mask;
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
    pub fn clamped_value(&self, value: impl AsRef<str>) -> String {
        match self.max_length {
            Some(max_length) => value.as_ref().chars().take(max_length).collect(),
            None => value.as_ref().to_string(),
        }
    }
}

/// Emitted when [`UiPasswordInput`] value changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPasswordInputChanged {
    pub input: Entity,
    pub value: String,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartPasswordInputField;

impl UiComponentTemplate for UiPasswordInput {
    fn expand(world: &mut World, entity: Entity) {
        let placeholder = world
            .get::<UiPasswordInput>(entity)
            .map(|input| input.placeholder.clone());
        let Some(placeholder) = placeholder else {
            return;
        };

        let field = ensure_template_part::<PartPasswordInputField, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.password_input.field".to_string()]),
            )
        });

        if let Some(mut label) = world.get_mut::<UiLabel>(field) {
            label.text = placeholder;
        }
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_password_input(component, ctx)
    }
}

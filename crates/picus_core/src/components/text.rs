use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, TypographyPreset, UiView, components::UiComponentTemplate};

/// A text element that applies a Fluent v9 typography preset.
///
/// Use this instead of a bare `UiLabel` when you want consistent typography
/// styling from the Fluent type ramp (body1, caption1, title1, etc.).
///
/// The text content is resolved through the i18n system when `LocalizeText` is
/// also present on the entity.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiText {
    /// The text content to display.
    pub text: String,
    /// Optional typography preset override.
    ///
    /// When `None`, the preset is read from a separate `TypographyPreset`
    /// component on the same entity (defaults to `Body1`).
    pub preset: Option<TypographyPreset>,
}

impl UiText {
    /// Create a new `UiText` with the given text and typography preset.
    #[must_use]
    pub fn new(text: impl Into<String>, preset: TypographyPreset) -> Self {
        Self {
            text: text.into(),
            preset: Some(preset),
        }
    }

    /// Create a new `UiText` with only text content; the preset is determined
    /// by a separate `TypographyPreset` component (or defaults to `Body1`).
    #[must_use]
    pub fn new_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            preset: None,
        }
    }
}

impl UiComponentTemplate for UiText {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_text(component, ctx)
    }

    fn expand(_world: &mut World, entity: Entity) {
        // If the entity doesn't already have a TypographyPreset, add a default.
        if _world.get::<TypographyPreset>(entity).is_none() {
            _world.entity_mut(entity).insert(TypographyPreset::Body1);
        }
    }
}

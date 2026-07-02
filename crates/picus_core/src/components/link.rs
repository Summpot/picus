use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// A hyperlink-style text component (Fluent v9 Link).
///
/// Renders as interactive text that changes appearance on hover and emits
/// a `UiEvent` when clicked.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiLink {
    /// The link text content.
    pub text: String,
}

impl UiLink {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Action payload carried by the click event from a `UiLink`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiLinkAction {
    /// The entity that owns the link.
    pub target: Entity,
}

impl UiLinkAction {
    #[must_use]
    pub const fn new(target: Entity) -> Self {
        Self { target }
    }
}

impl UiComponentTemplate for UiLink {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_link(component, ctx)
    }
}

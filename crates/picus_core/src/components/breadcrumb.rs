use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// A single breadcrumb item in a navigation path.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiBreadcrumbItem {
    /// The display text for this breadcrumb segment.
    pub label: String,
}

impl UiBreadcrumbItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl UiComponentTemplate for UiBreadcrumbItem {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_breadcrumb_item(component, ctx)
    }
}

/// A breadcrumb navigation path (Fluent v9 Breadcrumb).
///
/// Renders a horizontal list of `UiBreadcrumbItem` children separated by
/// divider icons. The last item is rendered as plain text (current page).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiBreadcrumb;

impl UiComponentTemplate for UiBreadcrumb {
    fn project(_component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_breadcrumb(ctx)
    }
}

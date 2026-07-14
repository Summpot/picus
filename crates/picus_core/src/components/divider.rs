use bevy_ecs::prelude::*;
use crate::masonry_core::kurbo::Axis;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// A horizontal or vertical divider line (Fluent v9 Divider).
///
/// This component draws a thin line to visually separate content sections.
/// The divider's appearance (color, thickness) is controlled by the theme
/// styling system.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiDivider {
    /// The axis along which the divider runs.
    pub axis: Axis,
}

impl UiDivider {
    /// Create a new horizontal divider.
    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            axis: Axis::Horizontal,
        }
    }

    /// Create a new vertical divider.
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            axis: Axis::Vertical,
        }
    }
}

impl Default for UiDivider {
    fn default() -> Self {
        Self::horizontal()
    }
}

impl UiComponentTemplate for UiDivider {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_divider(component, ctx)
    }
}

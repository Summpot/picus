use bevy_ecs::{entity::Entity, prelude::*};

use crate::{OverlayPlacement, ProjectionCtx, UiView, components::UiComponentTemplate};

/// Generic anchored popover surface rendered in the overlay layer.
///
/// This provides shared placement metadata used by anchored floating controls
/// such as dropdown menus, tooltips, picker panels, and app-level popovers.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiPopover {
    /// The anchor entity this popover follows.
    pub anchor: Entity,
    /// Preferred placement relative to the anchor.
    pub placement: OverlayPlacement,
    /// Enables automatic placement flipping when the preferred side overflows.
    pub auto_flip_placement: bool,
    /// Optional width hint used for initial placement estimation.
    pub width: Option<f64>,
    /// Optional height hint used for initial placement estimation.
    pub height: Option<f64>,
}

impl UiPopover {
    #[must_use]
    pub fn new(anchor: Entity) -> Self {
        Self {
            anchor,
            placement: OverlayPlacement::BottomStart,
            auto_flip_placement: true,
            width: None,
            height: None,
        }
    }

    #[must_use]
    pub fn with_placement(mut self, placement: OverlayPlacement) -> Self {
        self.placement = placement;
        self
    }

    #[must_use]
    pub fn with_auto_flip_placement(mut self, auto_flip: bool) -> Self {
        self.auto_flip_placement = auto_flip;
        self
    }

    #[must_use]
    pub fn with_fixed_width(mut self, width: f64) -> Self {
        self.width = Some(width.max(1.0));
        self
    }

    #[must_use]
    pub fn with_fixed_height(mut self, height: f64) -> Self {
        self.height = Some(height.max(1.0));
        self
    }

    #[must_use]
    pub fn with_fixed_size(mut self, width: f64, height: f64) -> Self {
        self.width = Some(width.max(1.0));
        self.height = Some(height.max(1.0));
        self
    }

    #[must_use]
    pub fn size_hint(&self) -> (f64, f64) {
        (self.width.unwrap_or(240.0), self.height.unwrap_or(44.0))
    }
}

impl UiComponentTemplate for UiPopover {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::popover::project_popover(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popover_defaults_to_bottom_start_auto_flip() {
        let anchor = Entity::from_bits(7);
        let popover = UiPopover::new(anchor);

        assert_eq!(popover.anchor, anchor);
        assert_eq!(popover.placement, OverlayPlacement::BottomStart);
        assert!(popover.auto_flip_placement);
        assert_eq!(popover.width, None);
        assert_eq!(popover.height, None);
    }

    #[test]
    fn popover_fixed_size_builder_sets_hints() {
        let anchor = Entity::from_bits(99);
        let popover = UiPopover::new(anchor)
            .with_placement(OverlayPlacement::BottomEnd)
            .with_auto_flip_placement(false)
            .with_fixed_size(132.0, 40.0);

        assert_eq!(popover.placement, OverlayPlacement::BottomEnd);
        assert!(!popover.auto_flip_placement);
        assert_eq!(popover.size_hint(), (132.0, 40.0));
    }
}

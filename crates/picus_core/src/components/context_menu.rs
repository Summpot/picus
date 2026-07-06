use bevy_ecs::{entity::Entity, prelude::*};

use crate::{IconGlyph, ProjectionCtx, UiView, components::UiComponentTemplate};

/// A single item inside a context menu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiContextMenuItem {
    /// Display text.
    pub label: String,
    /// Optional icon glyph and font stack.
    pub icon_glyph: Option<IconGlyph>,
    /// Draw a separator line after this item.
    pub separator_after: bool,
    /// Whether this item can be selected.
    pub enabled: bool,
}

impl UiContextMenuItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon_glyph: None,
            separator_after: false,
            enabled: true,
        }
    }

    /// Attach an icon glyph.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<IconGlyph>) -> Self {
        self.icon_glyph = Some(icon.into());
        self
    }

    /// Draw a separator after this item.
    #[must_use]
    pub fn with_separator(mut self) -> Self {
        self.separator_after = true;
        self
    }

    /// Disable this item.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Marker component for entities that show a context menu on right-click.
#[derive(Component, Debug, Clone, Default)]
pub struct UiContextMenuTrigger {
    /// The menu items to show when right-clicked.
    pub items: Vec<UiContextMenuItem>,
}

impl UiContextMenuTrigger {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = UiContextMenuItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }
}

/// Floating context menu overlay, spawned at the cursor position on right-click.
///
/// This entity carries the menu item list and the trigger entity reference.
#[derive(Component, Debug, Clone)]
pub struct UiContextMenu {
    /// Menu items to render.
    pub items: Vec<UiContextMenuItem>,
    /// The trigger entity that spawned this context menu.
    pub trigger: Entity,
}

impl Default for UiContextMenu {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            trigger: Entity::PLACEHOLDER,
        }
    }
}

/// Emitted when a context menu item is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiContextMenuItemSelected {
    pub trigger: Entity,
    pub index: usize,
    pub label: String,
}

impl UiComponentTemplate for UiContextMenu {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_context_menu(component, ctx)
    }
}

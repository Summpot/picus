use bevy_ecs::{entity::Entity, prelude::*};

use crate::{OverlayPlacement, ProjectionCtx, UiView, components::UiComponentTemplate};

/// A selectable theme variant entry shown by [`UiThemePicker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiThemePickerOption {
    pub variant: String,
    pub label: String,
    pub label_key: Option<String>,
}

impl UiThemePickerOption {
    #[must_use]
    pub fn new(variant: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            variant: variant.into(),
            label: label.into(),
            label_key: None,
        }
    }

    #[must_use]
    pub fn with_label_key(mut self, key: impl Into<String>) -> Self {
        self.label_key = Some(key.into());
        self
    }
}

/// Inline theme-variant picker that opens an anchored dropdown from the normal UI tree.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiThemePicker {
    pub options: Vec<UiThemePickerOption>,
    pub selected: usize,
    pub is_open: bool,
    pub dropdown_placement: OverlayPlacement,
    pub auto_flip_placement: bool,
}

impl UiThemePicker {
    #[must_use]
    pub fn new(options: impl IntoIterator<Item = UiThemePickerOption>) -> Self {
        Self {
            options: options.into_iter().collect(),
            selected: 0,
            is_open: false,
            dropdown_placement: OverlayPlacement::BottomEnd,
            auto_flip_placement: true,
        }
    }

    #[must_use]
    pub fn fluent() -> Self {
        Self::new([
            UiThemePickerOption::new("dark", "Dark"),
            UiThemePickerOption::new("light", "Light"),
            UiThemePickerOption::new("high-contrast", "High Contrast"),
        ])
    }

    #[must_use]
    pub fn with_selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn with_dropdown_placement(mut self, placement: OverlayPlacement) -> Self {
        self.dropdown_placement = placement;
        self
    }

    #[must_use]
    pub fn with_auto_flip_placement(mut self, auto_flip: bool) -> Self {
        self.auto_flip_placement = auto_flip;
        self
    }

    #[must_use]
    pub fn clamped_selected(&self) -> Option<usize> {
        (!self.options.is_empty()).then_some(self.selected.min(self.options.len() - 1))
    }

    #[must_use]
    pub fn active_index_for_variant(&self, active_variant: Option<&str>) -> Option<usize> {
        active_variant
            .and_then(|variant| {
                self.options
                    .iter()
                    .position(|option| option.variant == variant)
            })
            .or_else(|| self.clamped_selected())
    }
}

/// Floating menu panel for an open [`UiThemePicker`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiThemePickerMenu {
    pub anchor: Entity,
}

/// Emitted when a [`UiThemePicker`] selects a new variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiThemePickerChanged {
    pub picker: Entity,
    pub selected: usize,
    pub variant: String,
}

impl UiComponentTemplate for UiThemePicker {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::theme_picker::project_theme_picker(component, ctx)
    }
}

impl UiComponentTemplate for UiThemePickerMenu {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::theme_picker::project_theme_picker_menu(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluent_picker_contains_three_expected_variants() {
        let picker = UiThemePicker::fluent();
        let variants = picker
            .options
            .iter()
            .map(|option| option.variant.as_str())
            .collect::<Vec<_>>();

        assert_eq!(variants, vec!["dark", "light", "high-contrast"]);
        assert_eq!(picker.dropdown_placement, OverlayPlacement::BottomEnd);
        assert!(picker.auto_flip_placement);
    }

    #[test]
    fn active_index_prefers_active_variant_name() {
        let picker = UiThemePicker::fluent().with_selected(0);
        assert_eq!(picker.active_index_for_variant(Some("light")), Some(1));
        assert_eq!(
            picker.active_index_for_variant(Some("high-contrast")),
            Some(2)
        );
        assert_eq!(picker.active_index_for_variant(Some("missing")), Some(0));
    }
}

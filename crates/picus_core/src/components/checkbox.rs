use bevy_ecs::{entity::Entity, prelude::*};

use crate::{
    ProjectionCtx, StyleClass, UiLabel, UiView, components::UiComponentTemplate,
    templates::ensure_template_part,
};

/// Built-in checkbox UI component with ECS-native state.
///
/// Visual states:
/// - `checked = false, indeterminate = false` → unchecked (☐)
/// - `checked = true,  indeterminate = false` → checked (☑)
/// - `indeterminate = true`                   → indeterminate (▬)
///
/// Click behaviour:
/// - Binary (`three_state = false`, default): unchecked ↔ checked. An
///   indeterminate value set programmatically clears to checked on click.
/// - Tri-state (`three_state = true`): cycles
///   unchecked → checked → indeterminate → unchecked on each click.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiCheckbox {
    pub label: String,
    pub checked: bool,
    /// When true the checkbox renders in the indeterminate state regardless of
    /// `checked`.
    pub indeterminate: bool,
    /// When true, clicks cycle through unchecked, checked, and indeterminate.
    /// When false (default), clicks only toggle checked/unchecked.
    pub three_state: bool,
}

impl UiCheckbox {
    #[must_use]
    pub fn new(label: impl Into<String>, checked: bool) -> Self {
        Self {
            label: label.into(),
            checked,
            indeterminate: false,
            three_state: false,
        }
    }

    /// Mark this checkbox as indeterminate (tri-state dash appearance).
    ///
    /// Does not enable click-to-cycle by itself; use [`Self::three_state`] when
    /// the control should also enter indeterminate via user clicks.
    #[must_use]
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Enable the full tri-state click cycle
    /// (unchecked → checked → indeterminate → unchecked).
    #[must_use]
    pub fn three_state(mut self, three_state: bool) -> Self {
        self.three_state = three_state;
        self
    }
}

/// Emitted when [`UiCheckbox`] state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCheckboxChanged {
    pub checkbox: Entity,
    pub checked: bool,
    /// True when the new state is indeterminate.
    pub indeterminate: bool,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartCheckboxIndicator;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartCheckboxLabel;

impl UiComponentTemplate for UiCheckbox {
    fn expand(world: &mut World, entity: Entity) {
        let checkbox = world.get::<UiCheckbox>(entity).cloned();
        let Some(checkbox) = checkbox else {
            return;
        };

        let indicator = ensure_template_part::<PartCheckboxIndicator, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.checkbox.indicator".to_string()]),
            )
        });
        let label_part = ensure_template_part::<PartCheckboxLabel, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["template.checkbox.label".to_string()]),
            )
        });

        if let Some(mut label) = world.get_mut::<UiLabel>(indicator) {
            label.text = if checkbox.indeterminate {
                "▬".to_string()
            } else if checkbox.checked {
                "☑".to_string()
            } else {
                "☐".to_string()
            };
        }
        if let Some(mut label) = world.get_mut::<UiLabel>(label_part) {
            label.text = checkbox.label;
        }
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_checkbox(component, ctx)
    }
}

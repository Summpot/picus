use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Selection mode for [`UiListView`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiListSelectionMode {
    None,
    #[default]
    Single,
    Multiple,
}

/// Lightweight list view for string-backed item lists.
///
/// This is intentionally data-first. Template/virtualized list views can build on
/// the same selection event shape later without replacing this simple component.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiListView {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    pub selected_indices: Vec<usize>,
    pub selection_mode: UiListSelectionMode,
    pub item_height: Option<f64>,
    pub item_padding: Option<f64>,
    pub empty_text: Option<String>,
}

impl UiListView {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            selected: None,
            selected_indices: Vec::new(),
            selection_mode: UiListSelectionMode::Single,
            item_height: None,
            item_padding: None,
            empty_text: None,
        }
    }

    #[must_use]
    pub fn with_selected(mut self, selected: usize) -> Self {
        self.selected = Some(selected);
        self.selected_indices = vec![selected];
        self
    }

    #[must_use]
    pub fn with_selected_indices(
        mut self,
        selected_indices: impl IntoIterator<Item = usize>,
    ) -> Self {
        self.selected_indices = selected_indices.into_iter().collect();
        self.selected = self.selected_indices.first().copied();
        self.selection_mode = UiListSelectionMode::Multiple;
        self
    }

    #[must_use]
    pub fn with_selection_mode(mut self, selection_mode: UiListSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    #[must_use]
    pub fn with_item_height(mut self, item_height: f64) -> Self {
        self.item_height = item_height.is_finite().then_some(item_height.max(0.0));
        self
    }

    #[must_use]
    pub fn with_item_padding(mut self, item_padding: f64) -> Self {
        self.item_padding = item_padding.is_finite().then_some(item_padding.max(0.0));
        self
    }

    #[must_use]
    pub fn with_empty_text(mut self, empty_text: impl Into<String>) -> Self {
        self.empty_text = Some(empty_text.into());
        self
    }

    #[must_use]
    pub fn clamped_selected(&self) -> Option<usize> {
        self.selected.filter(|index| *index < self.items.len())
    }

    #[must_use]
    pub fn clamped_selected_indices(&self) -> Vec<usize> {
        let mut selected = self
            .selected_indices
            .iter()
            .copied()
            .filter(|index| *index < self.items.len())
            .collect::<Vec<_>>();
        if let Some(index) = self.clamped_selected()
            && !selected.contains(&index)
        {
            selected.push(index);
        }
        selected.sort_unstable();
        selected.dedup();
        if matches!(self.selection_mode, UiListSelectionMode::Single) {
            selected.truncate(1);
        }
        selected
    }
}

/// Emitted when a [`UiListView`] selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiListViewSelectionChanged {
    pub list_view: Entity,
    pub selected: Option<usize>,
    pub selected_indices: Vec<usize>,
}

impl UiComponentTemplate for UiListView {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_list_view(component, ctx)
    }
}

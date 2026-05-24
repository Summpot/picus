use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

use super::UiListSelectionMode;

/// Sort direction for a [`UiDataTable`] column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSortDirection {
    Ascending,
    Descending,
}

impl UiSortDirection {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Active sort descriptor for [`UiDataTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiDataTableSort {
    pub column: usize,
    pub direction: UiSortDirection,
}

impl UiDataTableSort {
    #[must_use]
    pub const fn new(column: usize, direction: UiSortDirection) -> Self {
        Self { column, direction }
    }
}

/// Column metadata for [`UiDataTable`].
#[derive(Debug, Clone, PartialEq)]
pub struct UiDataColumn {
    pub id: String,
    pub label: String,
    pub sortable: bool,
    pub width: Option<f64>,
}

impl UiDataColumn {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sortable: true,
            width: None,
        }
    }

    #[must_use]
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    #[must_use]
    pub fn width(mut self, width: f64) -> Self {
        self.width = width.is_finite().then_some(width.max(0.0));
        self
    }
}

/// Data row for [`UiDataTable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDataRow {
    pub id: String,
    pub cells: Vec<String>,
}

impl UiDataRow {
    #[must_use]
    pub fn new(id: impl Into<String>, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
        }
    }
}

/// String-backed selectable data table.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiDataTable {
    pub columns: Vec<UiDataColumn>,
    pub rows: Vec<UiDataRow>,
    pub selected_row: Option<usize>,
    pub selected_rows: Vec<usize>,
    pub selection_mode: UiListSelectionMode,
    pub sort: Option<UiDataTableSort>,
    pub show_header: bool,
    pub striped: bool,
    pub row_height: Option<f64>,
}

impl UiDataTable {
    #[must_use]
    pub fn new(columns: impl IntoIterator<Item = UiDataColumn>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            rows: Vec::new(),
            selected_row: None,
            selected_rows: Vec::new(),
            selection_mode: UiListSelectionMode::Single,
            sort: None,
            show_header: true,
            striped: false,
            row_height: None,
        }
    }

    #[must_use]
    pub fn from_labels(columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            columns: columns
                .into_iter()
                .enumerate()
                .map(|(index, label)| UiDataColumn::new(format!("col-{index}"), label))
                .collect(),
            rows: Vec::new(),
            selected_row: None,
            selected_rows: Vec::new(),
            selection_mode: UiListSelectionMode::Single,
            sort: None,
            show_header: true,
            striped: false,
            row_height: None,
        }
    }

    #[must_use]
    pub fn with_row(mut self, row: UiDataRow) -> Self {
        self.rows.push(row);
        self
    }

    #[must_use]
    pub fn with_cells(
        mut self,
        id: impl Into<String>,
        cells: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.rows.push(UiDataRow::new(id, cells));
        self
    }

    #[must_use]
    pub fn with_selected_row(mut self, selected_row: usize) -> Self {
        self.selected_row = Some(selected_row);
        self.selected_rows = vec![selected_row];
        self
    }

    #[must_use]
    pub fn with_selected_rows(mut self, selected_rows: impl IntoIterator<Item = usize>) -> Self {
        self.selected_rows = selected_rows.into_iter().collect();
        self.selected_row = self.selected_rows.first().copied();
        self.selection_mode = UiListSelectionMode::Multiple;
        self
    }

    #[must_use]
    pub fn with_selection_mode(mut self, selection_mode: UiListSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    #[must_use]
    pub fn with_sort(mut self, sort: UiDataTableSort) -> Self {
        self.sort = Some(sort);
        self
    }

    #[must_use]
    pub fn show_header(mut self, show_header: bool) -> Self {
        self.show_header = show_header;
        self
    }

    #[must_use]
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    #[must_use]
    pub fn with_row_height(mut self, row_height: f64) -> Self {
        self.row_height = row_height.is_finite().then_some(row_height.max(0.0));
        self
    }

    #[must_use]
    pub fn clamped_selected_row(&self) -> Option<usize> {
        self.selected_row.filter(|index| *index < self.rows.len())
    }

    #[must_use]
    pub fn clamped_selected_rows(&self) -> Vec<usize> {
        let mut selected = self
            .selected_rows
            .iter()
            .copied()
            .filter(|index| *index < self.rows.len())
            .collect::<Vec<_>>();
        if let Some(index) = self.clamped_selected_row()
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

    #[must_use]
    pub fn sorted_row_indices(&self) -> Vec<usize> {
        let mut indices = (0..self.rows.len()).collect::<Vec<_>>();
        let Some(sort) = self.sort else {
            return indices;
        };
        if self
            .columns
            .get(sort.column)
            .is_some_and(|column| !column.sortable)
        {
            return indices;
        }
        indices.sort_by(|left, right| {
            let lhs = self.rows[*left]
                .cells
                .get(sort.column)
                .map(String::as_str)
                .unwrap_or_default();
            let rhs = self.rows[*right]
                .cells
                .get(sort.column)
                .map(String::as_str)
                .unwrap_or_default();
            match sort.direction {
                UiSortDirection::Ascending => lhs.cmp(rhs),
                UiSortDirection::Descending => rhs.cmp(lhs),
            }
        });
        indices
    }

    pub fn toggle_sort_column(&mut self, column: usize) -> Option<UiDataTableSort> {
        if self
            .columns
            .get(column)
            .is_some_and(|column| !column.sortable)
        {
            return None;
        }
        let next = match self.sort {
            Some(sort) if sort.column == column => {
                UiDataTableSort::new(column, sort.direction.toggled())
            }
            _ => UiDataTableSort::new(column, UiSortDirection::Ascending),
        };
        self.sort = Some(next);
        Some(next)
    }
}

/// Emitted when a [`UiDataTable`] row selection changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDataTableSelectionChanged {
    pub table: Entity,
    pub selected_row: Option<usize>,
    pub selected_rows: Vec<usize>,
}

/// Emitted when a [`UiDataTable`] sort descriptor changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDataTableSortChanged {
    pub table: Entity,
    pub sort: UiDataTableSort,
}

impl UiComponentTemplate for UiDataTable {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_data_table(component, ctx)
    }
}

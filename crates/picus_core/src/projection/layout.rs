use super::core::{ProjectionCtx, UiView};
use crate::{
    ecs::{UiFlexColumn, UiFlexRow, UiGrid, UiGridAutoFlow, UiGridCell, UiRoot},
    styling::{apply_flex_alignment, apply_widget_style, resolve_style},
};
use bevy_ecs::{entity::Entity, hierarchy::Children};
use masonry::layout::{Dim, Length};
use std::sync::Arc;
use xilem_masonry::style::Style;
use xilem_masonry::view::{FlexExt as _, GridExt as _, GridParams, flex_col, flex_row, grid};

fn child_entity_views(ctx: &ProjectionCtx<'_>) -> Vec<(Entity, UiView)> {
    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    child_entities
        .into_iter()
        .zip(ctx.children.iter().cloned())
        .collect::<Vec<_>>()
}

fn grid_index(value: u32) -> i32 {
    value.min(i32::MAX as u32) as i32
}

#[derive(Clone, Copy)]
struct GridPlacement {
    column: u32,
    row: u32,
    column_span: u32,
    row_span: u32,
}

fn auto_cell_for_index(index: usize, columns: u32, rows: u32, flow: UiGridAutoFlow) -> UiGridCell {
    match flow {
        UiGridAutoFlow::Row => UiGridCell {
            column: (index as u32) % columns.max(1),
            row: (index as u32) / columns.max(1),
            column_span: 1,
            row_span: 1,
            has_column: false,
            has_row: false,
        },
        UiGridAutoFlow::Column => UiGridCell {
            column: (index as u32) / rows.max(1),
            row: (index as u32) % rows.max(1),
            column_span: 1,
            row_span: 1,
            has_column: false,
            has_row: false,
        },
    }
}

fn clamped_placement(cell: UiGridCell, column_count: u32, row_count: u32) -> GridPlacement {
    let column = cell.column.min(column_count.saturating_sub(1));
    let row = cell.row.min(row_count.saturating_sub(1));
    let column_span = cell
        .column_span
        .max(1)
        .min(column_count.saturating_sub(column).max(1));
    let row_span = cell
        .row_span
        .max(1)
        .min(row_count.saturating_sub(row).max(1));

    GridPlacement {
        column,
        row,
        column_span,
        row_span,
    }
}

fn mark_occupied(occupied: &mut [Vec<bool>], placement: GridPlacement) {
    for row in placement.row..placement.row.saturating_add(placement.row_span) {
        let Some(row_cells) = occupied.get_mut(row as usize) else {
            continue;
        };
        for column in placement.column..placement.column.saturating_add(placement.column_span) {
            if let Some(cell) = row_cells.get_mut(column as usize) {
                *cell = true;
            }
        }
    }
}

fn can_place(
    occupied: &[Vec<bool>],
    row: u32,
    column: u32,
    row_span: u32,
    column_span: u32,
) -> bool {
    for r in row..row.saturating_add(row_span) {
        let Some(row_cells) = occupied.get(r as usize) else {
            return false;
        };
        for c in column..column.saturating_add(column_span) {
            if row_cells.get(c as usize).copied().unwrap_or(true) {
                return false;
            }
        }
    }
    true
}

fn find_first_fit(occupied: &[Vec<bool>], row_span: u32, column_span: u32) -> Option<(u32, u32)> {
    let row_count = occupied.len() as u32;
    let column_count = occupied.first().map_or(0, Vec::len) as u32;
    for row in 0..row_count {
        for column in 0..column_count {
            if can_place(occupied, row, column, row_span, column_span) {
                return Some((row, column));
            }
        }
    }
    None
}

fn find_in_row(occupied: &[Vec<bool>], row: u32, row_span: u32, column_span: u32) -> Option<u32> {
    let column_count = occupied.first().map_or(0, Vec::len) as u32;
    (0..column_count).find(|column| can_place(occupied, row, *column, row_span, column_span))
}

fn find_in_column(
    occupied: &[Vec<bool>],
    column: u32,
    row_span: u32,
    column_span: u32,
) -> Option<u32> {
    let row_count = occupied.len() as u32;
    (0..row_count).find(|row| can_place(occupied, *row, column, row_span, column_span))
}

fn auto_place(occupied: &[Vec<bool>], cell: UiGridCell, placement: GridPlacement) -> GridPlacement {
    if cell.has_row && !cell.has_column {
        if let Some(column) = find_in_row(
            occupied,
            placement.row,
            placement.row_span,
            placement.column_span,
        ) {
            return GridPlacement {
                column,
                ..placement
            };
        }
    } else if !cell.has_row && cell.has_column {
        if let Some(row) = find_in_column(
            occupied,
            placement.column,
            placement.row_span,
            placement.column_span,
        ) {
            return GridPlacement { row, ..placement };
        }
    } else if !cell.has_row
        && !cell.has_column
        && let Some((row, column)) =
            find_first_fit(occupied, placement.row_span, placement.column_span)
    {
        return GridPlacement {
            row,
            column,
            ..placement
        };
    }

    placement
}

pub(crate) fn project_ui_root(_: &UiRoot, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(children), &style)
            .gap(Length::px(style.layout.gap))
            .width(Dim::Stretch)
            .height(Dim::Stretch),
        &style,
    ))
}

pub(crate) fn project_flex_column(_: &UiFlexColumn, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_col(children), &style).gap(Length::px(style.layout.gap)),
        &style,
    ))
}

pub(crate) fn project_flex_row(_: &UiFlexRow, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        apply_flex_alignment(flex_row(children), &style).gap(Length::px(style.layout.gap)),
        &style,
    ))
}

pub(crate) fn project_grid(grid_component: &UiGrid, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let pairs = child_entity_views(&ctx);

    let base_columns = grid_component.effective_columns();
    let base_rows = grid_component.effective_rows();
    let mut column_count = base_columns;
    let mut row_count = base_rows;

    let cells = pairs
        .iter()
        .enumerate()
        .map(|(index, (entity, _))| {
            let fallback =
                auto_cell_for_index(index, base_columns, base_rows, grid_component.auto_flow);
            ctx.world
                .get::<UiGridCell>(*entity)
                .copied()
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();

    for cell in &cells {
        if cell.has_column {
            column_count = column_count.max(cell.column.saturating_add(cell.column_span.max(1)));
        }
        if cell.has_row {
            row_count = row_count.max(cell.row.saturating_add(cell.row_span.max(1)));
        }
    }

    let child_count = pairs.len().max(1) as u32;
    match grid_component.auto_flow {
        UiGridAutoFlow::Row => {
            row_count = row_count.max(child_count.div_ceil(column_count.max(1)));
        }
        UiGridAutoFlow::Column => {
            column_count = column_count.max(child_count.div_ceil(row_count.max(1)));
        }
    }

    let mut occupied = vec![vec![false; column_count as usize]; row_count as usize];
    for cell in cells
        .iter()
        .copied()
        .filter(|cell| cell.has_column && cell.has_row)
    {
        mark_occupied(
            &mut occupied,
            clamped_placement(cell, column_count, row_count),
        );
    }

    let positioned_children = pairs
        .into_iter()
        .enumerate()
        .map(|(index, (_entity, view))| {
            let cell = cells[index];
            let mut placement = clamped_placement(cell, column_count, row_count);
            if grid_component.auto_indexing && !(cell.has_column && cell.has_row) {
                placement = auto_place(&occupied, cell, placement);
            }
            mark_occupied(&mut occupied, placement);

            view.grid_item(GridParams::new(
                grid_index(placement.column),
                grid_index(placement.row),
                grid_index(placement.column_span),
                grid_index(placement.row_span),
            ))
        })
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        grid(
            positioned_children,
            grid_index(column_count),
            grid_index(row_count),
        )
        .gap(Length::px(style.layout.gap)),
        &style,
    ))
}

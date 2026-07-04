use crate::helpers::{card, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiButton, UiDataColumn, UiDataRow, UiDataTable,
};

/// DataTable / GridView component examples.
///
/// Corresponds to Fluent UI's DetailsList with multiple columns and sortable headers.
pub fn spawn_grid_view_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let data = card(commands, g, "DataTable / GridView");
    commands.spawn_scene(bsn! {
        template_value(
            UiDataTable::new([
                UiDataColumn::new("file", "File").width(180.0),
                UiDataColumn::new("kind", "Kind"),
                UiDataColumn::new("size", "Size"),
                UiDataColumn::new("changed", "Changed"),
            ])
            .with_row(UiDataRow::new(
                "1",
                ["fba_gallery.cs", "C# sample", "120 KB", "2026-05-21"],
            ))
            .with_row(UiDataRow::new(
                "2",
                ["main.rs", "Rust example", "42 KB", "2026-05-24"],
            ))
            .with_row(UiDataRow::new(
                "3",
                ["gallery.ron", "Theme", "12 KB", "2026-05-24"],
            ))
        )
        ChildOf(data)
    });

    let visual = card(commands, g, "Template Columns");
    note(
        commands,
        visual,
        "String-backed selectable rows, sortable headers, widths, selected row, and stripes are supported.",
    );
    placeholder(
        commands,
        visual,
        "Cell templates / images",
        "UiDataTable currently stores text cells, so per-cell templates and embedded images are not public yet.",
    );

    commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Native Message Placeholder"))
            ChildOf(visual)
        })
        .id()
}

//! Collection control pages (one component per page).

use crate::helpers::{card, class, generated_image, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    UiDataColumn, UiDataRow, UiDataTable, UiListView, UiTable, UiTreeNode,
    scene::{CommandsSceneExt, bsn, template_value},
};

pub fn spawn_list_view_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let single = card(commands, g, "Single selection");
    commands.spawn_scene(bsn! {
        template_value(
            UiListView::new((1..=20).map(|i| format!("Gallery item {i}")))
                .with_selected(4)
                .with_item_height(30.0)
                .with_item_padding(6.0)
        )
        ChildOf(single)
    });

    let multi = card(commands, g, "Multi-selection");
    commands.spawn_scene(bsn! {
        template_value(
            UiListView::new(["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"])
                .with_selected_indices([1, 3])
                .with_item_padding(7.0)
        )
        ChildOf(multi)
    });

    let compact = card(commands, g, "Compact items");
    commands.spawn_scene(bsn! {
        template_value(
            UiListView::new((1..=8).map(|i| format!("Item {i}")))
                .with_selected(2)
                .with_item_padding(7.0)
        )
        ChildOf(compact)
    });
}

pub fn spawn_tree_view_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let tree = card(commands, g, "Hierarchical tree");
    let root_node = commands
        .spawn_scene(bsn! {
            template_value(UiTreeNode::new("Root").expanded())
            ChildOf(tree)
        })
        .id();
    let docs = commands
        .spawn_scene(bsn! {
            template_value(UiTreeNode::new("Documents").expanded())
            ChildOf(root_node)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiTreeNode::new("report.pdf"))
        ChildOf(docs)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTreeNode::new("notes.txt"))
        ChildOf(docs)
    });
    let src = commands
        .spawn_scene(bsn! {
            template_value(UiTreeNode::new("src").expanded())
            ChildOf(root_node)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiTreeNode::new("main.rs"))
        ChildOf(src)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTreeNode::new("widgets.rs"))
        ChildOf(src)
    });
    note(
        commands,
        tree,
        "UiTreeNode children form the hierarchy; expanded() opens a node by default.",
    );
}

pub fn spawn_table_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let table = card(commands, g, "Simple table");
    commands.spawn_scene(bsn! {
        template_value(
            UiTable::new(["Name", "Role", "Status", "Score"])
                .with_row(["Alice Chen", "Engineer", "Active", "98"])
                .with_row(["Bob Smith", "Designer", "Away", "85"])
                .with_row(["Carol Davis", "Manager", "Busy", "91"])
        )
        template_value(class("gallery.table"))
        ChildOf(table)
    });
    note(
        commands,
        table,
        "UiTable is a lightweight header + string-row table for compact data.",
    );
}

pub fn spawn_data_table_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let data = card(commands, g, "Columns and rows");
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

    let visual = card(commands, g, "Image cell templates");
    note(
        commands,
        visual,
        "UiDataCell::Image renders an inline image inside a data table cell.",
    );
    commands.spawn_scene(bsn! {
        template_value(
            UiDataTable::new([
                UiDataColumn::new("icon", "Icon").width(64.0),
                UiDataColumn::new("name", "Name"),
                UiDataColumn::new("status", "Status"),
            ])
            .with_row(
                UiDataRow::new("1", ["", "Project Alpha", "Active"])
                    .with_cell_image(0, generated_image()),
            )
            .with_row(
                UiDataRow::new("2", ["", "Project Beta", "Archived"])
                    .with_cell_image(0, generated_image()),
            )
        )
        ChildOf(visual)
    });
}

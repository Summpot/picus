use crate::helpers::{card, class, grid};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{UiButton, UiListView, UiTable, UiTreeNode};

/// ListView, TreeView, and Table component examples.
///
/// Corresponds to Fluent UI's DetailsList, TreeView, and Table components.
pub fn spawn_lists_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let list = card(commands, g, "ListView");
    commands.spawn((
        UiListView::new((1..=20).map(|i| format!("Gallery item {i}")))
            .with_selected(4)
            .with_item_height(30.0)
            .with_item_padding(6.0),
        ChildOf(list),
    ));

    let multi = card(commands, g, "Multi-selection List");
    commands.spawn((
        UiListView::new(["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"])
            .with_selected_indices([1, 3])
            .with_item_padding(7.0),
        ChildOf(multi),
    ));

    let tree = card(commands, g, "TreeView");
    let root_node = commands
        .spawn((UiTreeNode::new("Root").expanded(), ChildOf(tree)))
        .id();
    let docs = commands
        .spawn((UiTreeNode::new("Documents").expanded(), ChildOf(root_node)))
        .id();
    commands.spawn((UiTreeNode::new("report.pdf"), ChildOf(docs)));
    commands.spawn((UiTreeNode::new("notes.txt"), ChildOf(docs)));
    let src = commands
        .spawn((UiTreeNode::new("src").expanded(), ChildOf(root_node)))
        .id();
    commands.spawn((UiTreeNode::new("main.rs"), ChildOf(src)));
    commands.spawn((UiTreeNode::new("widgets.rs"), ChildOf(src)));

    let table = card(commands, g, "Table");
    commands.spawn((
        UiTable::new(["Name", "Role", "Status", "Score"])
            .with_row(["Alice Chen", "Engineer", "Active", "98"])
            .with_row(["Bob Smith", "Designer", "Away", "85"])
            .with_row(["Carol Davis", "Manager", "Busy", "91"]),
        class("gallery.table"),
        ChildOf(table),
    ));

    commands
        .spawn((UiButton::new("Prompt Placeholder"), ChildOf(list)))
        .id()
}

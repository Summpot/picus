use crate::helpers::{card, class, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    UiButton, UiCheckbox, UiFlexColumn, UiGroupBox, UiLabel, UiListView, UiMultilineTextInput,
    UiSplitPane, UiTabBar, UiTextInput,
};

/// GroupBox, SplitPane, TabBar, and Popover component examples.
///
/// Corresponds to Fluent UI's GroupBox, SplitPane, Pivot/Tabs, and Popover components.
pub fn spawn_panels_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let group_box = card(commands, g, "GroupBox / Cards");
    let inner = commands
        .spawn((UiGroupBox::new("Nested group"), ChildOf(group_box)))
        .id();
    commands.spawn((
        UiLabel::new("Labels and controls can be grouped."),
        ChildOf(inner),
    ));
    commands.spawn((UiCheckbox::new("Inside a group", true), ChildOf(inner)));

    let split = card(commands, g, "SplitPane");
    let pane = commands
        .spawn((UiSplitPane::new(0.42), ChildOf(split)))
        .id();
    let left = commands
        .spawn((UiFlexColumn, class("gallery.split_panel"), ChildOf(pane)))
        .id();
    commands.spawn((UiLabel::new("Left panel"), ChildOf(left)));
    commands.spawn((
        UiListView::new(["One", "Two", "Three"]).with_selected(0),
        ChildOf(left),
    ));
    let right = commands
        .spawn((UiFlexColumn, class("gallery.split_panel"), ChildOf(pane)))
        .id();
    commands.spawn((UiLabel::new("Right panel"), ChildOf(right)));
    commands.spawn((UiTextInput::new("Resizable split content"), ChildOf(right)));

    let tabs = card(commands, g, "Tabs");
    let tab_bar = commands
        .spawn((
            UiTabBar::new(["Details", "Settings", "Logs"]),
            ChildOf(tabs),
        ))
        .id();
    commands.spawn((UiLabel::new("Details tab content"), ChildOf(tab_bar)));
    commands.spawn((UiCheckbox::new("Enable option", true), ChildOf(tab_bar)));
    commands.spawn((
        UiMultilineTextInput::new("Log line 1\nLog line 2"),
        ChildOf(tab_bar),
    ));

    let popover = card(commands, g, "Popover");
    let pop_btn = commands
        .spawn((UiButton::new("Open popover dialog"), ChildOf(popover)))
        .id();
    note(
        commands,
        popover,
        "Picus popovers are used by combo boxes, menus, pickers, and tooltips.",
    );

    pop_btn
}

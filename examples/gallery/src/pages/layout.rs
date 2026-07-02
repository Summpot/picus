use crate::helpers::{card, class, grid, placeholder, sample_canvas};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    UiBadge, UiButton, UiFlexRow, UiGrid, UiGridCell, UiGridLength, UiLabel, UiTextInput,
};

/// StackPanel/Flex, Grid, and Canvas/Absolute layout component examples.
///
/// Corresponds to Fluent UI's Stack, Grid layout primitives, and positioning.
pub fn spawn_layout_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let flex = card(commands, g, "StackPanel / Flex");
    let row = commands.spawn((UiFlexRow, ChildOf(flex))).id();
    commands.spawn((UiBadge::new("Auto"), ChildOf(row)));
    commands.spawn((UiBadge::new("Stretch"), ChildOf(row)));
    commands.spawn((UiBadge::new("Gap"), ChildOf(row)));
    commands.spawn((UiTextInput::new("Horizontal row"), ChildOf(flex)));

    let grid_card = card(commands, g, "Grid");
    let layout_grid = commands
        .spawn((
            UiGrid::new(3, 3)
                .with_column_tracks([
                    UiGridLength::auto(),
                    UiGridLength::star(1.0),
                    UiGridLength::px(160.0),
                ])
                .with_row_tracks([
                    UiGridLength::px(40.0),
                    UiGridLength::star(1.0),
                    UiGridLength::auto(),
                ])
                .show_grid_lines(true),
            ChildOf(grid_card),
        ))
        .id();
    commands.spawn((
        UiLabel::new("Span 2 columns"),
        class("gallery.swatch.blue"),
        UiGridCell::new(0, 0).with_span(2, 1),
        ChildOf(layout_grid),
    ));
    commands.spawn((
        UiLabel::new("Auto"),
        class("gallery.swatch.green"),
        UiGridCell::new(2, 0),
        ChildOf(layout_grid),
    ));
    commands.spawn((
        UiLabel::new("Star"),
        class("gallery.swatch.gold"),
        UiGridCell::new(0, 1).with_span(3, 1),
        ChildOf(layout_grid),
    ));

    let canvas_panel = card(commands, g, "Canvas / Absolute");
    commands.spawn((
        sample_canvas(),
        class("gallery.canvas"),
        ChildOf(canvas_panel),
    ));
    placeholder(
        commands,
        canvas_panel,
        "Right/bottom attached canvas children",
        "UiCanvasPosition stores right/bottom intent, but the current projector only applies left/top offsets.",
    );

    commands
        .spawn((UiButton::new("Confetti Placeholder"), ChildOf(canvas_panel)))
        .id()
}

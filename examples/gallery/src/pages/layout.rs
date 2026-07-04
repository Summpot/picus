use crate::helpers::{card, class, grid, placeholder, sample_canvas};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiBadge, UiButton, UiFlexRow, UiGrid, UiGridCell, UiGridLength, UiLabel, UiResponsiveGrid,
    UiResponsiveRow, UiTextInput, UiVisibleResponsive,
};

/// StackPanel/Flex, Grid, and Canvas/Absolute layout component examples.
///
/// Corresponds to Fluent UI's Stack, Grid layout primitives, and positioning.
pub fn spawn_layout_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    // ------------------------------------------------------------------
    // 1. Flex row (static)
    // ------------------------------------------------------------------
    let flex = card(commands, parent, "StackPanel / Flex");
    let row = commands
        .spawn_scene(bsn! {
            UiFlexRow
            ChildOf(flex)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("Auto"))
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("Stretch"))
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("Gap"))
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("Horizontal row"))
        ChildOf(flex)
    });

    // ------------------------------------------------------------------
    // 2. Responsive Row — collapses to column below "md" (640px)
    // ------------------------------------------------------------------
    let resp_row = card(commands, parent, "Responsive Row (collapses at md)");
    let collapsing = commands
        .spawn_scene(bsn! {
            template_value(UiResponsiveRow::new("md"))
            template_value(class("responsive.demo"))
            ChildOf(resp_row)
        })
        .id();
    // Children that will stack vertically on narrow windows
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Item A — responsive row"))
        template_value(class("gallery.swatch.blue"))
        ChildOf(collapsing)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Item B — wraps at md"))
        template_value(class("gallery.swatch.green"))
        ChildOf(collapsing)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Item C — collapses"))
        template_value(class("gallery.swatch.gold"))
        ChildOf(collapsing)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Resize the window narrower to see these items stack vertically."))
        template_value(class("gallery.note"))
        ChildOf(collapsing)
    });

    // ------------------------------------------------------------------
    // 3. Responsive Visibility — show/hide at breakpoints
    // ------------------------------------------------------------------
    let visibility = card(commands, parent, "Responsive Visibility");

    // Always visible label
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Always visible on all screens"))
        ChildOf(visibility)
    });

    // Only visible on md and larger (≥640px)
    let show_md_up = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::show_from("md"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("👁 Visible at md+ (≥640px)"))
        template_value(class("gallery.swatch.green"))
        ChildOf(show_md_up)
    });

    // Only visible below lg (<1024px)
    let show_below_lg = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::show_until("lg"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("👁 Hidden at lg+ (disappears ≥1024px)"))
        template_value(class("gallery.swatch.gold"))
        ChildOf(show_below_lg)
    });

    // Only visible on small screens (≥sm but <md = 480px–639px)
    let show_sm_only = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::range("sm", "md"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("👁 Small screens only (480–639px)"))
        template_value(class("gallery.swatch.blue"))
        ChildOf(show_sm_only)
    });

    // ------------------------------------------------------------------
    // 4. Grid (static with tracks)
    // ------------------------------------------------------------------
    let grid_card = card(commands, g, "Grid (static)");
    let layout_grid = commands
        .spawn_scene(bsn! {
            template_value(
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
                    .show_grid_lines(true)
            )
            ChildOf(grid_card)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Span 2 columns"))
        template_value(class("gallery.swatch.blue"))
        template_value(UiGridCell::new(0, 0).with_span(2, 1))
        ChildOf(layout_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Auto"))
        template_value(class("gallery.swatch.green"))
        template_value(UiGridCell::new(2, 0))
        ChildOf(layout_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Star"))
        template_value(class("gallery.swatch.gold"))
        template_value(UiGridCell::new(0, 1).with_span(3, 1))
        ChildOf(layout_grid)
    });

    // ------------------------------------------------------------------
    // 5. Responsive Grid — changes columns at breakpoints
    // ------------------------------------------------------------------
    let resp_grid_card = card(commands, g, "Responsive Grid (columns at breakpoints)");
    let resp_grid = commands
        .spawn_scene(bsn! {
            template_value(
                UiResponsiveGrid::new(
                    vec![
                        ("sm", 1), // <480px  → 1 column
                        ("md", 2), // 480-639 → 2 columns (note: sm is 480, md is 640)
                        ("lg", 4), // 640+    → 4 columns
                    ],
                    1,
                )
                .with_grid_lines(true)
            )
            template_value(class("responsive.demo"))
            ChildOf(resp_grid_card)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Cell 1"))
        template_value(class("gallery.swatch.blue"))
        template_value(UiGridCell::new(0, 0))
        ChildOf(resp_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Cell 2"))
        template_value(class("gallery.swatch.green"))
        template_value(UiGridCell::new(1, 0))
        ChildOf(resp_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Cell 3"))
        template_value(class("gallery.swatch.gold"))
        template_value(UiGridCell::new(2, 0))
        ChildOf(resp_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Cell 4"))
        template_value(class("gallery.swatch.pink"))
        template_value(UiGridCell::new(3, 0))
        ChildOf(resp_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Cell 5"))
        template_value(class("gallery.swatch.purple"))
        template_value(UiGridCell::new(4, 0))
        ChildOf(resp_grid)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Resize window: 1 col <480px, 2 cols ≥480, 4 cols ≥640"))
        template_value(class("gallery.note"))
        ChildOf(resp_grid)
    });

    // ------------------------------------------------------------------
    // 6. Canvas / Absolute
    // ------------------------------------------------------------------
    let canvas_panel = card(commands, g, "Canvas / Absolute");
    commands.spawn_scene(bsn! {
        template_value(sample_canvas())
        template_value(class("gallery.canvas"))
        ChildOf(canvas_panel)
    });
    placeholder(
        commands,
        canvas_panel,
        "Right/bottom attached canvas children",
        "UiCanvasPosition stores right/bottom intent, but the current projector only applies left/top offsets.",
    );

    commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Confetti Placeholder"))
            ChildOf(canvas_panel)
        })
        .id()
}

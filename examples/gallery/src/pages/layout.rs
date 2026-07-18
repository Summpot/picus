//! Layout control pages (one component per page).

use crate::helpers::{card, class, grid, note, sample_canvas};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use bevy_math::Vec2;
use picus::prelude::{
    ToastKind, UiBadge, UiButton, UiCanvasPosition, UiCard, UiCheckbox, UiDivider, UiExpander,
    UiFlexColumn, UiFlexRow, UiFormRow, UiGrid, UiGridCell, UiGridLength, UiGroupBox, UiLabel,
    UiListView, UiMultilineTextInput, UiResponsiveGrid, UiResponsiveRow, UiScrollView, UiSplitPane,
    UiSwitch, UiTabBar, UiTextInput, UiVisibleResponsive,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

pub fn spawn_stack_panel_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let flex = card(commands, g, "Horizontal stack (flex row)");
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
    note(
        commands,
        flex,
        "StackPanel maps to UiFlexRow / UiFlexColumn for single-axis layout.",
    );
}

pub fn spawn_grid_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let grid_card = card(commands, g, "Static tracks");
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
}

pub fn spawn_responsive_page(commands: &mut Commands, parent: Entity) {
    // Responsive row — collapses to column below "md" (640px)
    let resp_row = card(commands, parent, "Responsive row (collapses at md)");
    let collapsing = commands
        .spawn_scene(bsn! {
            template_value(UiResponsiveRow::new("md"))
            template_value(class("responsive.demo"))
            ChildOf(resp_row)
        })
        .id();
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

    // Responsive visibility
    let visibility = card(commands, parent, "Responsive visibility");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Always visible on all screens"))
        ChildOf(visibility)
    });
    let show_md_up = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::show_from("md"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Visible at md+ (≥640px)"))
        template_value(class("gallery.swatch.green"))
        ChildOf(show_md_up)
    });
    let show_below_lg = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::show_until("lg"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Hidden at lg+ (disappears ≥1024px)"))
        template_value(class("gallery.swatch.gold"))
        ChildOf(show_below_lg)
    });
    let show_sm_only = commands
        .spawn_scene(bsn! {
            template_value(UiVisibleResponsive::range("sm", "md"))
            ChildOf(visibility)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Small screens only (480–639px)"))
        template_value(class("gallery.swatch.blue"))
        ChildOf(show_sm_only)
    });

    // Responsive grid
    let resp_grid_card = card(commands, parent, "Responsive grid (columns at breakpoints)");
    let resp_grid = commands
        .spawn_scene(bsn! {
            template_value(
                UiResponsiveGrid::new(
                    vec![
                        ("sm", 1),
                        ("md", 2),
                        ("lg", 4),
                    ],
                    1,
                )
                .with_grid_lines(true)
            )
            template_value(class("responsive.demo"))
            ChildOf(resp_grid_card)
        })
        .id();
    for (i, (label, swatch)) in [
        ("Cell 1", "gallery.swatch.blue"),
        ("Cell 2", "gallery.swatch.green"),
        ("Cell 3", "gallery.swatch.gold"),
        ("Cell 4", "gallery.swatch.pink"),
        ("Cell 5", "gallery.swatch.purple"),
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(class(swatch))
            template_value(UiGridCell::new(i as u32, 0))
            ChildOf(resp_grid)
        });
    }
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Resize window: 1 col <480px, 2 cols ≥480, 4 cols ≥640"))
        template_value(class("gallery.note"))
        ChildOf(resp_grid)
    });
}

pub fn spawn_group_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let group_box = card(commands, g, "Nested group");
    let inner = commands
        .spawn_scene(bsn! {
            template_value(UiGroupBox::new("Nested group"))
            template_value(class("gallery.group_box"))
            ChildOf(group_box)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Labels and controls can be grouped."))
        ChildOf(inner)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Inside a group", true))
        ChildOf(inner)
    });
    note(
        commands,
        group_box,
        "UiGroupBox is a Picus-owned grouping helper; the gallery supplies local styling.",
    );
}

pub fn spawn_split_pane_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let split = card(commands, g, "Resizable split");
    let pane = commands
        .spawn_scene(bsn! {
            template_value(UiSplitPane::new(0.42))
            ChildOf(split)
        })
        .id();
    let left = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.split_panel"))
            ChildOf(pane)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Left panel"))
        ChildOf(left)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiListView::new(["One", "Two", "Three"]).with_selected(0)
        )
        ChildOf(left)
    });
    let right = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.split_panel"))
            ChildOf(pane)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Right panel"))
        ChildOf(right)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("Resizable split content"))
        ChildOf(right)
    });
}

pub fn spawn_tab_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let tabs = card(commands, g, "Tab bar");
    let tab_bar = commands
        .spawn_scene(bsn! {
            template_value(UiTabBar::new(["Details", "Settings", "Logs"]))
            ChildOf(tabs)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Details tab content"))
        ChildOf(tab_bar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Enable option", true))
        ChildOf(tab_bar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMultilineTextInput::new("Log line 1\nLog line 2"))
        ChildOf(tab_bar)
    });
    note(
        commands,
        tabs,
        "Each child of UiTabBar is the content for the corresponding tab index.",
    );
}

pub fn spawn_canvas_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let canvas_panel = card(commands, g, "Canvas drawing and absolute children");
    let canvas_size = (320.0, 200.0);
    let demo_canvas = commands
        .spawn_scene(bsn! {
            template_value(sample_canvas().with_size(canvas_size.0, canvas_size.1))
            template_value(class("gallery.canvas"))
            ChildOf(canvas_panel)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("right/bottom"))
        template_value(class("gallery.swatch.gold"))
        template_value(UiCanvasPosition::default().with_right(8.0).with_bottom(8.0))
        ChildOf(demo_canvas)
    });

    let confetti = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Confetti Placeholder"))
            ChildOf(canvas_panel)
        })
        .id();
    commands
        .entity(confetti)
        .insert(GalleryButtonAction::Toast {
            message: "Confetti placeholder: animated retained canvas is not public yet."
                .to_string(),
            kind: ToastKind::Warning,
            duration: 3.5,
        });
    note(
        commands,
        canvas_panel,
        "UiCanvasPosition anchors children against the canvas size (including right/bottom).",
    );
}

pub fn spawn_expander_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let basic = card(commands, g, "Collapsed by default");
    let exp = commands
        .spawn_scene(bsn! {
            template_value(UiExpander::new("More options"))
            ChildOf(basic)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Hidden until the header is expanded."))
        ChildOf(exp)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Enable advanced mode", false))
        ChildOf(exp)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(false).with_label("Verbose logging"))
        ChildOf(exp)
    });

    let open = card(commands, g, "Pre-expanded");
    let exp_open = commands
        .spawn_scene(bsn! {
            template_value(UiExpander::new("Details (open)").with_expanded())
            ChildOf(open)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new(
            "UiExpander toggles child visibility and emits UiExpanderChanged.",
        ))
        ChildOf(exp_open)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("Notes inside expander"))
        ChildOf(exp_open)
    });
    note(
        commands,
        open,
        "Maps to WinUI Expander; header stays visible while body content collapses.",
    );
}

pub fn spawn_divider_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let horizontal = card(commands, g, "Horizontal divider");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Section A"))
        ChildOf(horizontal)
    });
    commands.spawn_scene(bsn! {
        template_value(UiDivider::horizontal())
        ChildOf(horizontal)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Section B"))
        ChildOf(horizontal)
    });
    note(
        commands,
        horizontal,
        "UiDivider is the Picus rule control (closest WinUI analog: Border separators / AppBarSeparator).",
    );

    let vertical = card(commands, g, "Vertical divider in a row");
    let row = commands
        .spawn_scene(bsn! {
            UiFlexRow
            ChildOf(vertical)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Left"))
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiDivider::vertical())
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Right"))
        ChildOf(row)
    });
}

pub fn spawn_scroll_view_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let vertical = card(commands, g, "Vertical scroll");
    let scroll = commands
        .spawn_scene(bsn! {
            template_value(
                UiScrollView::new(Vec2::new(420.0, 180.0), Vec2::new(400.0, 640.0))
                    .with_vertical_scrollbar(true)
                    .with_horizontal_scrollbar(false)
            )
            template_value(class("gallery.content_scroll"))
            ChildOf(vertical)
        })
        .id();
    for i in 1..=16 {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(format!("Scrollable row {i}")))
            ChildOf(scroll)
        });
    }
    note(
        commands,
        vertical,
        "UiScrollView stores viewport/content extents and scroll offset (WinUI ScrollView / ScrollViewer).",
    );

    let both = card(commands, g, "Both axes");
    let scroll_xy = commands
        .spawn_scene(bsn! {
            template_value(
                UiScrollView::new(Vec2::new(360.0, 140.0), Vec2::new(720.0, 320.0))
                    .with_vertical_scrollbar(true)
                    .with_horizontal_scrollbar(true)
            )
            template_value(class("gallery.content_scroll"))
            ChildOf(both)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new(
            "Wide content that exceeds the viewport on both axes — pan with wheel or scrollbars.",
        ))
        ChildOf(scroll_xy)
    });
    for i in 1..=6 {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(format!(
                "Row {i}: Lorem ipsum dolor sit amet, consectetur adipiscing elit — extra width sample."
            )))
            ChildOf(scroll_xy)
        });
    }
}

pub fn spawn_form_row_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let form = card(commands, g, "Labeled fields");
    let name = commands
        .spawn_scene(bsn! {
            template_value(UiFormRow::new("Display name").with_label_width(120.0))
            ChildOf(form)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("").with_placeholder("Ada Lovelace"))
        ChildOf(name)
    });

    let email = commands
        .spawn_scene(bsn! {
            template_value(UiFormRow::new("Email").with_label_width(120.0))
            ChildOf(form)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("").with_placeholder("ada@example.com"))
        ChildOf(email)
    });

    let notify = commands
        .spawn_scene(bsn! {
            template_value(UiFormRow::new("Notifications").with_label_width(120.0))
            ChildOf(form)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Email digests"))
        ChildOf(notify)
    });
    note(
        commands,
        form,
        "UiFormRow projects a caption column plus child controls — useful for settings forms.",
    );
}

pub fn spawn_card_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let simple = card(commands, g, "UiCard surface");
    let ui_card = commands
        .spawn_scene(bsn! {
            UiCard
            ChildOf(simple)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Title inside UiCard"))
        template_value(class("gallery.card_title"))
        ChildOf(ui_card)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new(
            "UiCard is a Fluent content surface. Gallery example cards also use a local gallery.card class.",
        ))
        template_value(class("gallery.note"))
        ChildOf(ui_card)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Card action"))
        ChildOf(ui_card)
    });

    let nested = card(commands, g, "Nested controls");
    let ui_card2 = commands
        .spawn_scene(bsn! {
            UiCard
            ChildOf(nested)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Remember this surface", true))
        ChildOf(ui_card2)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("Card"))
        ChildOf(ui_card2)
    });
    note(
        commands,
        nested,
        "Prefer UiCard for product content chrome; the gallery.card helper is a local documentation skin.",
    );
}

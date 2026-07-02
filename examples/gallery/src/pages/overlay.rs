//! Dialog, Toast, and anchored overlay component examples.
//!
//! Corresponds to Fluent UI's Dialog, Toast, and Popover overlay components.

use crate::helpers::{card, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{HasTooltip, UiButton, UiColorPicker, UiComboBox, UiComboOption, UiDatePicker};

/// Dialog, Toast, and anchored overlay component examples.
///
/// Demonstrates modal dialogs, toast notifications, and anchored overlays
/// (combo box dropdowns, color picker popups, date picker calendars, tooltips).
pub fn spawn_overlay_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let dialogs = card(commands, g, "Dialog");
    commands.spawn((UiButton::new("Open Dialog"), ChildOf(dialogs)));
    note(
        commands,
        dialogs,
        "Modal dialog overlays are available through UiDialog.",
    );

    let toast = card(commands, g, "Toasts");
    commands.spawn((UiButton::new("Info Toast"), ChildOf(toast)));
    commands.spawn((UiButton::new("Success Toast"), ChildOf(toast)));
    commands.spawn((UiButton::new("Warning Toast"), ChildOf(toast)));
    commands.spawn((UiButton::new("Error Toast"), ChildOf(toast)));

    let anchored = card(commands, g, "Anchored overlays");
    commands.spawn((
        UiComboBox::new(vec![
            UiComboOption::new("top", "Top"),
            UiComboOption::new("bottom", "Bottom"),
            UiComboOption::new("start", "Start"),
        ])
        .with_placeholder("Combo overlay"),
        ChildOf(anchored),
    ));
    commands.spawn((UiColorPicker::new(0xE5, 0x48, 0x4D), ChildOf(anchored)));
    commands.spawn((UiDatePicker::new(2026, 5, 24), ChildOf(anchored)));
    commands.spawn((
        UiButton::new("Tooltip source"),
        HasTooltip::new("Tooltip overlay follows its source entity."),
        ChildOf(anchored),
    ));

    placeholder(
        commands,
        g,
        "Manual overlay positioning",
        "OverlayPlacement supports anchored and viewport placements; arbitrary manual pixel placement is not a public component.",
    );

    parent
}

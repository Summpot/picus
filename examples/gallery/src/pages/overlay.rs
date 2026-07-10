//! Dialog, Toast, and anchored overlay component examples.
//!
//! Corresponds to Fluent UI's Dialog, Toast, and Popover overlay components.

use crate::helpers::{card, dialog_button, grid, note, toast_button};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    HasTooltip, ToastKind, UiButton, UiColorPicker, UiComboBox, UiComboOption, UiDatePicker,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Dialog, Toast, and anchored overlay component examples.
///
/// Demonstrates modal dialogs, toast notifications, and anchored overlays
/// (combo box dropdowns, color picker popups, date picker calendars, tooltips).
pub fn spawn_overlay_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let dialogs = card(commands, g, "Dialog");
    dialog_button(
        commands,
        dialogs,
        "Open Dialog",
        "Overlay Dialog",
        "Modal dialogs are available through UiDialog, spawned from the Overlay page.",
    );
    note(
        commands,
        dialogs,
        "Modal dialog overlays are available through UiDialog.",
    );

    let toast = card(commands, g, "Toasts");
    toast_button(
        commands,
        toast,
        "Info Toast",
        "Info toast from the Overlay page.",
        ToastKind::Info,
        2.4,
    );
    toast_button(
        commands,
        toast,
        "Success Toast",
        "Success toast from the Overlay page.",
        ToastKind::Success,
        2.4,
    );
    toast_button(
        commands,
        toast,
        "Warning Toast",
        "Warning toast from the Overlay page.",
        ToastKind::Warning,
        3.2,
    );
    toast_button(
        commands,
        toast,
        "Error Toast",
        "Error toast from the Overlay page.",
        ToastKind::Error,
        3.2,
    );

    let anchored = card(commands, g, "Anchored overlays");
    commands.spawn_scene(bsn! {
        template_value(
            UiComboBox::new(vec![
                UiComboOption::new("top", "Top"),
                UiComboOption::new("bottom", "Bottom"),
                UiComboOption::new("start", "Start"),
            ])
            .with_placeholder("Combo overlay")
        )
        ChildOf(anchored)
    });
    commands.spawn_scene(bsn! {
        template_value(UiColorPicker::new(0xE5, 0x48, 0x4D))
        ChildOf(anchored)
    });
    commands.spawn_scene(bsn! {
        template_value(UiDatePicker::new(2026, 5, 24))
        ChildOf(anchored)
    });
    let tooltip_src = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Tooltip source"))
            template_value(HasTooltip::new("Tooltip overlay follows its source entity."))
            ChildOf(anchored)
        })
        .id();
    commands
        .entity(tooltip_src)
        .insert(GalleryButtonAction::Info {
            message: "Overlay: Tooltip source clicked (hover for tooltip).".to_string(),
        });

    // Manual overlay positioning: a button that opens a popover at a fixed pixel
    // location using spawn_manual_overlay_at.
    let manual_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Open manual popover"))
            ChildOf(anchored)
        })
        .id();
    commands.entity(manual_btn).insert(ManualOverlayMarker);

    note(
        commands,
        g,
        "Manual overlay positioning uses spawn_manual_overlay_at to place a floating panel at an explicit (x, y) pixel coordinate.",
    );

    parent
}

/// Marker: clicking this entity opens a manually-positioned popover overlay.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ManualOverlayMarker;

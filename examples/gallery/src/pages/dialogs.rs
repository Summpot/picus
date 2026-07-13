//! Dialog and flyout control pages (one component per page).

use crate::helpers::{card, dialog_button, grid, note, toast_button};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    ToastKind, UiButton, UiContextMenuItem, UiContextMenuTrigger,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Marker: clicking this entity opens a manually-positioned popover overlay.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ManualOverlayMarker;

pub fn spawn_dialog_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let kinds = card(commands, g, "Dialog kinds");
    dialog_button(
        commands,
        kinds,
        "Info Dialog",
        "Info",
        "This is an informational dialog spawned from the Dialog page.",
    );
    dialog_button(
        commands,
        kinds,
        "Warning Dialog",
        "Warning",
        "This is a warning dialog spawned from the Dialog page.",
    );
    dialog_button(
        commands,
        kinds,
        "Error Dialog",
        "Error",
        "This is an error dialog spawned from the Dialog page.",
    );
    note(
        commands,
        kinds,
        "UiDialog provides modal dialogs with title, body, and dismiss actions.",
    );

    let prompt = card(commands, g, "Prompt placeholder");
    dialog_button(
        commands,
        prompt,
        "Prompt Placeholder",
        "Prompt Placeholder",
        "Picus UiDialog does not yet expose an input slot, so the prompt sample is represented here.",
    );

    let native = card(commands, g, "Native message hook");
    dialog_button(
        commands,
        native,
        "Native Message Hook",
        "Native Hook Placeholder",
        "Platform-native message hooks are not part of the public Picus runtime API.",
    );
}

pub fn spawn_toast_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let kinds = card(commands, g, "Toast kinds");
    toast_button(
        commands,
        kinds,
        "Info Toast",
        "Info toast from the Toast page.",
        ToastKind::Info,
        2.4,
    );
    toast_button(
        commands,
        kinds,
        "Success Toast",
        "Success toast from the Toast page.",
        ToastKind::Success,
        2.4,
    );
    toast_button(
        commands,
        kinds,
        "Warning Toast",
        "Warning toast from the Toast page.",
        ToastKind::Warning,
        3.2,
    );
    toast_button(
        commands,
        kinds,
        "Error Toast",
        "Error toast from the Toast page.",
        ToastKind::Error,
        3.2,
    );

    let persistent = card(commands, g, "Persistent toast");
    toast_button(
        commands,
        persistent,
        "Show persistent toast",
        "Persistent info toast. Close it manually.",
        ToastKind::Info,
        0.0,
    );
    note(
        commands,
        persistent,
        "A duration of 0.0 keeps the toast until the user dismisses it.",
    );
}

pub fn spawn_context_menu_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let menu = card(commands, g, "Right-click context menu");
    let ctx_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Right-click for context menu"))
            ChildOf(menu)
        })
        .id();
    commands.entity(ctx_btn).insert(UiContextMenuTrigger::new([
        UiContextMenuItem::new("Cut"),
        UiContextMenuItem::new("Copy"),
        UiContextMenuItem::new("Paste"),
        UiContextMenuItem::new("Separator").with_separator(),
        UiContextMenuItem::new("Select All"),
    ]));
    note(
        commands,
        menu,
        "UiContextMenuTrigger attaches a right-click command list to a control.",
    );
}

pub fn spawn_popover_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let dialog = card(commands, g, "Dialog as popover content");
    dialog_button(
        commands,
        dialog,
        "Open popover dialog",
        "Popover Note",
        "Anchored overlays are implemented by combo boxes, menus, color pickers, date pickers, and tooltips.",
    );
    note(
        commands,
        dialog,
        "Picus popovers are used by combo boxes, menus, pickers, and tooltips.",
    );

    let manual = card(commands, g, "Manual pixel placement");
    let manual_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Open manual popover"))
            ChildOf(manual)
        })
        .id();
    commands.entity(manual_btn).insert(ManualOverlayMarker);
    note(
        commands,
        manual,
        "spawn_manual_overlay_at places a floating panel at an explicit (x, y) pixel coordinate.",
    );
}

//! Dialog and flyout control pages (one component per page).

use crate::helpers::{card, dialog_button, dialog_button_with_dismiss, grid, note, toast_button};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    OverlayPlacement, ToastKind, UiButton, UiContextMenuItem, UiContextMenuTrigger,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

/// Marker: clicking this entity opens an anchored flyout via [`spawn_popover_in_overlay_root`].
///
/// WinUI Flyout ≈ Picus [`UiPopover`] + `spawn_popover_in_overlay_root`.
#[derive(Component, Debug, Clone, Copy)]
pub struct AnchoredFlyoutMarker {
    pub placement: OverlayPlacement,
}

/// Marker: clicking this entity opens a WinUI Popup-style panel at an explicit
/// window-relative `(x, y)` via [`spawn_manual_overlay_at`].
#[derive(Component, Debug, Clone, Copy)]
pub struct ManualOverlayMarkerAt {
    pub x: f64,
    pub y: f64,
}

pub fn spawn_dialog_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    // Sample messages only change title/body copy — UiDialog has no severity kind.
    let samples = card(commands, g, "Sample messages");
    dialog_button(
        commands,
        samples,
        "Info sample",
        "Info",
        "Informational ContentDialog-style overlay (WinUI ContentDialog → UiDialog).",
    );
    dialog_button(
        commands,
        samples,
        "Warning sample",
        "Warning",
        "Warning-themed copy only — UiDialog does not yet have severity kinds like ToastKind.",
    );
    dialog_button(
        commands,
        samples,
        "Error sample",
        "Error",
        "Error-themed copy only — visual severity styling is not applied by UiDialog today.",
    );
    note(
        commands,
        samples,
        "WinUI ContentDialog → Picus UiDialog (modal overlay with title, body, and a dismiss action). Sample titles are copy-only; there is no dialog kind/severity API yet. Gallery dialogs use with_fixed_width(460.0).",
    );

    let actions = card(commands, g, "Dismiss labels (primary action slot)");
    dialog_button_with_dismiss(
        commands,
        actions,
        "OK dialog",
        "Confirm",
        "UiDialog exposes a single dismiss button today. Map WinUI PrimaryButtonText to dismiss_label (here: OK).",
        "OK",
    );
    dialog_button_with_dismiss(
        commands,
        actions,
        "Got it dialog",
        "Notice",
        "Custom dismiss label demonstrates the ContentDialog primary-action affordance with the current API.",
        "Got it",
    );
    dialog_button_with_dismiss(
        commands,
        actions,
        "Cancel dialog",
        "Cancel sample",
        "Secondary-style dismiss label. Dual primary/secondary buttons are planned (Phase 3c structured actions).",
        "Cancel",
    );
    note(
        commands,
        actions,
        "UiDialog.dismiss_label is the close/primary button text. Structured Primary + Secondary buttons are not on the public API yet.",
    );

    let content = card(commands, g, "Content slot notes");
    dialog_button(
        commands,
        content,
        "Prompt placeholder",
        "Prompt Placeholder",
        "Picus UiDialog does not yet expose a free-form content slot or input field; body text is the structured content surface today.",
    );
    dialog_button(
        commands,
        content,
        "Native message hook",
        "Native Hook Placeholder",
        "Platform-native message hooks are not part of the public Picus runtime API; use UiDialog for in-app modals.",
    );
    note(
        commands,
        content,
        "ContentDialog content/checkbox/command slots map to future expand work; gallery shows title+body+dismiss only. Use UiDialog::with_fixed_width / with_fixed_height / with_fixed_size for layout hints.",
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
    let g = grid(commands, parent, 2);

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
        "WinUI ContextFlyout / right-click menu → UiContextMenuTrigger + UiContextMenuItem (opens on right-click).",
    );

    let icons = card(commands, g, "Items with disabled + separator");
    let ctx_btn2 = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Right-click advanced items"))
            ChildOf(icons)
        })
        .id();
    commands.entity(ctx_btn2).insert(UiContextMenuTrigger::new([
        UiContextMenuItem::new("Open"),
        UiContextMenuItem::new("Open with…").disabled(),
        UiContextMenuItem::new("sep").with_separator(),
        UiContextMenuItem::new("Delete"),
    ]));
    note(
        commands,
        icons,
        "UiContextMenuItem supports disabled rows and separator_after. For left-click MenuFlyout, see the MenuFlyout page.",
    );
}

/// Popover page also covers WinUI Flyout and Popup composition samples.
pub fn spawn_popover_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let flyout = card(commands, g, "Anchored Flyout (UiPopover)");
    for (label, placement) in [
        ("BottomStart flyout", OverlayPlacement::BottomStart),
        ("TopStart flyout", OverlayPlacement::TopStart),
        ("RightStart flyout", OverlayPlacement::RightStart),
    ] {
        let btn = commands
            .spawn_scene(bsn! {
                template_value(UiButton::new(label))
                ChildOf(flyout)
            })
            .id();
        commands
            .entity(btn)
            .insert(AnchoredFlyoutMarker { placement });
    }
    note(
        commands,
        flyout,
        "WinUI Flyout ≈ Picus UiPopover + spawn_popover_in_overlay_root(anchor, placement). Light-dismiss, non-modal.",
    );

    let popup = card(commands, g, "Popup (manual pixel placement)");
    for (label, x, y) in [
        ("Open Popup at (120, 80)", 120.0, 80.0),
        ("Open Popup at (420, 160)", 420.0, 160.0),
    ] {
        let btn = commands
            .spawn_scene(bsn! {
                template_value(UiButton::new(label))
                ChildOf(popup)
            })
            .id();
        commands.entity(btn).insert(ManualOverlayMarkerAt { x, y });
    }
    note(
        commands,
        popup,
        "WinUI Popup → compose with spawn_manual_overlay_at(world, bundle, x, y) for explicit window-relative coordinates. Popup demos use a light panel (not UiDialog).",
    );

    let map = card(commands, g, "WinUI name map");
    note(
        commands,
        map,
        "Flyout → UiPopover / spawn_popover_in_overlay_root. Popup → spawn_manual_overlay_at (light panel). Modal ContentDialog → UiDialog + spawn_in_overlay_root. MenuFlyout → see MenuFlyout page.",
    );
}

use crate::helpers::{card, class, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{UiButton, UiCheckbox, UiProgressBar, UiSlider, UiSwitch};

/// Button, Switch, Checkbox, ProgressBar, and Slider component examples.
///
/// Corresponds to Fluent UI's Button, Toggle, Checkbox, ProgressBar, and Slider components.
pub fn spawn_buttons_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let buttons = card(commands, g, "Buttons");
    commands.spawn((UiButton::new("Default"), ChildOf(buttons)));
    commands.spawn((
        UiButton::new("Accent"),
        class("gallery.accent_button"),
        ChildOf(buttons),
    ));
    commands.spawn((
        UiButton::new("Flat"),
        class("gallery.flat_button"),
        ChildOf(buttons),
    ));
    commands.spawn((
        UiButton::new("Danger"),
        class("gallery.danger_button"),
        ChildOf(buttons),
    ));
    let open_dialog_btn = commands
        .spawn((UiButton::new("Open Dialog"), ChildOf(buttons)))
        .id();
    note(
        commands,
        buttons,
        "Double-click and disabled button states from MewUI are placeholders below.",
    );

    let toggles = card(commands, g, "Toggle / Switch");
    commands.spawn((
        UiSwitch::new(true).with_label("Streaming"),
        ChildOf(toggles),
    ));
    commands.spawn((
        UiSwitch::new(false).with_label("Notifications"),
        ChildOf(toggles),
    ));
    commands.spawn((
        UiCheckbox::new("ToggleButton-style checkbox", true),
        ChildOf(toggles),
    ));
    commands.spawn((
        UiCheckbox::new("Unchecked toggle state", false),
        ChildOf(toggles),
    ));

    let progress = card(commands, g, "Progress");
    commands.spawn((
        UiProgressBar::determinate(0.20),
        class("gallery.progress"),
        ChildOf(progress),
    ));
    commands.spawn((
        UiProgressBar::determinate(0.65),
        class("gallery.progress"),
        ChildOf(progress),
    ));
    commands.spawn((
        UiProgressBar::indeterminate(),
        class("gallery.progress"),
        ChildOf(progress),
    ));
    commands.spawn((
        UiSlider::new(0.0, 100.0, 25.0).with_step(5.0),
        ChildOf(progress),
    ));

    placeholder(
        commands,
        g,
        "Disabled / double-click button states",
        "Picus UiButton currently exposes click events but not disabled state or double-click action routing.",
    );

    open_dialog_btn
}

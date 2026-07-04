use crate::helpers::{card, class, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiButton, UiCheckbox, UiProgressBar, UiSlider, UiSwitch,
};

/// Button, Switch, Checkbox, ProgressBar, and Slider component examples.
///
/// Corresponds to Fluent UI's Button, Toggle, Checkbox, ProgressBar, and Slider components.
pub fn spawn_buttons_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let buttons = card(commands, g, "Buttons");
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Default"))
        ChildOf(buttons)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Accent"))
        template_value(class("gallery.accent_button"))
        ChildOf(buttons)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Flat"))
        template_value(class("gallery.flat_button"))
        ChildOf(buttons)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Danger"))
        template_value(class("gallery.danger_button"))
        ChildOf(buttons)
    });
    let open_dialog_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Open Dialog"))
            ChildOf(buttons)
        })
        .id();
    note(
        commands,
        buttons,
        "Double-click and disabled button states from MewUI are placeholders below.",
    );

    let toggles = card(commands, g, "Toggle / Switch");
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Streaming"))
        ChildOf(toggles)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(false).with_label("Notifications"))
        ChildOf(toggles)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("ToggleButton-style checkbox", true))
        ChildOf(toggles)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Unchecked toggle state", false))
        ChildOf(toggles)
    });

    let progress = card(commands, g, "Progress");
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::determinate(0.20))
        template_value(class("gallery.progress"))
        ChildOf(progress)
    });
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::determinate(0.65))
        template_value(class("gallery.progress"))
        ChildOf(progress)
    });
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::indeterminate())
        template_value(class("gallery.progress"))
        ChildOf(progress)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSlider::new(0.0, 100.0, 25.0).with_step(5.0))
        ChildOf(progress)
    });

    placeholder(
        commands,
        g,
        "Disabled / double-click button states",
        "Picus UiButton currently exposes click events but not disabled state or double-click action routing.",
    );

    open_dialog_btn
}

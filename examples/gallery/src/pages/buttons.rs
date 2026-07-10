use crate::helpers::{card, class, grid, info_button, note, placeholder};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    UiButton, UiCheckbox, UiProgressBar, UiSlider, UiSwitch,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Button, Switch, Checkbox, ProgressBar, and Slider component examples.
///
/// Corresponds to Fluent UI's Button, Toggle, Checkbox, ProgressBar, and Slider components.
pub fn spawn_buttons_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let buttons = card(commands, g, "Buttons");
    info_button(commands, buttons, "Default", "Buttons: Default clicked.");
    let accent = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Accent"))
            template_value(class("gallery.accent_button"))
            ChildOf(buttons)
        })
        .id();
    commands.entity(accent).insert(GalleryButtonAction::Info {
        message: "Buttons: Accent clicked.".to_string(),
    });
    let flat = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Flat"))
            template_value(class("gallery.flat_button"))
            ChildOf(buttons)
        })
        .id();
    commands.entity(flat).insert(GalleryButtonAction::Info {
        message: "Buttons: Flat clicked.".to_string(),
    });
    let danger = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Danger"))
            template_value(class("gallery.danger_button"))
            ChildOf(buttons)
        })
        .id();
    commands.entity(danger).insert(GalleryButtonAction::Info {
        message: "Buttons: Danger clicked.".to_string(),
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
        "Double-click and disabled button states are placeholders below.",
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
        "Double-click button state",
        "Picus UiButton exposes single-click actions; double-click detection is not a built-in button contract yet.",
    );

    // Disabled button state — supported by UiButton.disabled.
    let disabled_card = card(commands, g, "Disabled button");
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Disabled default").disabled(true))
        ChildOf(disabled_card)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Disabled accent").disabled(true))
        template_value(class("gallery.accent_button"))
        ChildOf(disabled_card)
    });

    open_dialog_btn
}

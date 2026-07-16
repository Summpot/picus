//! Basic input control pages (one component per page).

use crate::helpers::{card, class, grid, info_button, note, placeholder};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    UiButton, UiCheckbox, UiColorPicker, UiComboBox, UiComboOption, UiDatePicker, UiNumericUpDown,
    UiRadioGroup, UiSlider, UiSwitch,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

pub fn spawn_button_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let standard = card(commands, g, "Standard buttons");
    info_button(commands, standard, "Default", "Button: Default clicked.");
    let accent = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Accent"))
            template_value(class("gallery.accent_button"))
            ChildOf(standard)
        })
        .id();
    commands.entity(accent).insert(GalleryButtonAction::Info {
        message: "Button: Accent clicked.".to_string(),
    });
    let flat = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Flat"))
            template_value(class("gallery.flat_button"))
            ChildOf(standard)
        })
        .id();
    commands.entity(flat).insert(GalleryButtonAction::Info {
        message: "Button: Flat clicked.".to_string(),
    });
    let danger = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Danger"))
            template_value(class("gallery.danger_button"))
            ChildOf(standard)
        })
        .id();
    commands.entity(danger).insert(GalleryButtonAction::Info {
        message: "Button: Danger clicked.".to_string(),
    });

    let dialog = card(commands, g, "Open a dialog");
    commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Open Dialog"))
            ChildOf(dialog)
        })
        .insert(GalleryButtonAction::Dialog {
            title: "Button Dialog".to_string(),
            body: "Demonstrates Picus UiDialog for message boxes.".to_string(),
        });
    note(
        commands,
        dialog,
        "Buttons can open dialogs or fire other ECS actions through GalleryButtonAction.",
    );

    let disabled = card(commands, g, "Disabled");
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Disabled default").disabled(true))
        ChildOf(disabled)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Disabled accent").disabled(true))
        template_value(class("gallery.accent_button"))
        ChildOf(disabled)
    });

    placeholder(
        commands,
        g,
        "Double-click button state",
        "Picus UiButton exposes single-click actions; double-click detection is not a built-in button contract yet.",
    );
}

pub fn spawn_toggle_switch_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Toggle switches");
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Streaming"))
        ChildOf(basic)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(false).with_label("Notifications"))
        ChildOf(basic)
    });
    note(
        commands,
        basic,
        "UiSwitch represents a binary on/off setting with an optional label.",
    );

    let animated = card(commands, g, "Animated target");
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Animated switch target"))
        ChildOf(animated)
    });
    note(
        commands,
        animated,
        "Toggle switches participate in theme color transitions when the active style variant changes.",
    );
}

pub fn spawn_checkbox_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Check boxes");
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("CheckBox", false))
        ChildOf(basic)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Checked", true))
        ChildOf(basic)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiCheckbox::new("Indeterminate", false)
                .indeterminate(true)
                .three_state(true)
        )
        ChildOf(basic)
    });
    note(
        commands,
        basic,
        "Tri-state checkboxes cycle unchecked → checked → indeterminate → unchecked on click.",
    );

    let toggle_style = card(commands, g, "Toggle-style labels");
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("ToggleButton-style checkbox", true))
        ChildOf(toggle_style)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Unchecked toggle state", false))
        ChildOf(toggle_style)
    });
}

pub fn spawn_radio_button_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Radio group");
    commands.spawn_scene(bsn! {
        template_value(
            UiRadioGroup::new(["Apple", "Banana", "Cherry", "Long long option"]).with_selected(1)
        )
        ChildOf(basic)
    });
    note(
        commands,
        basic,
        "UiRadioGroup presents mutually exclusive options; only one item can be selected.",
    );

    let backdrop_like = card(commands, g, "Material-style options");
    commands.spawn_scene(bsn! {
        template_value(UiRadioGroup::new(["None", "Mica", "Acrylic"]).with_selected(1))
        ChildOf(backdrop_like)
    });
    note(
        commands,
        backdrop_like,
        "See the WindowBackdrop page for a radio group that drives the native material.",
    );
}

pub fn spawn_slider_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Basic slider");
    commands.spawn_scene(bsn! {
        template_value(UiSlider::new(0.0, 100.0, 25.0).with_step(5.0))
        ChildOf(basic)
    });
    note(commands, basic, "Step of 5 over the range 0–100.");

    let fine = card(commands, g, "Fine step");
    commands.spawn_scene(bsn! {
        template_value(UiSlider::new(0.0, 100.0, 42.5).with_step(0.5))
        ChildOf(fine)
    });
    note(commands, fine, "Half-unit step for finer control.");
}

pub fn spawn_combo_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let language = card(commands, g, "Language combo");
    let mut combo = UiComboBox::new(vec![
        UiComboOption::new("rust", "Rust"),
        UiComboOption::new("csharp", "C#"),
        UiComboOption::new("swift", "Swift"),
        UiComboOption::new("kotlin", "Kotlin"),
    ])
    .with_placeholder("Pick a language");
    combo.selected = 0;
    commands.spawn_scene(bsn! {
        template_value(combo)
        ChildOf(language)
    });

    let size = card(commands, g, "Size combo");
    commands.spawn_scene(bsn! {
        template_value(
            UiComboBox::new(vec![
                UiComboOption::new("small", "Small"),
                UiComboOption::new("medium", "Medium"),
                UiComboOption::new("large", "Large"),
            ])
            .with_placeholder("Size")
        )
        ChildOf(size)
    });
    note(
        commands,
        size,
        "Combo boxes open an anchored overlay panel for their option list.",
    );
}

pub fn spawn_color_picker_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let rgb = card(commands, g, "RGB color");
    commands.spawn_scene(bsn! {
        template_value(UiColorPicker::new(0x60, 0xA5, 0xFA))
        ChildOf(rgb)
    });

    let rgba = card(commands, g, "RGBA color");
    commands.spawn_scene(bsn! {
        template_value(UiColorPicker::new_rgba(0xE5, 0x48, 0x4D, 0xCC))
        ChildOf(rgba)
    });
    note(
        commands,
        rgba,
        "Color pickers open an anchored panel for hue, saturation, and alpha editing.",
    );
}

pub fn spawn_date_picker_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Date picker");
    commands.spawn_scene(bsn! {
        template_value(UiDatePicker::new(2026, 5, 24))
        ChildOf(basic)
    });
    note(
        commands,
        basic,
        "UiDatePicker renders its month grid as an anchored overlay panel.",
    );

    let alt = card(commands, g, "Another date");
    commands.spawn_scene(bsn! {
        template_value(UiDatePicker::new(2024, 6, 15))
        ChildOf(alt)
    });
    placeholder(
        commands,
        alt,
        "Always-visible calendar",
        "UiDatePicker renders its month grid only as an anchored overlay panel.",
    );
}

pub fn spawn_number_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let pixels = card(commands, g, "Integer with suffix");
    commands.spawn_scene(bsn! {
        template_value(
            UiNumericUpDown::new(0.0, 100.0, 25.0)
                .with_step(5.0)
                .with_suffix(" px")
        )
        ChildOf(pixels)
    });

    let seconds = card(commands, g, "Decimal with precision");
    commands.spawn_scene(bsn! {
        template_value(
            UiNumericUpDown::new(0.0, 1.0, 0.30)
                .with_step(0.05)
                .with_precision(2)
                .with_suffix(" s")
        )
        ChildOf(seconds)
    });
    note(
        commands,
        seconds,
        "NumberBox maps to Picus UiNumericUpDown with step, precision, and suffix support.",
    );
}

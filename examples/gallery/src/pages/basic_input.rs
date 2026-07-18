//! Basic input control pages (one component per page).

use crate::helpers::{card, class, grid, info_button, note, placeholder};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    ButtonAppearance, ButtonSize, FluentIcon, RatingColor, RatingSize, UiButton, UiCheckbox,
    UiColorPicker, UiComboBox, UiComboOption, UiDatePicker, UiLabel, UiLink, UiNumericUpDown,
    UiRadioGroup, UiRating, UiSlider, UiSwitch, UiTimePicker,
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

    let appearances = card(commands, g, "Fluent appearances");
    for (label, appearance) in [
        ("Default", ButtonAppearance::Default),
        ("Primary", ButtonAppearance::Primary),
        ("Outline", ButtonAppearance::Outline),
        ("Subtle", ButtonAppearance::Subtle),
        ("Transparent", ButtonAppearance::Transparent),
    ] {
        let id = commands
            .spawn_scene(bsn! {
                template_value(UiButton::new(label).with_appearance(appearance))
                ChildOf(appearances)
            })
            .id();
        commands.entity(id).insert(GalleryButtonAction::Info {
            message: format!("Button appearance: {label}"),
        });
    }
    note(
        commands,
        appearances,
        "UiButton appearances map to Fluent UI v9 Button variants.",
    );

    let sized = card(commands, g, "Sizes and icon");
    commands.spawn_scene(bsn! {
        template_value(
            UiButton::new("Small")
                .with_size(ButtonSize::Small)
                .with_icon(FluentIcon::Add)
        )
        ChildOf(sized)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiButton::new("Large")
                .with_size(ButtonSize::Large)
                .with_icon(FluentIcon::Send)
        )
        ChildOf(sized)
    });

    placeholder(
        commands,
        g,
        "RepeatButton / SplitButton",
        "Picus does not yet ship dedicated repeat or split-button components; use UiButton + menu overlays for similar UX.",
    );
}

pub fn spawn_hyperlink_button_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let links = card(commands, g, "Hyperlink text");
    commands.spawn_scene(bsn! {
        template_value(UiLink::new("Open documentation"))
        ChildOf(links)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLink::new("https://example.com/gallery"))
        ChildOf(links)
    });
    note(
        commands,
        links,
        "UiLink is the Fluent-style hyperlink control; clicks emit UiLinkAction.",
    );

    let mixed = card(commands, g, "Inline with surrounding text");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Read more about"))
        ChildOf(mixed)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLink::new("component contracts"))
        ChildOf(mixed)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("in the Picus docs."))
        ChildOf(mixed)
    });
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

pub fn spawn_rating_control_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let basic = card(commands, g, "Standard rating");
    commands.spawn_scene(bsn! {
        template_value(UiRating::new(3.0))
        ChildOf(basic)
    });
    note(
        commands,
        basic,
        "UiRating emits UiRatingChanged when the user selects a new star value.",
    );

    let half = card(commands, g, "Half-star step");
    commands.spawn_scene(bsn! {
        template_value(UiRating::new(3.5).with_step(0.5).with_color(RatingColor::Marigold))
        ChildOf(half)
    });

    let sizes = card(commands, g, "Sizes");
    commands.spawn_scene(bsn! {
        template_value(UiRating::new(4.0).with_size(RatingSize::Small))
        ChildOf(sizes)
    });
    commands.spawn_scene(bsn! {
        template_value(UiRating::new(4.0).with_size(RatingSize::Large).with_color(RatingColor::Brand))
        ChildOf(sizes)
    });

    let max = card(commands, g, "Custom max");
    commands.spawn_scene(bsn! {
        template_value(UiRating::new(7.0).with_max(10))
        ChildOf(max)
    });
    note(commands, max, "with_max sets the number of stars (here 10).");
}

pub fn spawn_time_picker_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let twenty_four = card(commands, g, "24-hour mode");
    commands.spawn_scene(bsn! {
        template_value(UiTimePicker::new(14, 30, 0))
        ChildOf(twenty_four)
    });
    note(
        commands,
        twenty_four,
        "UiTimePicker opens an anchored overlay panel for hour/minute/second selection.",
    );

    let twelve = card(commands, g, "12-hour (AM/PM) mode");
    commands.spawn_scene(bsn! {
        template_value(UiTimePicker::new(9, 15, 0).with_12h())
        ChildOf(twelve)
    });

    let from_12h = card(commands, g, "Constructed from 12h parts");
    commands.spawn_scene(bsn! {
        template_value(UiTimePicker::from_12h(6, true, 45, 0))
        ChildOf(from_12h)
    });
    note(
        commands,
        from_12h,
        "from_12h(6, true, …) is 6:45 PM (18:45).",
    );
}

use crate::helpers::{card, grid, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    HasTooltip, UiButton, UiComboBox, UiComboOption, UiMultilineTextInput, UiPasswordInput,
    UiSlider, UiTextInput,
};

/// Text input, password, multiline text, combo box, and tooltip component examples.
///
/// Corresponds to Fluent UI's TextField, ComboBox, and Tooltip components.
pub fn spawn_inputs_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let text = card(commands, g, "TextBox");
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("").with_placeholder("Type your name..."))
        ChildOf(text)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("This is my name"))
        ChildOf(text)
    });
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("Read/write ECS text"))
        ChildOf(text)
    });

    let password = card(commands, g, "PasswordBox");
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("").with_placeholder("Password"))
        ChildOf(password)
    });
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("secret").with_mask('*'))
        ChildOf(password)
    });
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("disabled placeholder").read_only(true))
        ChildOf(password)
    });

    let multiline = card(commands, g, "MultiLineTextBox");
    commands.spawn_scene(bsn! {
        template_value(UiMultilineTextInput::new(
            "The quick brown fox jumps over the lazy dog.\n\n- Wrap supported\n- Selection is provided by Masonry text input\n- ECS value sync is enabled",
        ).with_placeholder("Notes"))
        ChildOf(multiline)
    });

    let combo = card(commands, g, "Combo / Numeric");
    let mut language = UiComboBox::new(vec![
        UiComboOption::new("rust", "Rust"),
        UiComboOption::new("csharp", "C#"),
        UiComboOption::new("swift", "Swift"),
        UiComboOption::new("kotlin", "Kotlin"),
    ])
    .with_placeholder("Pick a language");
    language.selected = 0;
    commands.spawn_scene(bsn! {
        template_value(language)
        ChildOf(combo)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSlider::new(0.0, 100.0, 42.5).with_step(0.5))
        ChildOf(combo)
    });
    placeholder(
        commands,
        combo,
        "NumericUpDown",
        "Picus has UiSlider but no spinner/text hybrid numeric-up-down control yet.",
    );

    let tooltip = card(commands, g, "ToolTip / Context");
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Hover for tooltip"))
        template_value(HasTooltip::new("Tooltip overlay anchored to this button."))
        ChildOf(tooltip)
    });
    placeholder(
        commands,
        tooltip,
        "Context menu",
        "Picus has menu-bar overlays, but no right-click ContextMenu component or key gesture model yet.",
    );

    let drag_drop = card(commands, g, "Drag and Drop");
    placeholder(
        commands,
        drag_drop,
        "Window drag/drop",
        "Picus input bridge does not expose platform file-drop IDataObject/XDND events to ECS yet.",
    );

    commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Show persistent toast"))
            ChildOf(text)
        })
        .id()
}

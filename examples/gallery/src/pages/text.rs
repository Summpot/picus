//! Text input control pages (one component per page).

use crate::helpers::{card, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    UiMultilineTextInput, UiPasswordInput, UiTextInput,
    scene::{CommandsSceneExt, bsn, template_value},
};

pub fn spawn_text_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let empty = card(commands, g, "Empty with placeholder");
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("").with_placeholder("Type your name..."))
        ChildOf(empty)
    });

    let filled = card(commands, g, "Pre-filled value");
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("This is my name"))
        ChildOf(filled)
    });

    let ecs = card(commands, g, "ECS-backed value");
    commands.spawn_scene(bsn! {
        template_value(UiTextInput::new("Read/write ECS text"))
        ChildOf(ecs)
    });
    note(
        commands,
        ecs,
        "Edits update the UiTextInput component and emit change events into UiEventQueue.",
    );
}

pub fn spawn_password_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let empty = card(commands, g, "Empty with placeholder");
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("").with_placeholder("Password"))
        ChildOf(empty)
    });

    let masked = card(commands, g, "Custom mask");
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("secret").with_mask('*'))
        ChildOf(masked)
    });

    let readonly = card(commands, g, "Read-only");
    commands.spawn_scene(bsn! {
        template_value(UiPasswordInput::new("disabled placeholder").read_only(true))
        ChildOf(readonly)
    });
    note(
        commands,
        readonly,
        "Password boxes obscure characters while still syncing the ECS value.",
    );
}

pub fn spawn_multiline_text_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let notes = card(commands, g, "Multi-line notes");
    commands.spawn_scene(bsn! {
        template_value(UiMultilineTextInput::new(
            "The quick brown fox jumps over the lazy dog.\n\n- Wrap supported\n- Selection is provided by Masonry text input\n- ECS value sync is enabled",
        ).with_placeholder("Notes"))
        ChildOf(notes)
    });

    let readonly = card(commands, g, "Read-only wrapping sample");
    commands.spawn_scene(bsn! {
        template_value(
            UiMultilineTextInput::new(
                "Covers font families, weight, wrapping, alignment, and editable text. Picus exposes most text through labels and text inputs today.",
            )
            .read_only(true)
        )
        ChildOf(readonly)
    });
}

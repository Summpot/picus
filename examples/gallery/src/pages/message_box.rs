use crate::helpers::{card, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiButton,
};

/// Dialog and message box component examples.
///
/// Corresponds to Fluent UI's Dialog and MessageBar components.
pub fn spawn_message_box_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let dialog = card(commands, g, "Dialog / MessageBox");
    let error_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Show Error Toast"))
            ChildOf(dialog)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Info Dialog"))
        ChildOf(dialog)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Warning Dialog"))
        ChildOf(dialog)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Error Dialog"))
        ChildOf(dialog)
    });
    note(
        commands,
        dialog,
        "UiDialog provides modal dialogs with title, body, and dismiss actions.",
    );

    let prompt = card(commands, g, "Prompt");
    placeholder(
        commands,
        prompt,
        "Prompt text dialog",
        "UiDialog has title/body/dismiss fields but no built-in input slot or confirm/cancel result contract.",
    );

    let native = card(commands, g, "Native Message Hook");
    placeholder(
        commands,
        native,
        "Native message hook",
        "Masonry runtime is abstracted behind Picus; platform-native message hooks are not public API.",
    );

    error_btn
}

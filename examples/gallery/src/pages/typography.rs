use crate::helpers::{card, class, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{UiLabel, UiMultilineTextInput};

/// Text scale, CJK/Unicode, and text wrapping component examples.
///
/// Corresponds to Fluent UI's Text component with typography scale and internationalization.
pub fn spawn_typography_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let scale = card(commands, g, "Text Scale");
    commands.spawn((
        UiLabel::new("Display / Hero"),
        class("gallery.typo.hero"),
        ChildOf(scale),
    ));
    commands.spawn((
        UiLabel::new("Title text"),
        class("gallery.typo.title"),
        ChildOf(scale),
    ));
    commands.spawn((
        UiLabel::new("Body text for gallery descriptions and form copy."),
        class("gallery.typo.body"),
        ChildOf(scale),
    ));
    commands.spawn((
        UiLabel::new("Caption / secondary metadata"),
        class("gallery.typo.caption"),
        ChildOf(scale),
    ));

    let cjk = card(commands, g, "CJK / Unicode");
    commands.spawn((
        UiLabel::new("Picus Gallery: 骨 / 骨 / こんにちは / 你好"),
        class("gallery.typo.title"),
        ChildOf(cjk),
    ));
    note(
        commands,
        cjk,
        "Fonts are bridged through Picus; this gallery registers the bundled Noto Sans font.",
    );

    let wrapping = card(commands, g, "Text wrapping");
    commands.spawn((
        UiMultilineTextInput::new(
            "MewUI TypographyPage covers families, weight, wrapping, alignment, and editable text. Picus exposes most text through labels and text inputs today.",
        ).read_only(true),
        ChildOf(wrapping),
    ));

    placeholder(
        commands,
        g,
        "Rich text runs",
        "UiLabel is plain text; mixed inline spans/weights/colors require a richer text component.",
    );

    parent
}

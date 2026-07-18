//! Text input control pages (one component per page).

use crate::helpers::{card, class, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    UiLabel, UiMultilineTextInput, UiPasswordInput, UiSearch, UiText, UiTextInput,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

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
        "Edits update the UiTextInput component and emit change events into UiAction messages.",
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

pub fn spawn_search_box_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let empty = card(commands, g, "Empty search box");
    commands.spawn_scene(bsn! {
        template_value(UiSearch::new("Search components…"))
        ChildOf(empty)
    });
    note(
        commands,
        empty,
        "UiSearch is the Picus counterpart of WinUI AutoSuggestBox for query entry (without a built-in suggestion list).",
    );

    let prefilled = card(commands, g, "Pre-filled query");
    let mut search = UiSearch::new("Filter list…");
    search.value = "Button".to_string();
    commands.spawn_scene(bsn! {
        template_value(search)
        ChildOf(prefilled)
    });
    note(
        commands,
        prefilled,
        "Value changes emit UiSearchChanged; the gallery shell also uses UiSearch for nav filtering.",
    );
}

pub fn spawn_text_block_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let plain = card(commands, g, "Plain text block");
    commands.spawn_scene(bsn! {
        template_value(UiText::new_text(
            "UiText is a static text element on the Fluent type ramp.",
        ))
        ChildOf(plain)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new(
            "UiLabel remains available for lightweight captions and notes.",
        ))
        ChildOf(plain)
    });

    let ramp = card(commands, g, "Typography classes");
    for (label, class_name) in [
        ("Display / hero sample", "gallery.typo.title"),
        ("Body copy for paragraphs and descriptions.", "gallery.page_description"),
        ("Caption / secondary meta text", "gallery.note"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(class(class_name))
            ChildOf(ramp)
        });
    }
    note(
        commands,
        ramp,
        "WinUI TextBlock maps to UiText / UiLabel; see the Typography page for the full type scale.",
    );
}

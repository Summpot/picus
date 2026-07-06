//! Fluent icon glyph grid component examples.
//!
//! Corresponds to Fluent UI's Icon component with a gallery of available glyphs.

use crate::helpers::{card, class, grid, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    FluentIcon, UiLabel,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Fluent icon glyph grid component examples.
///
/// Displays common Fluent Design glyphs from the Windows symbol font stack,
/// similar to Fluent UI's icon grid documentation page.
pub fn spawn_icons_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 4);

    for (name, icon) in [
        ("Accept", FluentIcon::Accept),
        ("Add", FluentIcon::Add),
        ("Cancel", FluentIcon::Cancel),
        ("Settings", FluentIcon::Settings),
        ("Search", FluentIcon::Search),
        ("Send", FluentIcon::Send),
        ("Refresh", FluentIcon::Refresh),
        ("Message", FluentIcon::Message),
    ] {
        let c = card(commands, g, name);
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(icon.glyph().to_string()))
            template_value(class("gallery.icon"))
            ChildOf(c)
        });
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(name))
            template_value(class("gallery.icon_label"))
            ChildOf(c)
        });
    }

    placeholder(
        commands,
        parent,
        "Full Fluent icon browser",
        "Picus exposes FluentIcon glyphs backed by Segoe Fluent Icons with MDL2/Fabric fallback.",
    );

    parent
}

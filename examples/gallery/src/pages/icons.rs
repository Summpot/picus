//! PicusIcon glyph grid component examples.
//!
//! Corresponds to Fluent UI's Icon component with a gallery of available glyphs.

use crate::helpers::{card, class, grid, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{PicusIcon, UiLabel};

/// PicusIcon glyph grid component examples.
///
/// Displays available icon glyphs from the bundled Lucide icon font,
/// similar to Fluent UI's icon grid documentation page.
pub fn spawn_icons_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 4);

    for (name, icon) in [
        ("Check", PicusIcon::Check),
        ("Chevron Down", PicusIcon::ChevronDown),
        ("Chevron Up", PicusIcon::ChevronUp),
        ("Chevron Right", PicusIcon::ChevronRight),
        ("Circle", PicusIcon::Circle),
        ("Circle Dot", PicusIcon::CircleDot),
        ("Close", PicusIcon::X),
        ("Theme", PicusIcon::SunMoon),
    ] {
        let c = card(commands, g, name);
        commands.spawn((
            UiLabel::new(icon.glyph().to_string()),
            class("gallery.icon"),
            ChildOf(c),
        ));
        commands.spawn((UiLabel::new(name), class("gallery.icon_label"), ChildOf(c)));
    }

    placeholder(
        commands,
        parent,
        "Full Icons.xaml browser",
        "Picus exposes a curated PicusIcon enum backed by Lucide font bytes; it does not parse MewUI Icons.xaml path resources.",
    );

    parent
}

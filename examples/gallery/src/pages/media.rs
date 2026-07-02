//! Image, Canvas media, and video/animated image component examples.
//!
//! Corresponds to Fluent UI's Image component with various fit modes and fallback states.

use crate::helpers::{card, class, generated_image, grid, note, placeholder, sample_canvas};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::UiImage;

/// Image, Canvas media, and video/animated image component examples.
///
/// Picus supports in-memory generated images and empty image placeholders.
/// Canvas drawing provides vector graphics as an alternative media surface.
pub fn spawn_media_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let generated = card(commands, g, "Image");
    commands.spawn((
        generated_image(),
        class("gallery.image"),
        ChildOf(generated),
    ));
    note(
        commands,
        generated,
        "The source image is generated in-memory so the example is self-contained.",
    );

    let empty = card(commands, g, "Image fallback");
    commands.spawn((
        UiImage::empty().with_alt_text("Image resource unavailable"),
        class("gallery.image"),
        ChildOf(empty),
    ));
    placeholder(
        commands,
        empty,
        "Remote image loading",
        "MewUI downloads sample resources at runtime; this example avoids cargo run/network dependency for gallery startup.",
    );

    let canvas = card(commands, g, "Canvas media");
    commands.spawn((sample_canvas(), class("gallery.canvas"), ChildOf(canvas)));

    placeholder(
        commands,
        g,
        "Video / animated image",
        "Picus has bitmap image and canvas components, but no video or animated image component yet.",
    );

    parent
}

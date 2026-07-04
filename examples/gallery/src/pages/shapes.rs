//! Canvas shapes and brush color swatch component examples.
//!
//! Corresponds to Fluent UI's shape primitives and theme color swatch patterns.

use crate::helpers::{card, class, grid, placeholder, sample_canvas};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiLabel,
};

/// Canvas shapes and brush/swatch color component examples.
///
/// Demonstrates available canvas drawing commands (rectangles, circles, paths)
/// and color swatch labels that map to theme palette tokens.
pub fn spawn_shapes_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let primitives = card(commands, g, "Shapes");
    commands.spawn_scene(bsn! {
        template_value(sample_canvas())
        template_value(class("gallery.canvas"))
        ChildOf(primitives)
    });

    let fills = card(commands, g, "Brushes");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Red"))
        template_value(class("gallery.swatch.red"))
        ChildOf(fills)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Green"))
        template_value(class("gallery.swatch.green"))
        ChildOf(fills)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Blue"))
        template_value(class("gallery.swatch.blue"))
        ChildOf(fills)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Gold"))
        template_value(class("gallery.swatch.gold"))
        ChildOf(fills)
    });

    placeholder(
        commands,
        g,
        "Gradient / transform brushes",
        "UiCanvasCommand supports solid-color fills and strokes; gradient brush stops are not exposed.",
    );

    placeholder(
        commands,
        g,
        "Shape hit testing",
        "Canvas drawing is visual only; per-shape pointer hit testing is not a public component contract.",
    );

    parent
}

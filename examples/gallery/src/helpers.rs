//! Shared helpers for the Fluent UI-style Gallery example.
//!
//! Provides utility functions for creating styled cards, grids, notes, placeholders,
//! section headers, and reusable widgets (canvas samples, generated images).

use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use bevy_math::Vec2;
use picus_core::{
    StyleClass, UiAvatar, UiCanvas, UiCanvasCommand, UiCanvasPathCommand, UiDivider, UiFlexColumn,
    UiGrid, UiImage, UiLabel, avatar_sizes, xilem::Color,
};

/// Create a single class name for an entity.
pub fn class(name: &str) -> StyleClass {
    StyleClass(vec![name.to_string()])
}

/// Create multiple class names for an entity.
pub fn classes(names: &[&str]) -> StyleClass {
    StyleClass(names.iter().map(|name| (*name).to_string()).collect())
}

/// Create a card container (UiFlexColumn with "gallery.card" class) inside `parent`.
pub fn card(commands: &mut Commands, parent: Entity, title: &str) -> Entity {
    let card = commands
        .spawn((UiFlexColumn, class("gallery.card"), ChildOf(parent)))
        .id();
    commands.spawn((
        UiLabel::new(title),
        class("gallery.card_title"),
        ChildOf(card),
    ));
    card
}

/// Create a grid container inside `parent` with the given number of columns.
pub fn grid(commands: &mut Commands, parent: Entity, columns: u32) -> Entity {
    commands
        .spawn((
            UiGrid::new(columns, 1),
            class("gallery.card_grid"),
            ChildOf(parent),
        ))
        .id()
}

/// Add a descriptive note label inside `parent`.
pub fn note(commands: &mut Commands, parent: Entity, text: impl Into<String>) {
    commands.spawn((UiLabel::new(text), class("gallery.note"), ChildOf(parent)));
}

/// Add a placeholder card inside `parent` for a feature that is not yet implemented.
pub fn placeholder(commands: &mut Commands, parent: Entity, title: &str, reason: &str) {
    let panel = commands
        .spawn((UiFlexColumn, class("gallery.placeholder"), ChildOf(parent)))
        .id();
    commands.spawn((
        UiLabel::new(title),
        class("gallery.card_title"),
        ChildOf(panel),
    ));
    commands.spawn((UiLabel::new(reason), class("gallery.note"), ChildOf(panel)));
}

/// Add a category section header with divider in the sidebar.
pub fn sidebar_category_header(commands: &mut Commands, parent: Entity, label: &str) {
    commands.spawn((
        UiLabel::new(label),
        class("gallery.sidebar_category"),
        ChildOf(parent),
    ));
}

/// Add a page description label.
#[allow(dead_code)]
pub fn page_description(commands: &mut Commands, parent: Entity, text: &str) {
    commands.spawn((
        UiLabel::new(text),
        class("gallery.page_description"),
        ChildOf(parent),
    ));
}

/// Add a horizontal divider.
#[allow(dead_code)]
pub fn divider(commands: &mut Commands, parent: Entity) {
    commands.spawn((UiDivider::horizontal(), ChildOf(parent)));
}

/// Create a sample canvas widget demonstrating Picus canvas drawing capabilities.
pub fn sample_canvas() -> UiCanvas {
    UiCanvas::new()
        .with_alt_text("Canvas shape sample")
        .with_command(UiCanvasCommand::FillCanvas {
            color: Color::from_rgb8(0x1E, 0x29, 0x3B),
        })
        .with_command(UiCanvasCommand::FillRoundedRect {
            x: 16.0,
            y: 16.0,
            width: 180.0,
            height: 90.0,
            radius: 12.0,
            color: Color::from_rgb8(0x25, 0x63, 0xEB),
        })
        .with_command(UiCanvasCommand::StrokeRoundedRect {
            x: 16.0,
            y: 16.0,
            width: 180.0,
            height: 90.0,
            radius: 12.0,
            color: Color::from_rgb8(0xBF, 0xDB, 0xFE),
            stroke_width: 2.0,
        })
        .with_command(UiCanvasCommand::FillCircle {
            cx: 238.0,
            cy: 62.0,
            radius: 42.0,
            color: Color::from_rgb8(0xF9, 0x73, 0x16),
        })
        .with_command(UiCanvasCommand::Line {
            x1: 24.0,
            y1: 132.0,
            x2: 296.0,
            y2: 132.0,
            color: Color::from_rgb8(0xF8, 0xFA, 0xFC),
            stroke_width: 3.0,
        })
        .with_command(UiCanvasCommand::FillPath {
            commands: vec![
                UiCanvasPathCommand::MoveTo { x: 42.0, y: 168.0 },
                UiCanvasPathCommand::LineTo { x: 118.0, y: 142.0 },
                UiCanvasPathCommand::LineTo { x: 164.0, y: 190.0 },
                UiCanvasPathCommand::ClosePath,
            ],
            color: Color::from_rgb8(0x22, 0xC5, 0x5E),
        })
        .with_command(UiCanvasCommand::StrokePath {
            commands: vec![
                UiCanvasPathCommand::MoveTo { x: 190.0, y: 170.0 },
                UiCanvasPathCommand::CubicTo {
                    x1: 214.0,
                    y1: 132.0,
                    x2: 266.0,
                    y2: 208.0,
                    x: 296.0,
                    y: 156.0,
                },
            ],
            color: Color::from_rgb8(0xE0, 0xE7, 0xFF),
            stroke_width: 4.0,
        })
}

/// Create a self-contained generated image for the media showcase.
pub fn generated_image() -> UiImage {
    let width = 320_u32;
    let height = 180_u32;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width - 1) as f32;
            let fy = y as f32 / (height - 1) as f32;
            let r = (42.0 + fx * 160.0) as u8;
            let g = (90.0 + fy * 120.0) as u8;
            let b = (180.0 - fx * 70.0 + fy * 40.0).clamp(0.0, 255.0) as u8;
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    UiImage::from_rgba8(width, height, rgba).with_alt_text("Generated Picus media sample")
}

/// Create an avatar for the top bar branding.
#[allow(dead_code)]
pub fn brand_avatar(name: &str) -> UiAvatar {
    UiAvatar::new(name).with_size(avatar_sizes::MD)
}

/// Fluent UI-style page viewport and content dimensions.
pub const PAGE_VIEWPORT: Vec2 = Vec2::new(1040.0, 560.0);
pub const PAGE_CONTENT: Vec2 = Vec2::new(1040.0, 5200.0);

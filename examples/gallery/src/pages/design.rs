//! Media and design control pages (one component per page).

use crate::helpers::{
    card, class, generated_image, grid, info_button, note, placeholder, sample_canvas,
};
use crate::state::{GalleryButtonAction, GalleryLocaleCombo};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    FluentIcon, LocalizeText, UiButton, UiCanvas, UiCanvasCommand, UiFlexColumn, UiFlexRow,
    UiGradientStop, UiImage, UiLabel, UiMarkdown, UiMultilineTextInput, UiSwitch, UiThemePicker,
    scene::{CommandsSceneExt, bsn, template_value},
    xilem::Color,
};

const MARKDOWN_SAMPLE: &str = r#"# Markdown

Picus renders **strong**, _emphasized_, `inline code`, ~~struck text~~, and [links](https://example.com).

- [x] Parse CommonMark and GFM extensions
- [ ] Keep streaming tails cheap

> Block quotes use the same resolved text palette.

| Feature | Status |
| :-- | --: |
| Tables | Ready |
| Fenced code | Highlighted |

```rust
fn render() {
    println!("markdown");
}
```"#;

pub fn spawn_image_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let generated = card(commands, g, "Generated image");
    commands.spawn_scene(bsn! {
        template_value(generated_image())
        template_value(class("gallery.image"))
        ChildOf(generated)
    });
    note(
        commands,
        generated,
        "The source image is generated in-memory so the example is self-contained.",
    );

    let empty = card(commands, g, "Image fallback");
    commands.spawn_scene(bsn! {
        template_value(UiImage::empty().with_alt_text("Image resource unavailable"))
        template_value(class("gallery.image"))
        ChildOf(empty)
    });
    placeholder(
        commands,
        empty,
        "Remote image loading",
        "This example avoids cargo run/network dependency for gallery startup.",
    );

    placeholder(
        commands,
        g,
        "Video / animated image",
        "Picus has bitmap image and canvas components, but no video or animated image component yet.",
    );
}

pub fn spawn_icons_page(commands: &mut Commands, parent: Entity) {
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
}

pub fn spawn_shapes_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let primitives = card(commands, g, "Canvas primitives");
    commands.spawn_scene(bsn! {
        template_value(sample_canvas())
        template_value(class("gallery.canvas"))
        ChildOf(primitives)
    });
    placeholder(
        commands,
        g,
        "Shape hit testing",
        "Canvas drawing is visual only; per-shape pointer hit testing is not a public component contract.",
    );
}

pub fn spawn_brushes_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let fills = card(commands, g, "Solid swatches");
    for (name, swatch) in [
        ("Red", "gallery.swatch.red"),
        ("Green", "gallery.swatch.green"),
        ("Blue", "gallery.swatch.blue"),
        ("Gold", "gallery.swatch.gold"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(name))
            template_value(class(swatch))
            ChildOf(fills)
        });
    }

    let gradients = card(commands, g, "Gradient brushes");
    commands.spawn_scene(bsn! {
        template_value(
            UiCanvas::new()
                .with_alt_text("Linear gradient sample")
                .with_size(320.0, 120.0)
                .with_command(UiCanvasCommand::FillLinearGradientRect {
                    x: 8.0,
                    y: 8.0,
                    width: 304.0,
                    height: 104.0,
                    start_x: 8.0,
                    start_y: 8.0,
                    end_x: 312.0,
                    end_y: 8.0,
                    stops: vec![
                        UiGradientStop::new(0.0, Color::from_rgb8(0x25, 0x63, 0xEB)),
                        UiGradientStop::new(0.5, Color::from_rgb8(0x7C, 0x3A, 0xED)),
                        UiGradientStop::new(1.0, Color::from_rgb8(0xDB, 0x27, 0x77)),
                    ],
                })
        )
        template_value(class("gallery.canvas"))
        ChildOf(gradients)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiCanvas::new()
                .with_alt_text("Radial gradient sample")
                .with_size(320.0, 120.0)
                .with_command(UiCanvasCommand::FillRadialGradientCircle {
                    cx: 160.0,
                    cy: 60.0,
                    radius: 52.0,
                    inner_radius: 0.0,
                    stops: vec![
                        UiGradientStop::new(0.0, Color::from_rgb8(0xF9, 0x73, 0x16)),
                        UiGradientStop::new(1.0, Color::from_rgb8(0x1E, 0x29, 0x3B)),
                    ],
                })
        )
        template_value(class("gallery.canvas"))
        ChildOf(gradients)
    });
}

pub fn spawn_typography_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let scale = card(commands, g, "Text scale");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Display / Hero"))
        template_value(class("gallery.typo.hero"))
        ChildOf(scale)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Title text"))
        template_value(class("gallery.typo.title"))
        ChildOf(scale)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Body text for gallery descriptions and form copy."))
        template_value(class("gallery.typo.body"))
        ChildOf(scale)
    });
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Caption / secondary metadata"))
        template_value(class("gallery.typo.caption"))
        ChildOf(scale)
    });

    let cjk = card(commands, g, "CJK / Unicode");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Picus Gallery: 骨 / 骨 / こんにちは / 你好"))
        template_value(class("gallery.typo.title"))
        ChildOf(cjk)
    });
    note(
        commands,
        cjk,
        "Fonts are bridged through Picus; this gallery registers the bundled Noto Sans font.",
    );

    let wrapping = card(commands, g, "Text wrapping");
    commands.spawn_scene(bsn! {
        template_value(
            UiMultilineTextInput::new(
                "Covers font families, weight, wrapping, alignment, and editable text. Picus exposes most text through labels and text inputs today.",
            )
            .read_only(true)
        )
        ChildOf(wrapping)
    });
}

pub fn spawn_markdown_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let markdown = card(commands, g, "Markdown sample");
    commands.spawn_scene(bsn! {
        template_value(UiMarkdown::new(MARKDOWN_SAMPLE))
        template_value(class("gallery.markdown"))
        ChildOf(markdown)
    });
    note(
        commands,
        markdown,
        "Parsing uses pulldown-cmark with CommonMark + GFM; fenced code uses syntect highlighting.",
    );
}

pub fn spawn_theme_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let theme = card(commands, g, "Theme transitions");
    commands.spawn_scene(bsn! {
        template_value(UiThemePicker::fluent())
        ChildOf(theme)
    });
    note(
        commands,
        theme,
        "Changing theme variants exercises style target sync and color transition animation.",
    );
    let hover_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Hover / press transition"))
            template_value(class("gallery.accent_button"))
            ChildOf(theme)
        })
        .id();
    commands
        .entity(hover_btn)
        .insert(GalleryButtonAction::Info {
            message: "Theme: hover/press transition clicked.".to_string(),
        });
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Animated switch target"))
        ChildOf(theme)
    });

    placeholder(
        commands,
        g,
        "Storyboard transitions",
        "Current public styling exposes color/scale transitions, not arbitrary property storyboards.",
    );
}

pub fn spawn_i18n_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let locale_card = card(commands, g, "Locale switching");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Select a locale to switch the active Fluent bundle and font stack:"))
        ChildOf(locale_card)
    });

    commands.spawn_scene(bsn! {
        template_value(
            picus::UiComboBox::new(vec![
                picus::UiComboOption::new("en-US", "English (en-US)"),
                picus::UiComboOption::new("zh-CN", "中文 (zh-CN)"),
                picus::UiComboOption::new("ja-JP", "日本語 (ja-JP)"),
            ])
            .with_placeholder("Choose locale")
        )
        GalleryLocaleCombo
        ChildOf(locale_card)
    });

    note(
        commands,
        locale_card,
        "Changing the locale reloads the Fluent bundle and applies the matching font fallback stack.",
    );

    let demo_row = commands
        .spawn_scene(bsn! {
            UiFlexRow
            template_value(class("gallery.card"))
            ChildOf(locale_card)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Localized greeting:"))
        ChildOf(demo_row)
    });
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-hello"))
        template_value(class("gallery.typo.title"))
        ChildOf(demo_row)
    });

    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-current-locale"))
        template_value(class("gallery.note"))
        ChildOf(locale_card)
    });

    let cjk_card = card(commands, g, "CJK / Unicode font fallback");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Picus Gallery: 骨 / 骨 / こんにちは / 你好"))
        template_value(class("gallery.typo.title"))
        ChildOf(cjk_card)
    });
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-cjk-note"))
        ChildOf(cjk_card)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMultilineTextInput::new("Han unification test\n骨 门 关 直").read_only(true))
        ChildOf(cjk_card)
    });

    let trans_card = card(commands, g, "Translation keys");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("The following text is resolved from Fluent bundles by key:"))
        ChildOf(trans_card)
    });
    let key_demo = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.card"))
            ChildOf(trans_card)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-han-unification"))
        ChildOf(key_demo)
    });
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-locale-action"))
        ChildOf(key_demo)
    });
    info_button(
        commands,
        key_demo,
        "Demo Button (static)",
        "I18n: Demo Button (static) clicked.",
    );
}

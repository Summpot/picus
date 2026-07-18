//! Media and design control pages (one component per page).

use crate::helpers::{
    card, class, classes, generated_image, grid, info_button, note, placeholder, sample_canvas,
};
use crate::state::{GalleryButtonAction, GalleryIconGrid, GalleryIconSearch, GalleryLocaleCombo};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    FluentIcon, LocalizeText, UiButton, UiCanvas, UiCanvasCommand, UiComboBox, UiComboOption,
    UiFlexColumn, UiFlexRow, UiGradientStop, UiGrid, UiImage, UiLabel, UiMarkdown,
    UiMultilineTextInput, UiSearch, UiSwitch, UiThemePicker, xilem::Color,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

/// Gallery browser table: name + glyph for every [`FluentIcon::ALL`] entry.
///
/// Kept as a named table for filter UX; length/coverage is guarded by tests
/// against `FluentIcon::ALL` so new enum variants cannot silently drop out.
pub const FLUENT_ICON_ENTRIES: &[(&str, FluentIcon)] = &[
    ("Accept", FluentIcon::Accept),
    ("Add", FluentIcon::Add),
    ("AllApps", FluentIcon::AllApps),
    ("Back", FluentIcon::Back),
    ("Brightness", FluentIcon::Brightness),
    ("Cancel", FluentIcon::Cancel),
    ("Character", FluentIcon::Character),
    ("Checkmark", FluentIcon::Checkmark),
    ("Checkbox", FluentIcon::Checkbox),
    ("ChevronDown", FluentIcon::ChevronDown),
    ("ChevronLeft", FluentIcon::ChevronLeft),
    ("ChevronRight", FluentIcon::ChevronRight),
    ("ChevronUp", FluentIcon::ChevronUp),
    ("Clock", FluentIcon::Clock),
    ("Contact", FluentIcon::Contact),
    ("Delete", FluentIcon::Delete),
    ("DockLeft", FluentIcon::DockLeft),
    ("Edit", FluentIcon::Edit),
    ("Folder", FluentIcon::Folder),
    ("Font", FluentIcon::Font),
    ("Forward", FluentIcon::Forward),
    ("GlobalNavigationButton", FluentIcon::GlobalNavigationButton),
    ("Globe", FluentIcon::Globe),
    ("Help", FluentIcon::Help),
    ("Info", FluentIcon::Info),
    ("List", FluentIcon::List),
    ("Map", FluentIcon::Map),
    ("Message", FluentIcon::Message),
    ("More", FluentIcon::More),
    ("Pictures", FluentIcon::Pictures),
    ("Placeholder", FluentIcon::Placeholder),
    ("Refresh", FluentIcon::Refresh),
    ("Remove", FluentIcon::Remove),
    ("Search", FluentIcon::Search),
    ("Send", FluentIcon::Send),
    ("Settings", FluentIcon::Settings),
    ("Stop", FluentIcon::Stop),
    ("Sync", FluentIcon::Sync),
    ("TouchPointer", FluentIcon::TouchPointer),
    ("ViewAll", FluentIcon::ViewAll),
];

/// Icons whose names contain `filter` (case-insensitive). Empty filter → full set.
fn filtered_icon_entries(filter: &str) -> impl Iterator<Item = (&'static str, FluentIcon)> + '_ {
    let q = filter.trim().to_lowercase();
    FLUENT_ICON_ENTRIES
        .iter()
        .copied()
        .filter(move |(name, _)| q.is_empty() || name.to_lowercase().contains(&q))
}

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

fn spawn_token_swatch(commands: &mut Commands, parent: Entity, label: &str, token_class: &str) {
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new(label))
        template_value(classes(&["gallery.token.swatch", token_class]))
        ChildOf(parent)
    });
}

pub fn spawn_color_page(commands: &mut Commands, parent: Entity) {
    note(
        commands,
        parent,
        "WinUI Color design page → Picus theme RON tokens (fluent_theme variants) + gallery.token.swatch + per-token color classes. Interactive picking lives on ColorPicker.",
    );

    let g = grid(commands, parent, 2);

    let surfaces = card(commands, g, "Surface / fill tokens");
    for (label, token_class) in [
        ("surface-bg", "gallery.token.surface-bg"),
        ("surface-panel", "gallery.token.surface-panel"),
        ("surface-subtle", "gallery.token.surface-subtle"),
        ("surface-card", "gallery.token.surface-card"),
        ("surface-elevated", "gallery.token.surface-elevated"),
        ("surface-accent", "gallery.token.surface-accent"),
        ("fill-layer-default", "gallery.token.fill-layer-default"),
        ("fill-control-default", "gallery.token.fill-control-default"),
    ] {
        spawn_token_swatch(commands, surfaces, label, token_class);
    }

    let text = card(commands, g, "Text / accent tokens");
    for (label, token_class) in [
        ("text-primary", "gallery.token.text-primary"),
        ("text-secondary", "gallery.token.text-secondary"),
        ("text-heading", "gallery.token.text-heading"),
        ("text-disabled", "gallery.token.text-disabled"),
        ("accent-primary", "gallery.token.accent-primary"),
        ("brand-foreground", "gallery.token.brand-foreground"),
        ("text-link", "gallery.token.text-link"),
        ("text-on-accent", "gallery.token.text-on-accent"),
    ] {
        spawn_token_swatch(commands, text, label, token_class);
    }

    let status = card(commands, g, "Status tokens");
    for (label, token_class) in [
        ("status-info", "gallery.token.status-info"),
        ("status-success", "gallery.token.status-success"),
        ("status-warning", "gallery.token.status-warning"),
        ("status-error", "gallery.token.status-error"),
    ] {
        spawn_token_swatch(commands, status, label, token_class);
    }

    let borders = card(commands, g, "Border tokens");
    for (label, token_class) in [
        ("border-default", "gallery.token.border-default"),
        ("border-muted", "gallery.token.border-muted"),
        ("border-subtle", "gallery.token.border-subtle"),
        ("focus-stroke", "gallery.token.focus-stroke"),
    ] {
        // Border-color demos use gallery.token.border_swatch for a thicker visible stroke.
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(classes(&["gallery.token.border_swatch", token_class]))
            ChildOf(borders)
        });
    }
}

pub fn spawn_geometry_page(commands: &mut Commands, parent: Entity) {
    note(
        commands,
        parent,
        "WinUI Geometry → Picus layout tokens radius-* / border-* in fluent_theme RON (selected scale: none…2xl + pill; 3xl–5xl exist in the theme). Vector primitives are on the Shapes page (UiCanvas).",
    );

    let g = grid(commands, parent, 2);

    let radii = card(commands, g, "Corner radius tokens");
    for (label, token_class) in [
        ("radius-none", "gallery.radius.none"),
        ("radius-xs", "gallery.radius.xs"),
        ("radius-sm", "gallery.radius.sm"),
        ("radius-md", "gallery.radius.md"),
        ("radius-lg", "gallery.radius.lg"),
        ("radius-xl", "gallery.radius.xl"),
        ("radius-2xl", "gallery.radius.2xl"),
        ("radius-pill", "gallery.radius.pill"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(classes(&["gallery.radius.sample", token_class]))
            ChildOf(radii)
        });
    }

    let strokes = card(commands, g, "Stroke width tokens");
    for (label, token_class) in [
        ("border-thin", "gallery.stroke.thin"),
        ("border-thick", "gallery.stroke.thick"),
        ("border-thicker", "gallery.stroke.thicker"),
        ("border-thickest", "gallery.stroke.thickest"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(classes(&["gallery.stroke.sample", token_class]))
            ChildOf(strokes)
        });
    }

    let canvas_card = card(commands, g, "Canvas geometry sample");
    commands.spawn_scene(bsn! {
        template_value(sample_canvas())
        template_value(class("gallery.canvas"))
        ChildOf(canvas_card)
    });
    note(
        commands,
        canvas_card,
        "Rounded rects, circles, and paths use absolute geometry on UiCanvas — not stylesheet corner_radius.",
    );
}

pub fn spawn_spacing_page(commands: &mut Commands, parent: Entity) {
    note(
        commands,
        parent,
        "WinUI Spacing → Picus space-* and gap-* tokens. Apply via stylesheet padding/gap or InlineStyle; gallery.space.sample + per-step classes resolve Var tokens.",
    );

    let g = grid(commands, parent, 1);

    let scale = card(commands, g, "Spacing scale (padding demos)");
    for (label, token_class) in [
        ("space-xxs", "gallery.space.xxs"),
        ("space-xs", "gallery.space.xs"),
        ("space-sm", "gallery.space.sm"),
        ("space-md", "gallery.space.md"),
        ("space-lg", "gallery.space.lg"),
        ("space-m", "gallery.space.m"),
        ("space-xl", "gallery.space.xl"),
        ("space-xxl", "gallery.space.xxl"),
        ("space-xxxl", "gallery.space.xxxl"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(classes(&["gallery.space.sample", token_class]))
            ChildOf(scale)
        });
    }

    let gaps = card(commands, g, "Layout gap demos");
    note(
        commands,
        gaps,
        "Rows below use gallery.gap.sm / md / lg (maps to gap-sm/md/lg tokens) around three sample tiles.",
    );
    for (title, gap_class) in [
        ("gap-sm", "gallery.gap.sm"),
        ("gap-md", "gallery.gap.md"),
        ("gap-lg", "gallery.gap.lg"),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(title))
            template_value(class("gallery.note"))
            ChildOf(gaps)
        });
        let row = commands
            .spawn_scene(bsn! {
                UiFlexRow
                template_value(class(gap_class))
                ChildOf(gaps)
            })
            .id();
        for tile in ["A", "B", "C"] {
            commands.spawn_scene(bsn! {
                template_value(UiLabel::new(tile))
                template_value(class("gallery.space.tile"))
                ChildOf(row)
            });
        }
    }

    let stack = card(commands, g, "Stacked spacing");
    let col = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.gap.lg"))
            ChildOf(stack)
        })
        .id();
    for label in ["First block", "Second block", "Third block"] {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(label))
            template_value(classes(&["gallery.space.sample", "gallery.space.md"]))
            ChildOf(col)
        });
    }
    note(
        commands,
        stack,
        "Compose UiFlexColumn/UiFlexRow with gap tokens for StackPanel-style spacing (see also StackPanel page).",
    );
}

pub fn spawn_icons_page(commands: &mut Commands, parent: Entity) {
    note(
        commands,
        parent,
        "WinUI Iconography → Picus FluentIcon (facade). Glyphs use Segoe Fluent Icons with Segoe MDL2 / FabricMDL2 / Segoe UI Symbol fallbacks.",
    );

    let browser = card(commands, parent, "Fluent glyph browser");
    commands.spawn_scene(bsn! {
        template_value(UiSearch::new("Filter icons by name\u{2026}"))
        GalleryIconSearch
        ChildOf(browser)
    });
    note(
        commands,
        browser,
        "Type to filter the grid. Each cell shows the glyph character and FluentIcon name via UiLabel + gallery.icon font stack.",
    );

    let g = commands
        .spawn_scene(bsn! {
            template_value(UiGrid::new(6, 1))
            template_value(class("gallery.card_grid"))
            GalleryIconGrid
            ChildOf(browser)
        })
        .id();
    spawn_icon_grid_cells(commands, g, "");
}

/// Populate (or re-populate) the icon browser grid with entries matching `filter`.
pub fn spawn_icon_grid_cells(commands: &mut Commands, grid: Entity, filter: &str) {
    let mut matched = 0usize;
    for (name, icon) in filtered_icon_entries(filter) {
        matched += 1;
        let cell = commands
            .spawn_scene(bsn! {
                UiFlexColumn
                template_value(class("gallery.icon_cell"))
                ChildOf(grid)
            })
            .id();
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(icon.glyph().to_string()))
            template_value(class("gallery.icon"))
            ChildOf(cell)
        });
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(name))
            template_value(class("gallery.icon_label"))
            ChildOf(cell)
        });
    }
    if matched == 0 {
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new("No icons match this filter."))
            template_value(class("gallery.note"))
            ChildOf(grid)
        });
    }
}

/// Rebuild the icon grid from a search query (exclusive-system path).
///
/// Despawns existing cells, then reuses [`spawn_icon_grid_cells`] via the
/// world's deferred command queue so filter + cell construction stay single-path.
pub fn rebuild_icon_grid(world: &mut World, filter: &str) {
    let grids: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<GalleryIconGrid>>();
        query.iter(world).collect()
    };
    for &grid in &grids {
        let children: Vec<Entity> = world
            .get::<bevy_ecs::hierarchy::Children>(grid)
            .map(|c| c.iter().collect())
            .unwrap_or_default();
        for child in children {
            world.entity_mut(child).despawn();
        }
    }
    {
        let mut commands = world.commands();
        for grid in grids {
            spawn_icon_grid_cells(&mut commands, grid, filter);
        }
    }
    world.flush();
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
            UiComboBox::new(vec![
                UiComboOption::new("en-US", "English (en-US)"),
                UiComboOption::new("zh-CN", "中文 (zh-CN)"),
                UiComboOption::new("ja-JP", "日本語 (ja-JP)"),
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

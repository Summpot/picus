//! Internationalization, locale switching, and CJK font fallback examples.
//!
//! Corresponds to Fluent UI's localization pattern — switching display language
//! and verifying CJK font fallback behavior through registered i18n bundles.

use crate::helpers::{card, class, grid, info_button, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    LocalizeText, UiComboBox, UiComboOption, UiFlexColumn, UiFlexRow, UiLabel,
    UiMultilineTextInput,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Locale combo and CJK font fallback component examples.
pub fn spawn_i18n_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    // --- Locale switching card ---
    let locale_card = card(commands, g, "Locale Switching");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Select a locale to switch the active Fluent bundle and font stack:"))
        ChildOf(locale_card)
    });

    let locale_combo = commands
        .spawn_scene(bsn! {
            template_value(
                UiComboBox::new(vec![
                    UiComboOption::new("en-US", "English (en-US)"),
                    UiComboOption::new("zh-CN", "中文 (zh-CN)"),
                    UiComboOption::new("ja-JP", "日本語 (ja-JP)"),
                ])
                .with_placeholder("Choose locale")
            )
            ChildOf(locale_card)
        })
        .id();

    note(
        commands,
        locale_card,
        "Changing the locale reloads the Fluent bundle and applies the matching font fallback stack.",
    );

    // LocalizeText demo: shows a greeting that changes with the locale
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

    // Current locale display (also reactive to locale changes)
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-current-locale"))
        template_value(class("gallery.note"))
        ChildOf(locale_card)
    });

    // --- CJK / Unicode card ---
    let cjk_card = card(commands, g, "CJK / Unicode Font Fallback");
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Picus Gallery: 骨 / 骨 / こんにちは / 你好"))
        template_value(class("gallery.typo.title"))
        ChildOf(cjk_card)
    });
    commands.spawn_scene(bsn! {
        template_value(LocalizeText::new("gallery-cjk-note"))
        ChildOf(cjk_card)
    });

    // Han unification test string (same glyphs in all locales)
    commands.spawn_scene(bsn! {
        template_value(UiMultilineTextInput::new("Han unification test\n骨 门 关 直").read_only(true))
        ChildOf(cjk_card)
    });

    // --- Translation demo card ---
    let trans_card = card(commands, g, "Translation Keys");
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

    locale_combo
}

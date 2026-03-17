#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::{process::Command, sync::Arc, time::Duration};

#[cfg(not(target_os = "macos"))]
use std::sync::Mutex;

use anyhow::{Context, Result};
use bevy_asset::{AssetPlugin, Assets, Handle, RenderAssetUsages};
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};
use bevy_image::Image as BevyImage;
use bevy_text::TextPlugin;
use crossbeam_channel::{Receiver, Sender, unbounded};
use lucide_icons::Icon as LucideIcon;
#[cfg(target_os = "macos")]
use picus_activation::MacosBundleConfig;
use picus_activation::{
    ActivationConfig, ActivationService, BootstrapOutcome, ProtocolRegistration, bootstrap,
};
#[cfg(test)]
use picus_core::bevy_app::PreUpdate;
use picus_core::{
    AppI18n, AppPicusExt, LUCIDE_FONT_FAMILY, OverlayConfig, OverlayPlacement, OverlayState,
    PicusPlugin, ProjectionCtx, ResolvedStyle, StyleClass, StyleSheet, StyleValue, SyncAssetSource,
    SyncTextSource, UiComboBox, UiComboBoxChanged, UiComboOption, UiEventQueue, UiRoot,
    UiTextInput, UiTextInputChanged, UiThemePicker, UiView, apply_direct_widget_style,
    apply_label_style, apply_widget_style,
    bevy_app::{App, Startup, Update},
    bevy_ecs::{hierarchy::ChildOf, prelude::*},
    bevy_tasks::{AsyncComputeTaskPool, IoTaskPool, TaskPool},
    bevy_tween::{
        BevyTweenRegisterSystems,
        bevy_time_runner::{TimeContext, TimeRunner, TimeSpan},
        component_tween_system,
        interpolate::Interpolator,
        interpolation::EaseKind,
        tween::ComponentTween,
    },
    bevy_window::WindowResized,
    button_with_child, resolve_style, resolve_style_for_classes, resolve_style_for_entity_classes,
    run_app_with_window_options, spawn_in_overlay_root,
    xilem::{
        Color,
        masonry::layout::{Dim, Length},
        style::Style as _,
        view::{
            CrossAxisAlignment, FlexExt as _, MainAxisAlignment, flex_col, flex_row, image, label,
            portal, sized_box, virtual_scroll,
        },
        winit::{dpi::LogicalSize, error::EventLoopError},
    },
};
use pixiv_client::{
    AuthSession, DecodedImageRgba, IdpUrlResponse, Illust, PixivApiClient, PixivContentKind,
    PixivResponse, build_browser_login_url, generate_pkce_code_verifier, pkce_s256_challenge,
};
use reqwest::Url;
use shared_utils::init_logging;
use unic_langid::LanguageIdentifier;
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

mod actions;
mod activation;
mod bootstrap;
mod network;
mod persistence;
mod state;
mod ui;

use bootstrap::*;
use state::*;

use actions::{drain_ui_actions_and_dispatch, track_viewport_metrics};
use activation::poll_activation_messages;
pub(crate) use bootstrap::run;
use network::{apply_image_results, apply_network_results, spawn_image_tasks, spawn_network_tasks};
use ui::{
    project_auth_panel, project_detail_overlay, project_home_feed, project_illust_card,
    project_main_column, project_overlay_tag, project_overlay_tags, project_response_panel,
    project_root, project_search_panel, project_sidebar,
};

#[cfg(test)]
mod tests {
    use super::*;
    use picus_core::bevy_ecs::schedule::Schedule;

    fn mock_illust(title: &str) -> Illust {
        Illust {
            id: 1,
            title: title.to_string(),
            image_urls: pixiv_client::ImageUrls {
                medium: "https://example.com/m.jpg".to_string(),
                large: "https://example.com/l.jpg".to_string(),
                square_medium: "https://example.com/s.jpg".to_string(),
            },
            user: pixiv_client::User {
                id: 9,
                name: "artist".to_string(),
                account: Some("artist_account".to_string()),
                profile_image_urls: pixiv_client::ProfileImageUrls {
                    medium: "https://example.com/avatar.jpg".to_string(),
                },
            },
            tags: Vec::new(),
            total_view: 0,
            total_bookmarks: 0,
            total_comments: 0,
            is_bookmarked: false,
            page_count: 1,
            meta_single_page: None,
            content_kind: pixiv_client::PixivContentKind::Illust,
            description: None,
        }
    }

    #[test]
    fn feed_layout_scales_with_viewport_width() {
        let (narrow_columns, _) = ui::compute_feed_layout(900.0, false);
        let (wide_columns, _) = ui::compute_feed_layout(1700.0, false);

        assert!(wide_columns >= narrow_columns);
        assert!(wide_columns > 1);
    }

    #[test]
    fn collapsed_sidebar_yields_more_card_space() {
        let (expanded_columns, expanded_card_width) = ui::compute_feed_layout(1360.0, false);
        let (collapsed_columns, collapsed_card_width) = ui::compute_feed_layout(1360.0, true);

        assert!(collapsed_columns >= expanded_columns);
        assert!(collapsed_card_width >= expanded_card_width);
    }

    #[test]
    fn card_height_estimator_reflects_title_length() {
        let mut world = World::new();

        let short = world
            .spawn((mock_illust("short"), IllustVisual::default()))
            .id();
        let long = world
            .spawn((
                mock_illust(
                    "a very long illustration title that should wrap to multiple lines in cards",
                ),
                IllustVisual::default(),
            ))
            .id();

        let short_h = ui::estimate_illust_card_height(&world, short, 280.0);
        let long_h = ui::estimate_illust_card_height(&world, long, 280.0);

        assert!(long_h > short_h);
    }

    #[test]
    fn locale_combo_event_applies_even_without_app_action_events() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(StyleSheet::default());
        world.insert_resource(UiState::default());

        let locale_combo = world
            .spawn((UiComboBox::new(vec![
                UiComboOption::new("en-US", "English"),
                UiComboOption::new("zh-CN", "简体中文"),
                UiComboOption::new("ja-JP", "日本語"),
            ]),))
            .id();
        world.insert_resource(PixivUiComponents {
            toggle_sidebar: Entity::PLACEHOLDER,
            locale_combo,
            code_verifier_input: Entity::PLACEHOLDER,
            auth_code_input: Entity::PLACEHOLDER,
            refresh_token_input: Entity::PLACEHOLDER,
            search_input: Entity::PLACEHOLDER,
            home_tab: Entity::PLACEHOLDER,
            rankings_tab: Entity::PLACEHOLDER,
            manga_tab: Entity::PLACEHOLDER,
            novels_tab: Entity::PLACEHOLDER,
            search_tab: Entity::PLACEHOLDER,
            open_browser_login: Entity::PLACEHOLDER,
            exchange_auth_code: Entity::PLACEHOLDER,
            refresh_token: Entity::PLACEHOLDER,
            search_submit: Entity::PLACEHOLDER,
            copy_response: Entity::PLACEHOLDER,
            clear_response: Entity::PLACEHOLDER,
            close_overlay: Entity::PLACEHOLDER,
        });

        world.resource::<UiEventQueue>().push_typed(
            locale_combo,
            UiComboBoxChanged {
                combo: locale_combo,
                selected: 1,
                value: "zh-CN".to_string(),
            },
        );

        drain_ui_actions_and_dispatch(&mut world);

        assert_eq!(
            world.resource::<AppI18n>().active_locale,
            parse_locale("zh-CN")
        );
        assert_eq!(
            world
                .get::<UiComboBox>(locale_combo)
                .and_then(UiComboBox::clamped_selected),
            Some(1)
        );
    }

    #[test]
    fn auth_code_can_be_extracted_from_nested_redirect() {
        let nested = "https://example.com/callback?redirect_uri=https%3A%2F%2Fapp.example.com%2Fauth%3Fcode%3Dabc123";
        assert_eq!(
            activation::extract_auth_code_from_input(nested).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn pixiv_custom_scheme_auth_code_is_supported() {
        let uri = "pixiv://account/login?code=from_protocol&via=login";
        assert_eq!(
            activation::extract_auth_code_from_input(uri).as_deref(),
            Some("from_protocol")
        );
        assert!(activation::is_pixiv_callback_uri(uri));
    }

    #[test]
    fn info_plist_keeps_expected_bundle_identifier() {
        let plist = include_str!("../Info.plist");
        assert!(
            plist.contains("<string>dev.summpot.example-pixiv-client</string>"),
            "Info.plist should keep the Pixiv app bundle identifier stable"
        );
    }

    #[test]
    fn app_actions_emitted_in_preupdate_are_drained_in_update() {
        let mut app = App::new();
        app.insert_resource(UiEventQueue::default());
        app.insert_resource(UiState::default());
        app.insert_resource(PixivUiComponents {
            toggle_sidebar: Entity::PLACEHOLDER,
            locale_combo: Entity::PLACEHOLDER,
            code_verifier_input: Entity::PLACEHOLDER,
            auth_code_input: Entity::PLACEHOLDER,
            refresh_token_input: Entity::PLACEHOLDER,
            search_input: Entity::PLACEHOLDER,
            home_tab: Entity::PLACEHOLDER,
            rankings_tab: Entity::PLACEHOLDER,
            manga_tab: Entity::PLACEHOLDER,
            novels_tab: Entity::PLACEHOLDER,
            search_tab: Entity::PLACEHOLDER,
            open_browser_login: Entity::PLACEHOLDER,
            exchange_auth_code: Entity::PLACEHOLDER,
            refresh_token: Entity::PLACEHOLDER,
            search_submit: Entity::PLACEHOLDER,
            copy_response: Entity::PLACEHOLDER,
            clear_response: Entity::PLACEHOLDER,
            close_overlay: Entity::PLACEHOLDER,
        });

        app.add_systems(PreUpdate, |queue: Res<UiEventQueue>| {
            queue.push_typed(
                Entity::PLACEHOLDER,
                AppAction::SetSearchText("same-frame text".to_string()),
            );
        });
        app.add_systems(Update, drain_ui_actions_and_dispatch);

        app.update();

        assert_eq!(
            app.world().resource::<UiState>().search_text,
            "same-frame text"
        );
    }

    #[test]
    fn setup_builds_componentized_ui_tree() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let tree = *world.resource::<PixivUiTree>();
        let ui_components = *world.resource::<PixivUiComponents>();
        assert!(world.get::<PixivHomeFeed>(tree.home_feed).is_some());
        assert!(world.get::<PixivOverlayTags>(tree.overlay_tags).is_some());
        let overlay_parent = world
            .get::<ChildOf>(tree.overlay_tags)
            .map(ChildOf::parent)
            .expect("overlay tags should be parented to detail overlay");
        let overlay_state = world
            .get::<OverlayState>(overlay_parent)
            .expect("detail overlay should carry OverlayState");
        assert!(overlay_state.is_modal);
        assert!(
            world
                .get::<UiComboBox>(ui_components.locale_combo)
                .is_some()
        );
        assert!(
            world
                .get::<UiTextInput>(ui_components.code_verifier_input)
                .is_some()
        );
        assert!(
            world
                .get::<UiTextInput>(ui_components.auth_code_input)
                .is_some()
        );
        assert!(
            world
                .get::<UiTextInput>(ui_components.refresh_token_input)
                .is_some()
        );
        assert!(
            world
                .get::<UiTextInput>(ui_components.search_input)
                .is_some()
        );
        assert!(world.get_entity(ui_components.manga_tab).is_ok());
        assert!(world.get_entity(ui_components.novels_tab).is_ok());
        assert_eq!(
            world
                .get::<UiComboBox>(ui_components.locale_combo)
                .and_then(UiComboBox::clamped_selected),
            Some(0)
        );
    }

    #[test]
    fn ensure_task_pool_initializes_io_pool() {
        ensure_task_pool_initialized();
        let _ = IoTaskPool::get();
    }

    #[test]
    fn pixiv_locale_ids_do_not_use_dot_namespace() {
        let locales = [
            ("en-US", include_str!("../assets/locales/en-US/main.ftl")),
            ("zh-CN", include_str!("../assets/locales/zh-CN/main.ftl")),
            ("ja-JP", include_str!("../assets/locales/ja-JP/main.ftl")),
        ];

        for (locale, content) in locales {
            assert!(
                !content
                    .lines()
                    .map(str::trim_start)
                    .any(|line| line.starts_with("pixiv.")),
                "{locale} locale still contains dot-separated pixiv message IDs"
            );
        }
    }

    #[test]
    fn text_input_events_and_programmatic_updates_stay_in_sync() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(StyleSheet::default());
        world.insert_resource(UiState::default());
        world.insert_resource(AuthState::default());

        let code_verifier_input = world
            .spawn((UiTextInput::new("").with_placeholder("PKCE code_verifier"),))
            .id();
        let auth_code_input = world
            .spawn((UiTextInput::new("").with_placeholder("Auth code"),))
            .id();
        let refresh_token_input = world
            .spawn((UiTextInput::new("").with_placeholder("Refresh token"),))
            .id();
        let search_input = world
            .spawn((UiTextInput::new("").with_placeholder("Search illust keyword"),))
            .id();

        world.insert_resource(PixivUiComponents {
            toggle_sidebar: Entity::PLACEHOLDER,
            locale_combo: Entity::PLACEHOLDER,
            code_verifier_input,
            auth_code_input,
            refresh_token_input,
            search_input,
            home_tab: Entity::PLACEHOLDER,
            rankings_tab: Entity::PLACEHOLDER,
            manga_tab: Entity::PLACEHOLDER,
            novels_tab: Entity::PLACEHOLDER,
            search_tab: Entity::PLACEHOLDER,
            open_browser_login: Entity::PLACEHOLDER,
            exchange_auth_code: Entity::PLACEHOLDER,
            refresh_token: Entity::PLACEHOLDER,
            search_submit: Entity::PLACEHOLDER,
            copy_response: Entity::PLACEHOLDER,
            clear_response: Entity::PLACEHOLDER,
            close_overlay: Entity::PLACEHOLDER,
        });

        world.resource::<UiEventQueue>().push_typed(
            search_input,
            UiTextInputChanged {
                input: search_input,
                value: "same-frame keyword".to_string(),
            },
        );

        drain_ui_actions_and_dispatch(&mut world);

        assert_eq!(
            world.resource::<UiState>().search_text,
            "same-frame keyword"
        );
        assert_eq!(
            world
                .get::<UiTextInput>(search_input)
                .expect("search input should exist")
                .value,
            "same-frame keyword"
        );

        world.resource::<UiEventQueue>().push_typed(
            Entity::PLACEHOLDER,
            AppAction::SetSearchText("猫咪".to_string()),
        );

        drain_ui_actions_and_dispatch(&mut world);

        assert_eq!(world.resource::<UiState>().search_text, "猫咪");
        assert_eq!(
            world
                .get::<UiTextInput>(search_input)
                .expect("search input should exist")
                .value,
            "猫咪"
        );
    }

    #[test]
    fn drain_dispatch_consumes_pending_widget_text_actions_before_sync() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(StyleSheet::default());
        world.insert_resource(UiState::default());
        world.insert_resource(AuthState::default());

        let code_verifier_input = world
            .spawn((UiTextInput::new("").with_placeholder("PKCE code_verifier"),))
            .id();
        let auth_code_input = world
            .spawn((UiTextInput::new("").with_placeholder("Auth code"),))
            .id();
        let refresh_token_input = world
            .spawn((UiTextInput::new("").with_placeholder("Refresh token"),))
            .id();
        let search_input = world
            .spawn((UiTextInput::new("").with_placeholder("Search illust keyword"),))
            .id();

        world.insert_resource(PixivUiComponents {
            toggle_sidebar: Entity::PLACEHOLDER,
            locale_combo: Entity::PLACEHOLDER,
            code_verifier_input,
            auth_code_input,
            refresh_token_input,
            search_input,
            home_tab: Entity::PLACEHOLDER,
            rankings_tab: Entity::PLACEHOLDER,
            manga_tab: Entity::PLACEHOLDER,
            novels_tab: Entity::PLACEHOLDER,
            search_tab: Entity::PLACEHOLDER,
            open_browser_login: Entity::PLACEHOLDER,
            exchange_auth_code: Entity::PLACEHOLDER,
            refresh_token: Entity::PLACEHOLDER,
            search_submit: Entity::PLACEHOLDER,
            copy_response: Entity::PLACEHOLDER,
            clear_response: Entity::PLACEHOLDER,
            close_overlay: Entity::PLACEHOLDER,
        });

        world.resource::<UiEventQueue>().push_typed(
            search_input,
            picus_core::WidgetUiAction::SetTextInput {
                input: search_input,
                value: "same-frame widget action".to_string(),
            },
        );

        drain_ui_actions_and_dispatch(&mut world);

        assert_eq!(
            world.resource::<UiState>().search_text,
            "same-frame widget action"
        );
        assert_eq!(
            world
                .get::<UiTextInput>(search_input)
                .expect("search input should exist")
                .value,
            "same-frame widget action"
        );
    }

    #[test]
    fn embedded_pixiv_theme_ron_parses() {
        picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixiv_client.ron"))
            .expect("embedded pixiv_client stylesheet should parse");
    }

    #[test]
    fn pixiv_primary_button_uses_neutral_fluent_tokens() {
        let sheet =
            picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixiv_client.ron"))
                .expect("embedded pixiv_client stylesheet should parse");

        let button = sheet
            .get_class_values("pixiv.button")
            .expect("pixiv.button class should exist");
        let primary = sheet
            .get_class_values("pixiv.button.primary")
            .expect("pixiv.button.primary class should exist");

        let corner_radius = match button.layout.corner_radius.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button corner_radius should come from a theme token"),
        };
        let primary_bg = match primary.colors.bg.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.primary bg should come from a theme token"),
        };
        let primary_border = match primary.colors.border.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.primary border should come from a theme token"),
        };

        assert_eq!(corner_radius, "radius-md");
        assert_eq!(primary_bg, "surface-panel");
        assert_eq!(primary_border, "border-default");
    }

    #[test]
    fn pixiv_text_input_uses_neutral_fluent_tokens() {
        let sheet =
            picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixiv_client.ron"))
                .expect("embedded pixiv_client stylesheet should parse");

        let input = sheet
            .get_class_values("pixiv.text-input")
            .expect("pixiv.text-input class should exist");

        let bg = match input.colors.bg.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.text-input bg should come from a theme token"),
        };
        let border = match input.colors.border.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.text-input border should come from a theme token"),
        };

        assert_eq!(bg, "surface-subtle");
        assert_eq!(border, "border-default");
    }

    #[test]
    fn pixiv_warn_button_uses_fluent_tokens() {
        let sheet =
            picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixiv_client.ron"))
                .expect("embedded pixiv_client stylesheet should parse");
        let warn = sheet
            .get_class_values("pixiv.button.warn")
            .expect("pixiv.button.warn class should exist");

        let bg = match warn.colors.bg.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.warn bg should come from a theme token"),
        };
        let hover_bg = match warn.colors.hover_bg.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.warn hover_bg should come from a theme token"),
        };
        let pressed_bg = match warn.colors.pressed_bg.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.warn pressed_bg should come from a theme token"),
        };
        let border = match warn.colors.border.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.warn border should come from a theme token"),
        };
        let text = match warn.colors.text.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.button.warn text should come from a theme token"),
        };

        assert_eq!(bg, "status-error-bg");
        assert_eq!(hover_bg, "status-error-border");
        assert_eq!(pressed_bg, "surface-overlay-item-pressed");
        assert_eq!(border, "status-error-border");
        assert_eq!(text, "text-primary");
    }

    #[test]
    fn sync_font_stack_for_locale_preserves_tokenized_fields() {
        let mut sheet =
            picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixiv_client.ron"))
                .expect("embedded pixiv_client stylesheet should parse");

        // Pixiv sheet intentionally carries class rules with token refs but no local token map.
        assert!(sheet.tokens.is_empty());

        let stack = vec!["Inter".to_string(), "sans-serif".to_string()];
        sync_font_stack_for_locale(&mut sheet, Some(&stack));

        let root = sheet
            .get_class_values("pixiv.root")
            .expect("pixiv.root class should exist");

        let padding_token = match root.layout.padding.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.root padding should remain tokenized"),
        };
        assert_eq!(padding_token, "space-lg");

        let font_family = match root.font_family.as_ref() {
            Some(picus_core::StyleValue::Value(value)) => value,
            _ => panic!("font family should be written as a literal style value"),
        };
        assert_eq!(font_family, &stack);

        let sidebar_button = sheet
            .get_class_values("pixiv.sidebar.button")
            .expect("pixiv.sidebar.button class should exist");
        let sidebar_font_family = match sidebar_button.font_family.as_ref() {
            Some(picus_core::StyleValue::Value(value)) => value,
            _ => panic!("sidebar button font family should be written as a literal style value"),
        };
        assert_eq!(sidebar_font_family, &stack);
    }

    #[test]
    fn locale_combo_initial_selection_follows_active_locale() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("ja-JP")));

        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let ui_components = *world.resource::<PixivUiComponents>();
        let combo = world
            .get::<UiComboBox>(ui_components.locale_combo)
            .expect("locale combo should exist");
        let selected = combo
            .clamped_selected()
            .expect("locale combo should select active locale");

        assert_eq!(combo.options[selected].value, "ja-JP");
    }

    #[test]
    fn pixiv_ui_drops_legacy_unicode_icons() {
        let app_source = include_str!("app.rs");
        let ui_source = include_str!("app/ui.rs");
        let source = format!("{app_source}\n{ui_source}");
        for codepoint in [0x25B6, 0x25C0, 0x1F464, 0x1F441, 0x2764, 0x2665, 0x2661] {
            let legacy_icon = char::from_u32(codepoint)
                .expect("valid unicode codepoint")
                .to_string();
            assert!(
                !source.contains(&legacy_icon),
                "legacy unicode icon `{legacy_icon}` should be replaced by lucide"
            );
        }
        assert!(
            source.contains("LucideIcon"),
            "pixiv client should use lucide icons in app UI"
        );
    }
}

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
    AppI18n, AppPicusExt, LUCIDE_FONT_FAMILY, OverlayComputedPosition, PicusPlugin, ProjectionCtx,
    ResolvedStyle, StyleClass, StyleSheet, StyleValue, SyncAssetSource, SyncTextSource, UiComboBox,
    UiComboBoxChanged, UiComboOption, UiDialog, UiEventQueue, UiRoot, UiTextInput,
    UiTextInputChanged, UiThemePicker, UiView, apply_direct_widget_style, apply_label_style,
    apply_widget_style,
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
    bevy_window::{PrimaryWindow, Window, WindowResized},
    button, button_with_child, resolve_style, resolve_style_for_classes,
    resolve_style_for_entity_classes, run_app_with_window_options, spawn_in_overlay_root,
    xilem::{
        Color,
        masonry::layout::{Dim, Length},
        style::Style as _,
        view::{
            CrossAxisAlignment, FlexExt as _, MainAxisAlignment, flex_col, flex_row, image, label,
            sized_box, virtual_scroll,
        },
        winit::{dpi::LogicalSize, error::EventLoopError},
    },
};
use pixcus::{
    AuthSession, AuthUserSummary, DecodedImageRgba, IdpUrlResponse, Illust, PixivApiClient,
    PixivContentKind, PixivResponse, build_browser_login_url, generate_pkce_code_verifier,
    pkce_s256_challenge,
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
    project_account_menu, project_auth_dialog_form, project_auth_panel, project_detail_meta_rail,
    project_detail_overlay, project_home_feed, project_illust_card, project_main_column,
    project_overlay_tag, project_overlay_tags, project_response_panel, project_root,
    project_search_panel, project_sidebar,
};

#[cfg(test)]
mod tests {
    use super::*;
    use picus_core::{
        OverlayPlacement, UiScrollView, bevy_ecs::schedule::Schedule, bevy_math::Vec2,
    };

    fn mock_illust(title: &str) -> Illust {
        Illust {
            id: 1,
            title: title.to_string(),
            image_urls: pixcus::ImageUrls {
                medium: "https://example.com/m.jpg".to_string(),
                large: "https://example.com/l.jpg".to_string(),
                square_medium: "https://example.com/s.jpg".to_string(),
            },
            user: pixcus::User {
                id: 9,
                name: "artist".to_string(),
                account: Some("artist_account".to_string()),
                profile_image_urls: pixcus::ProfileImageUrls {
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
            content_kind: pixcus::PixivContentKind::Illust,
            description: None,
            width: 800,
            height: 600,
        }
    }

    fn mock_illust_with_id(id: u64) -> Illust {
        let mut illust = mock_illust("sample");
        illust.id = id;
        illust.title = format!("illust-{id}");
        illust
    }

    fn mock_auth_session() -> AuthSession {
        AuthSession {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_type: "bearer".to_string(),
            expires_in: 3600,
            scope: "all".to_string(),
        }
    }

    fn mock_user_summary() -> AuthUserSummary {
        AuthUserSummary {
            id: 33_239_622,
            name: "summpot".to_string(),
            account: Some("user_knrk3528".to_string()),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
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
    fn feed_scroll_viewport_reserves_space_for_optional_panels() {
        let (base_width, base_height) =
            ui::compute_feed_scroll_viewport_size(1360.0, 860.0, false, false, false);
        let (_, search_height) =
            ui::compute_feed_scroll_viewport_size(1360.0, 860.0, false, true, false);
        let (_, response_height) =
            ui::compute_feed_scroll_viewport_size(1360.0, 860.0, false, false, true);

        assert!(base_width >= state::CARD_MIN_WIDTH);
        assert!(search_height < base_height);
        assert!(response_height < base_height);
    }

    #[test]
    fn feed_layout_for_precomputed_width_matches_window_based_layout() {
        let (feed_width, _) =
            ui::compute_feed_scroll_viewport_size(1360.0, 860.0, false, false, false);

        assert_eq!(
            ui::compute_feed_layout_for_width(feed_width),
            ui::compute_feed_layout(1360.0, false)
        );
    }

    #[test]
    fn feed_layout_width_prefers_ancestor_scroll_viewport() {
        let mut world = World::new();
        world.insert_resource(UiState::default());
        world.insert_resource(ViewportMetrics {
            width: 2400.0,
            height: 1400.0,
        });

        let scroll = world
            .spawn(UiScrollView::new(
                Vec2::new(720.0, 480.0),
                Vec2::new(720.0, 1600.0),
            ))
            .id();
        let feed = world.spawn((PixivHomeFeed, ChildOf(scroll))).id();
        let card = world.spawn((PixivIllustCard, ChildOf(feed))).id();

        assert_eq!(ui::feed_layout_width(&world, feed), 720.0);
        assert_eq!(ui::feed_layout_width(&world, card), 720.0);
    }

    #[test]
    fn feed_layout_width_falls_back_to_window_metrics_without_scroll_ancestor() {
        let mut world = World::new();
        world.insert_resource(UiState::default());
        world.insert_resource(ViewportMetrics {
            width: 1800.0,
            height: 1000.0,
        });

        let feed = world.spawn(PixivHomeFeed).id();
        let (expected_width, _) =
            ui::compute_feed_scroll_viewport_size(1800.0, 1000.0, false, false, false);

        assert_eq!(ui::feed_layout_width(&world, feed), expected_width);
    }

    #[test]
    fn sync_feed_scroll_viewport_uses_primary_window_dimensions() {
        let mut world = World::new();
        let feed_scroll = world.spawn(UiScrollView::default()).id();

        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed: Entity::PLACEHOLDER,
            detail_scroll: Entity::PLACEHOLDER,
            overlay_tags: Entity::PLACEHOLDER,
        });
        world.insert_resource(ViewportMetrics::default());
        world.insert_resource(UiState::default());
        world.insert_resource(ResponsePanelState::default());

        let mut window = Window::default();
        window.resolution.set(2560.0, 1440.0);
        world.spawn((window, PrimaryWindow));

        let mut schedule = Schedule::default();
        schedule.add_systems(actions::sync_feed_scroll_viewport);
        schedule.run(&mut world);

        let (expected_width, expected_height) =
            ui::compute_feed_scroll_viewport_size(2560.0, 1440.0, false, false, false);
        let scroll = world
            .get::<UiScrollView>(feed_scroll)
            .expect("feed scroll should exist after sync");

        assert_eq!(
            scroll.viewport_size,
            Vec2::new(expected_width as f32, expected_height as f32)
        );
    }

    #[test]
    fn sync_feed_scroll_viewport_falls_back_to_viewport_metrics_without_primary_window() {
        let mut world = World::new();
        let feed_scroll = world.spawn(UiScrollView::default()).id();

        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed: Entity::PLACEHOLDER,
            detail_scroll: Entity::PLACEHOLDER,
            overlay_tags: Entity::PLACEHOLDER,
        });
        world.insert_resource(ViewportMetrics {
            width: 1920.0,
            height: 1080.0,
        });
        world.insert_resource(UiState::default());
        world.insert_resource(ResponsePanelState::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(actions::sync_feed_scroll_viewport);
        schedule.run(&mut world);

        let (expected_width, expected_height) =
            ui::compute_feed_scroll_viewport_size(1920.0, 1080.0, false, false, false);
        let scroll = world
            .get::<UiScrollView>(feed_scroll)
            .expect("feed scroll should exist after sync");

        assert_eq!(
            scroll.viewport_size,
            Vec2::new(expected_width as f32, expected_height as f32)
        );
    }

    #[test]
    fn feed_results_append_without_duplicates_and_reset_scroll_on_replace() {
        let mut world = World::new();
        let feed_scroll = world.spawn(UiScrollView::default()).id();
        let home_feed = world.spawn(PixivHomeFeed).id();
        if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(feed_scroll) {
            scroll_view.scroll_offset = Vec2::new(0.0, 180.0);
        }

        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed,
            detail_scroll: Entity::PLACEHOLDER,
            overlay_tags: Entity::PLACEHOLDER,
        });
        world.insert_resource(UiState::default());
        world.insert_resource(FeedOrder::default());
        world.insert_resource(FeedPagination {
            next_url: None,
            loading: true,
            generation: 1,
        });
        world.insert_resource(FeedSeenIds::default());
        world.insert_resource(ResponsePanelState::default());

        let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
        let (result_tx, result_rx) = unbounded::<NetworkResult>();
        world.insert_resource(NetworkBridge {
            cmd_tx,
            cmd_rx,
            result_tx: result_tx.clone(),
            result_rx,
        });

        let (image_cmd_tx, image_cmd_rx) = unbounded::<ImageCommand>();
        let (image_result_tx, image_result_rx) = unbounded::<ImageResult>();
        world.insert_resource(ImageBridge {
            cmd_tx: image_cmd_tx,
            cmd_rx: image_cmd_rx,
            result_tx: image_result_tx,
            result_rx: image_result_rx,
        });

        result_tx
            .send(NetworkResult::FeedLoaded {
                source: NavTab::Home,
                payload: PixivResponse {
                    illusts: vec![mock_illust_with_id(1), mock_illust_with_id(2)],
                    next_url: Some("page-2".to_string()),
                },
                generation: 1,
                append: false,
            })
            .expect("initial feed result should send");
        network::apply_network_results(&mut world);

        let order = &world.resource::<FeedOrder>().0;
        assert_eq!(order.len(), 2);
        assert_eq!(world.resource::<FeedSeenIds>().0.len(), 2);
        assert_eq!(
            world.resource::<FeedPagination>().next_url.as_deref(),
            Some("page-2")
        );
        assert!(!world.resource::<FeedPagination>().loading);
        assert_eq!(
            world
                .get::<UiScrollView>(feed_scroll)
                .expect("feed scroll should exist")
                .scroll_offset,
            Vec2::ZERO
        );

        world.resource_mut::<FeedPagination>().loading = true;
        result_tx
            .send(NetworkResult::FeedLoaded {
                source: NavTab::Home,
                payload: PixivResponse {
                    illusts: vec![mock_illust_with_id(2), mock_illust_with_id(3)],
                    next_url: None,
                },
                generation: 1,
                append: true,
            })
            .expect("append feed result should send");
        network::apply_network_results(&mut world);

        let ids = world
            .resource::<FeedOrder>()
            .0
            .iter()
            .map(|entity| {
                world
                    .get::<Illust>(*entity)
                    .expect("feed entity should keep illust")
                    .id
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2, 3]);
        assert_eq!(world.resource::<FeedSeenIds>().0.len(), 3);
    }

    #[test]
    fn stale_feed_generation_is_ignored() {
        let mut world = World::new();
        let feed_scroll = world.spawn(UiScrollView::default()).id();
        let home_feed = world.spawn(PixivHomeFeed).id();

        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed,
            detail_scroll: Entity::PLACEHOLDER,
            overlay_tags: Entity::PLACEHOLDER,
        });
        world.insert_resource(UiState::default());
        world.insert_resource(FeedOrder::default());
        world.insert_resource(FeedPagination {
            next_url: None,
            loading: true,
            generation: 2,
        });
        world.insert_resource(FeedSeenIds::default());
        world.insert_resource(ResponsePanelState::default());

        let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
        let (result_tx, result_rx) = unbounded::<NetworkResult>();
        world.insert_resource(NetworkBridge {
            cmd_tx,
            cmd_rx,
            result_tx: result_tx.clone(),
            result_rx,
        });

        let (image_cmd_tx, image_cmd_rx) = unbounded::<ImageCommand>();
        let (image_result_tx, image_result_rx) = unbounded::<ImageResult>();
        world.insert_resource(ImageBridge {
            cmd_tx: image_cmd_tx,
            cmd_rx: image_cmd_rx,
            result_tx: image_result_tx,
            result_rx: image_result_rx,
        });

        result_tx
            .send(NetworkResult::FeedLoaded {
                source: NavTab::Home,
                payload: PixivResponse {
                    illusts: vec![mock_illust_with_id(99)],
                    next_url: None,
                },
                generation: 1,
                append: false,
            })
            .expect("stale feed result should send");
        network::apply_network_results(&mut world);

        assert!(world.resource::<FeedOrder>().0.is_empty());
        assert!(world.resource::<FeedSeenIds>().0.is_empty());
        assert!(world.resource::<FeedPagination>().loading);
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
    fn card_height_estimator_handles_long_cjk_titles() {
        let mut world = World::new();

        let short = world
            .spawn((mock_illust("短标题"), IllustVisual::default()))
            .id();
        let long = world
            .spawn((
                mock_illust("这是一段需要在卡片中换行展示的较长插画标题示例"),
                IllustVisual::default(),
            ))
            .id();

        let short_h = ui::estimate_illust_card_height(&world, short, 280.0);
        let long_h = ui::estimate_illust_card_height(&world, long, 280.0);

        assert!(long_h > short_h);
    }

    #[test]
    fn card_height_estimator_uses_compact_footer_budget() {
        let mut world = World::new();
        let card = world
            .spawn((mock_illust("short"), IllustVisual::default()))
            .id();

        let estimated = ui::estimate_illust_card_height(&world, card, 280.0);
        let image_min = 280.0_f64 * 0.62_f64;
        let image_height = image_min.max(120.0_f64);
        let expected = image_height + 64.0_f64 + 18.0_f64;

        assert!((estimated - expected).abs() < f64::EPSILON);
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
            auth_dialog_toggle: Entity::PLACEHOLDER,
            account_menu_toggle: Entity::PLACEHOLDER,
            logout: Entity::PLACEHOLDER,
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
    fn authenticated_session_updates_auth_state_and_queues_home_feed() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiState::default());
        world.insert_resource(AuthState {
            auth_code_input: "callback-code".to_string(),
            refresh_token_input: "stale-refresh".to_string(),
            login_dialog_open: true,
            account_menu_open: true,
            ..AuthState::default()
        });
        world.insert_resource(ResponsePanelState {
            title: "Last response".to_string(),
            content: "details".to_string(),
        });
        world.insert_resource(FeedPagination::default());

        let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
        let (result_tx, result_rx) = unbounded::<NetworkResult>();
        world.insert_resource(NetworkBridge {
            cmd_tx,
            cmd_rx: cmd_rx.clone(),
            result_tx,
            result_rx,
        });

        let session = mock_auth_session();
        let user_summary = mock_user_summary();
        let resolved_user_summary = network::apply_authenticated_session(
            &mut world,
            session.clone(),
            Some(user_summary.clone()),
        );

        assert_eq!(resolved_user_summary, Some(user_summary.clone()));

        let auth = world.resource::<AuthState>();
        assert_eq!(auth.session.as_ref(), Some(&session));
        assert_eq!(auth.user_summary.as_ref(), Some(&user_summary));
        assert_eq!(auth.refresh_token_input, session.refresh_token);
        assert!(auth.auth_code_input.is_empty());
        assert!(!auth.login_dialog_open);
        assert!(!auth.account_menu_open);

        let response_panel = world.resource::<ResponsePanelState>();
        assert!(response_panel.title.is_empty());
        assert!(response_panel.content.is_empty());

        let pagination = world.resource::<FeedPagination>();
        assert!(pagination.loading);
        assert_eq!(pagination.generation, 1);

        let queued = cmd_rx
            .try_recv()
            .expect("home feed request should be queued");
        match queued {
            NetworkCommand::FetchHome { generation } => assert_eq!(generation, 1),
            other => panic!("expected FetchHome after auth, got {other:?}"),
        }
    }

    #[test]
    fn authenticated_session_preserves_existing_user_summary_when_refresh_has_none() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiState::default());
        world.insert_resource(AuthState {
            user_summary: Some(mock_user_summary()),
            ..AuthState::default()
        });
        world.insert_resource(ResponsePanelState::default());
        world.insert_resource(FeedPagination::default());

        let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
        let (result_tx, result_rx) = unbounded::<NetworkResult>();
        world.insert_resource(NetworkBridge {
            cmd_tx,
            cmd_rx: cmd_rx.clone(),
            result_tx,
            result_rx,
        });

        let resolved_user_summary =
            network::apply_authenticated_session(&mut world, mock_auth_session(), None);

        assert_eq!(resolved_user_summary, Some(mock_user_summary()));
        assert_eq!(
            world.resource::<AuthState>().user_summary,
            Some(mock_user_summary())
        );
        assert!(matches!(
            cmd_rx
                .try_recv()
                .expect("home feed request should be queued"),
            NetworkCommand::FetchHome { .. }
        ));
    }

    #[test]
    fn clear_authenticated_runtime_resets_logout_sensitive_state() {
        let mut world = World::new();
        let feed_scroll = world.spawn(UiScrollView::default()).id();
        let home_feed = world.spawn(PixivHomeFeed).id();
        let overlay_parent = world.spawn_empty().id();
        let feed_card = world
            .spawn((mock_illust_with_id(7), ChildOf(home_feed)))
            .id();
        let overlay_tag = world
            .spawn((
                OverlayTag {
                    text: "tag-a".to_string(),
                },
                ChildOf(overlay_parent),
            ))
            .id();
        let selected_illust = world.spawn(mock_illust_with_id(8)).id();

        if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(feed_scroll) {
            scroll_view.scroll_offset = Vec2::new(0.0, 220.0);
        }

        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed,
            detail_scroll: Entity::PLACEHOLDER,
            overlay_tags: overlay_parent,
        });
        world.insert_resource(UiState {
            active_tab: NavTab::Search,
            selected_illust: Some(selected_illust),
            ..UiState::default()
        });
        world.insert_resource(AuthState {
            session: Some(mock_auth_session()),
            user_summary: Some(mock_user_summary()),
            code_verifier_input: "verifier".to_string(),
            auth_code_input: "code".to_string(),
            refresh_token_input: "refresh".to_string(),
            login_dialog_open: true,
            account_menu_open: true,
            ..AuthState::default()
        });
        world.insert_resource(FeedOrder(vec![feed_card]));
        world.insert_resource(FeedSeenIds(std::collections::HashSet::from([7])));
        world.insert_resource(FeedPagination {
            next_url: Some("https://example.com/next".to_string()),
            loading: true,
            generation: 4,
        });
        world.insert_resource(OverlayTags(vec![overlay_tag]));
        world.insert_resource(ResponsePanelState {
            title: "Error".to_string(),
            content: "details".to_string(),
        });

        actions::clear_authenticated_runtime(&mut world);

        let auth = world.resource::<AuthState>();
        assert!(auth.session.is_none());
        assert!(auth.user_summary.is_none());
        assert!(auth.code_verifier_input.is_empty());
        assert!(auth.auth_code_input.is_empty());
        assert!(auth.refresh_token_input.is_empty());
        assert!(!auth.login_dialog_open);
        assert!(!auth.account_menu_open);

        let ui = world.resource::<UiState>();
        assert_eq!(ui.active_tab, NavTab::Home);
        assert!(ui.selected_illust.is_none());

        assert!(world.resource::<FeedOrder>().0.is_empty());
        assert!(world.resource::<FeedSeenIds>().0.is_empty());
        let pagination = world.resource::<FeedPagination>();
        assert_eq!(pagination.generation, 5);
        assert!(!pagination.loading);
        assert!(pagination.next_url.is_none());
        assert!(world.resource::<OverlayTags>().0.is_empty());
        assert!(world.get_entity(feed_card).is_err());
        assert!(world.get_entity(overlay_tag).is_err());
        assert_eq!(
            world
                .get::<UiScrollView>(feed_scroll)
                .expect("feed scroll should exist")
                .scroll_offset,
            Vec2::ZERO
        );
        assert!(world.resource::<ResponsePanelState>().title.is_empty());
        assert!(world.resource::<ResponsePanelState>().content.is_empty());
    }

    #[test]
    fn auth_visibility_actions_toggle_dialog_and_account_menu() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(UiState::default());
        world.insert_resource(AuthState::default());
        world.insert_resource(PixivUiComponents {
            toggle_sidebar: Entity::PLACEHOLDER,
            locale_combo: Entity::PLACEHOLDER,
            auth_dialog_toggle: Entity::PLACEHOLDER,
            account_menu_toggle: Entity::PLACEHOLDER,
            logout: Entity::PLACEHOLDER,
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
        });

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenLoginDialog);
        drain_ui_actions_and_dispatch(&mut world);
        assert!(world.resource::<AuthState>().login_dialog_open);
        assert!(
            world
                .query_filtered::<Entity, With<PixivAuthDialog>>()
                .iter(&world)
                .next()
                .is_some()
        );

        dismiss_auth_dialog_overlay(&mut world);
        world.resource_mut::<AuthState>().login_dialog_open = false;
        assert!(
            world
                .query_filtered::<Entity, With<PixivAuthDialog>>()
                .iter(&world)
                .next()
                .is_none()
        );

        {
            let mut auth = world.resource_mut::<AuthState>();
            auth.session = Some(mock_auth_session());
        }
        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::ToggleAccountMenu);
        drain_ui_actions_and_dispatch(&mut world);

        let auth = world.resource::<AuthState>();
        assert!(!auth.login_dialog_open);
        assert!(auth.account_menu_open);
    }

    #[test]
    fn toggle_account_menu_respawns_overlay_when_missing() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        world.resource_mut::<AuthState>().session = Some(mock_auth_session());

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::ToggleAccountMenu);
        drain_ui_actions_and_dispatch(&mut world);

        let account_menu = world
            .query_filtered::<Entity, With<PixivAccountMenu>>()
            .iter(&world)
            .next()
            .expect("account menu overlay should exist after opening");
        world.entity_mut(account_menu).despawn();

        world.resource_mut::<AuthState>().account_menu_open = true;
        ensure_account_menu_overlay(&mut world);

        let respawned = world
            .query_filtered::<Entity, With<PixivAccountMenu>>()
            .iter(&world)
            .next()
            .expect("account menu overlay should respawn when reopened");
        let popover = world
            .get::<picus_core::UiPopover>(respawned)
            .expect("respawned account menu should carry shared popover metadata");

        assert!(world.resource::<AuthState>().account_menu_open);
        assert_eq!(popover.placement, OverlayPlacement::TopEnd);
        assert!(popover.auto_flip_placement);
        assert_eq!(popover.size_hint(), (132.0, 56.0));
    }

    #[test]
    fn compact_author_name_keeps_short_names_and_ellipsizes_long_ones() {
        assert_eq!(ui::compact_author_name("summpot"), "summpot");
        assert_eq!(
            ui::compact_author_name("a very long display name for an illustrator"),
            "a very long display na…"
        );
        assert_eq!(
            ui::compact_author_name("这是一位名字特别特别长的作者示例"),
            "这是一位名字特别特别长的作者示例"
        );
    }

    #[test]
    fn author_overlay_uses_noninteractive_sized_wrapper_inside_thumbnail_button() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let overlay_body: UiView = if hovered {")
                && ui_source.contains(
                    "let author_overlay: UiView = Arc::new(\n        sized_box(overlay_body)"
                ),
            "author overlay should stay a non-interactive sized layer inside the thumbnail button"
        );
    }

    #[test]
    fn home_feed_wraps_translated_tiles_with_entity_hitboxes() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let (card_entity, child_view) = &child_views[item.index];")
                && ui_source.contains("transformed(opaque_hitbox_for_entity(*card_entity, tile))")
                && ui_source.contains(".translate((item.x, row.y))")
                && !ui_source.contains("transformed(tile).translate((item.x, row.y))"),
            "home feed tiles should use the card entity when wrapping translated tiles in opaque hitboxes"
        );
    }

    #[test]
    fn card_hover_considers_thumbnail_and_bookmark_entities() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("get::<InteractionState>(action_entities.open_thumbnail)")
                && ui_source.contains("get::<InteractionState>(action_entities.bookmark)")
                && ui_source.contains(".unwrap_or(false)\n        || ctx"),
            "card hover should stay active when either thumbnail or bookmark is hovered"
        );
    }

    #[test]
    fn author_overlay_stays_inside_thumbnail_image_stack() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let image_with_overlay: UiView = Arc::new(")
                && ui_source.contains("zstack(vec![Arc::new(image_view), author_overlay])")
                && ui_source.contains("button_with_child(")
                && ui_source.contains("image_with_overlay,"),
            "author overlay should be painted inside the thumbnail image stack"
        );
    }

    #[test]
    fn author_overlay_wrapper_has_explicit_stack_dimensions() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let author_overlay: UiView = Arc::new(")
                && ui_source.contains(".width(Dim::Stretch)")
                && ui_source.contains(".height(Dim::Fixed(Length::px(32.0)))"),
            "author overlay wrapper should keep explicit dimensions inside the zstack"
        );
    }

    #[test]
    fn author_overlay_budgets_text_width_inside_the_thumbnail_row() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let author_label =")
                && ui_source.contains(
                    "sized_box(apply_label_style(label(author.to_string()), &overlay_style))"
                )
                && ui_source.contains(".width(Dim::Stretch);")
                && ui_source.contains("Arc::new(author_label).flex(1.0).into_any_flex()"),
            "author overlay should constrain the author label to the remaining thumbnail row width"
        );
    }

    #[test]
    fn author_overlay_preserves_inherited_layout_style() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let overlay_style = picus_core::ResolvedStyle {")
                && ui_source.contains("layout: style.layout,")
                && !ui_source.contains("layout: Default::default(),"),
            "author overlay should preserve inherited layout instead of resetting to the default scale"
        );
    }

    #[test]
    fn heart_button_is_direct_top_right_child_without_stretch_overlay_shell() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let heart_button = sized_box")
                && ui_source.contains("zstack(vec![open_button_view, Arc::new(heart_button)])")
                && ui_source.contains(".alignment(UnitPoint::TOP_RIGHT)")
                && !ui_source.contains("let heart_overlay: UiView = Arc::new(")
                && !ui_source.contains("zstack(vec![Arc::new(heart_button)])")
                && !ui_source.contains("badged(open_button_view, heart_button)"),
            "heart button should be the outer stack's direct top-right child without a stretch overlay shell"
        );
    }

    #[test]
    fn heart_button_uses_icon_only_transparent_button_styling() {
        let ui_source = include_str!("app/ui.rs");

        assert!(
            ui_source.contains("let heart_button = sized_box(Arc::new(\n        button_with_child(")
                && ui_source.contains("AppAction::Bookmark(ctx.entity)")
                && ui_source.contains(".padding(0.0)")
                && ui_source.contains(".border(Color::TRANSPARENT, 0.0)")
                && ui_source.contains(".background_color(Color::TRANSPARENT)")
                && !ui_source.contains("let heart_button = sized_box(Arc::new(apply_direct_widget_style(")
                && ui_source.contains("let heart_icon_color = subtle_button_style.colors.text.unwrap_or(Color::WHITE);"),
            "heart button should stay clickable while rendering as an icon-only transparent affordance"
        );
    }

    #[test]
    fn detail_dialog_meta_rail_uses_real_scroll_view_pattern() {
        let ui_source = include_str!("app/ui.rs");
        let bootstrap_source = include_str!("app/bootstrap.rs");

        assert!(
            ui_source
                .contains("sized_box(ctx.children.into_iter().next().unwrap_or_else(empty_ui))")
                && ui_source.contains("pub(super) fn project_detail_meta_rail")
                && bootstrap_source.contains("UiScrollView::new(")
                && bootstrap_source.contains("PixivDetailRailScroll")
                && bootstrap_source.contains("PixivDetailMetaRail"),
            "detail dialog metadata should be rendered through a real UiScrollView child rail"
        );
    }

    #[test]
    fn idp_discovery_does_not_replace_authenticated_status_line() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiState {
            status_line: "Authenticated. Loading home feed…".to_string(),
            ..UiState::default()
        });
        world.insert_resource(AuthState {
            session: Some(mock_auth_session()),
            ..AuthState::default()
        });
        world.insert_resource(FeedOrder::default());
        world.insert_resource(FeedPagination::default());
        world.insert_resource(FeedSeenIds::default());
        world.insert_resource(ResponsePanelState::default());

        let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
        let (result_tx, result_rx) = unbounded::<NetworkResult>();
        world.insert_resource(NetworkBridge {
            cmd_tx,
            cmd_rx,
            result_tx: result_tx.clone(),
            result_rx,
        });

        let (image_cmd_tx, image_cmd_rx) = unbounded::<ImageCommand>();
        let (image_result_tx, image_result_rx) = unbounded::<ImageResult>();
        world.insert_resource(ImageBridge {
            cmd_tx: image_cmd_tx,
            cmd_rx: image_cmd_rx,
            result_tx: image_result_tx,
            result_rx: image_result_rx,
        });

        result_tx
            .send(NetworkResult::IdpDiscovered(IdpUrlResponse {
                auth_token_url: "https://example.com/auth".to_string(),
                auth_token_redirect_url: "pixiv://account/login".to_string(),
            }))
            .expect("idp result should send");

        network::apply_network_results(&mut world);

        assert_eq!(
            world.resource::<UiState>().status_line,
            "Authenticated. Loading home feed…"
        );
        assert_eq!(
            world
                .resource::<AuthState>()
                .idp_urls
                .as_ref()
                .map(|value| value.auth_token_url.as_str()),
            Some("https://example.com/auth")
        );
    }

    #[test]
    fn info_plist_keeps_expected_bundle_identifier() {
        let plist = include_str!("../Info.plist");
        assert!(
            plist.contains("<string>dev.summpot.example-pixcus</string>"),
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
            auth_dialog_toggle: Entity::PLACEHOLDER,
            account_menu_toggle: Entity::PLACEHOLDER,
            logout: Entity::PLACEHOLDER,
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
        assert!(world.get::<ChildOf>(tree.overlay_tags).is_none());
        assert!(
            world
                .query_filtered::<Entity, With<PixivAuthDialog>>()
                .iter(&world)
                .next()
                .is_none(),
            "auth dialog overlay should be spawned on demand"
        );
        assert!(
            world
                .query_filtered::<Entity, With<PixivAccountMenu>>()
                .iter(&world)
                .next()
                .is_none(),
            "account menu overlay should be spawned on demand"
        );
        assert!(
            world
                .query_filtered::<Entity, With<PixivDetailDialog>>()
                .iter(&world)
                .next()
                .is_none(),
            "detail dialog should be spawned on demand"
        );
        assert!(
            world
                .get::<UiComboBox>(ui_components.locale_combo)
                .is_some()
        );
        assert_eq!(ui_components.code_verifier_input, Entity::PLACEHOLDER);
        assert_eq!(ui_components.auth_code_input, Entity::PLACEHOLDER);
        assert_eq!(ui_components.refresh_token_input, Entity::PLACEHOLDER);
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
    fn open_login_dialog_spawns_built_in_auth_dialog_overlay_and_inputs() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        {
            let mut auth = world.resource_mut::<AuthState>();
            auth.session = None;
            auth.user_summary = None;
            auth.login_dialog_open = false;
            auth.account_menu_open = false;
        }

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenLoginDialog);
        drain_ui_actions_and_dispatch(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivAuthDialog>>()
            .iter(&world)
            .next()
            .expect("login dialog should spawn after clicking Login");
        assert!(world.get::<UiDialog>(dialog).is_some());

        let ui_components = *world.resource::<PixivUiComponents>();
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
    }

    #[test]
    fn dismissing_spawned_login_dialog_clears_open_state_and_input_handles() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        {
            let mut auth = world.resource_mut::<AuthState>();
            auth.session = None;
            auth.user_summary = None;
            auth.login_dialog_open = false;
            auth.account_menu_open = false;
        }

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenLoginDialog);
        drain_ui_actions_and_dispatch(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivAuthDialog>>()
            .iter(&world)
            .next()
            .expect("login dialog should spawn before dismissal");
        world
            .resource::<UiEventQueue>()
            .push_typed(dialog, picus_core::OverlayUiAction::DismissDialog);
        picus_core::handle_overlay_actions(&mut world);
        drain_ui_actions_and_dispatch(&mut world);

        assert!(world.get_entity(dialog).is_err());

        let auth = world.resource::<AuthState>();
        assert!(!auth.login_dialog_open);
        let ui_components = *world.resource::<PixivUiComponents>();
        assert_eq!(ui_components.code_verifier_input, Entity::PLACEHOLDER);
        assert_eq!(ui_components.auth_code_input, Entity::PLACEHOLDER);
        assert_eq!(ui_components.refresh_token_input, Entity::PLACEHOLDER);
    }

    #[test]
    fn open_illust_spawns_built_in_detail_dialog_and_reparents_tags() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(ViewportMetrics {
            width: 1600.0,
            height: 1000.0,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let illust = world.spawn(mock_illust_with_id(77)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenIllust(illust));
        drain_ui_actions_and_dispatch(&mut world);
        ensure_detail_dialog_overlay(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivDetailDialog>>()
            .iter(&world)
            .next()
            .expect("detail dialog should spawn after opening an illustration");
        let dialog_component = world
            .get::<UiDialog>(dialog)
            .expect("detail dialog should carry UiDialog component");
        let (expected_width, expected_height) = ui::compute_detail_dialog_size(1600.0, 1000.0);
        assert_eq!(dialog_component.width, Some(expected_width));
        assert_eq!(dialog_component.height, Some(expected_height));

        let detail_body = world
            .query_filtered::<Entity, With<PixivDetailOverlay>>()
            .iter(&world)
            .next()
            .expect("detail dialog should contain the example detail body");
        assert_eq!(
            world
                .get::<ChildOf>(detail_body)
                .expect("detail body should be parented to the dialog")
                .parent(),
            dialog
        );

        let detail_scroll = world.resource::<PixivUiTree>().detail_scroll;
        assert!(world.get::<UiScrollView>(detail_scroll).is_some());
        assert_eq!(
            world
                .get::<ChildOf>(detail_scroll)
                .expect("detail rail scroll should be parented to the detail body")
                .parent(),
            detail_body
        );

        let detail_meta_rail = world
            .query_filtered::<Entity, With<PixivDetailMetaRail>>()
            .iter(&world)
            .next()
            .expect("detail dialog should contain a scrollable metadata rail child");
        assert_eq!(
            world
                .get::<ChildOf>(detail_meta_rail)
                .expect("detail metadata rail should be parented to the detail scroll view")
                .parent(),
            detail_scroll
        );

        let tags_entity = world.resource::<PixivUiTree>().overlay_tags;
        assert_eq!(
            world
                .get::<ChildOf>(tags_entity)
                .expect("tags should be reparented under the detail metadata rail")
                .parent(),
            detail_meta_rail
        );
    }

    #[test]
    fn reopening_detail_dialog_recreates_tags_container_and_children() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let mut illust = mock_illust_with_id(91);
        illust.tags = vec![
            pixcus::Tag {
                name: "landscape".to_string(),
                translated_name: Some("Landscape".to_string()),
            },
            pixcus::Tag {
                name: "night".to_string(),
                translated_name: None,
            },
        ];
        let illust = world.spawn(illust).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenIllust(illust));
        drain_ui_actions_and_dispatch(&mut world);
        ensure_detail_dialog_overlay(&mut world);

        let first_dialog = world
            .query_filtered::<Entity, With<PixivDetailDialog>>()
            .iter(&world)
            .next()
            .expect("detail dialog should exist before dismissal");
        world.entity_mut(first_dialog).despawn();
        reconcile_detail_dialog_overlay_state(&mut world);

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenIllust(illust));
        drain_ui_actions_and_dispatch(&mut world);
        ensure_detail_dialog_overlay(&mut world);

        let detail_body = world
            .query_filtered::<Entity, With<PixivDetailOverlay>>()
            .iter(&world)
            .next()
            .expect("detail body should exist after reopening");
        let detail_scroll = world.resource::<PixivUiTree>().detail_scroll;
        assert_eq!(
            world
                .get::<ChildOf>(detail_scroll)
                .expect("recreated detail rail scroll should be parented under detail body")
                .parent(),
            detail_body
        );
        let detail_meta_rail = world
            .query_filtered::<Entity, With<PixivDetailMetaRail>>()
            .iter(&world)
            .next()
            .expect("recreated detail metadata rail should exist");
        assert_eq!(
            world
                .get::<ChildOf>(detail_meta_rail)
                .expect("recreated detail metadata rail should be parented under detail scroll")
                .parent(),
            detail_scroll
        );
        let tags_entity = world.resource::<PixivUiTree>().overlay_tags;
        assert!(world.get_entity(tags_entity).is_ok());
        assert_eq!(
            world
                .get::<ChildOf>(tags_entity)
                .expect("recreated tags container should be parented under detail metadata rail")
                .parent(),
            detail_meta_rail
        );
        assert_eq!(world.resource::<OverlayTags>().0.len(), 2);
    }

    #[test]
    fn ensure_detail_dialog_overlay_refreshes_size_when_viewport_changes() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(ViewportMetrics {
            width: 1360.0,
            height: 860.0,
        });
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let illust = world.spawn(mock_illust_with_id(123)).id();
        world.resource_mut::<UiState>().selected_illust = Some(illust);
        ensure_detail_dialog_overlay(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivDetailDialog>>()
            .iter(&world)
            .next()
            .expect("detail dialog should exist after first ensure");
        let initial = world
            .get::<UiDialog>(dialog)
            .expect("detail dialog should exist after first ensure");
        let initial_size = (initial.width, initial.height);

        *world.resource_mut::<ViewportMetrics>() = ViewportMetrics {
            width: 1920.0,
            height: 1200.0,
        };
        ensure_detail_dialog_overlay(&mut world);

        let updated = world
            .get::<UiDialog>(dialog)
            .expect("detail dialog should exist after viewport update");
        let expected = ui::compute_detail_dialog_size(1920.0, 1200.0);

        assert_ne!(initial_size, (updated.width, updated.height));
        assert_eq!(updated.width, Some(expected.0));
        assert_eq!(updated.height, Some(expected.1));
    }

    #[test]
    fn dismissing_spawned_detail_dialog_clears_selected_illust_state() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let illust = world.spawn(mock_illust_with_id(88)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(Entity::PLACEHOLDER, AppAction::OpenIllust(illust));
        drain_ui_actions_and_dispatch(&mut world);
        ensure_detail_dialog_overlay(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivDetailDialog>>()
            .iter(&world)
            .next()
            .expect("detail dialog should exist before dismissal");
        world
            .resource::<UiEventQueue>()
            .push_typed(dialog, picus_core::OverlayUiAction::DismissDialog);
        picus_core::handle_overlay_actions(&mut world);
        drain_ui_actions_and_dispatch(&mut world);

        assert!(world.get_entity(dialog).is_err());

        assert!(world.resource::<UiState>().selected_illust.is_none());
        assert!(world.resource::<OverlayTags>().0.is_empty());
    }

    #[test]
    fn reconcile_detail_dialog_still_clears_state_as_fallback_when_overlay_disappears() {
        let mut world = World::new();
        world.insert_resource(AppI18n::new(parse_locale("en-US")));
        world.insert_resource(UiEventQueue::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(setup);
        schedule.run(&mut world);

        let illust = world.spawn(mock_illust_with_id(99)).id();
        let overlay_parent = world.resource::<PixivUiTree>().overlay_tags;
        let overlay_tag = world
            .spawn((
                OverlayTag {
                    text: "fallback-tag".to_string(),
                },
                ChildOf(overlay_parent),
            ))
            .id();
        world.insert_resource(OverlayTags(vec![overlay_tag]));

        world.resource_mut::<UiState>().selected_illust = Some(illust);
        ensure_detail_dialog_overlay(&mut world);

        let dialog = world
            .query_filtered::<Entity, With<PixivDetailDialog>>()
            .iter(&world)
            .next()
            .expect("detail dialog should exist before fallback reconciliation");
        world.entity_mut(dialog).despawn();

        reconcile_detail_dialog_overlay_state(&mut world);

        assert!(world.resource::<UiState>().selected_illust.is_none());
        assert!(world.resource::<OverlayTags>().0.is_empty());
        assert!(world.get_entity(overlay_tag).is_err());
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
    fn pixiv_auth_locale_keys_exist_in_all_bundles() {
        let locales = [
            include_str!("../assets/locales/en-US/main.ftl"),
            include_str!("../assets/locales/zh-CN/main.ftl"),
            include_str!("../assets/locales/ja-JP/main.ftl"),
        ];

        for content in locales {
            for key in [
                "pixiv-auth-title",
                "pixiv-auth-close",
                "pixiv-auth-show-login",
                "pixiv-auth-logout",
                "pixiv-overlay-artwork-info",
                "pixiv-overlay-author-info",
                "pixiv-overlay-image-info",
                "pixiv-overlay-caption",
                "pixiv-overlay-description-empty",
                "pixiv-status-activation-code-missing",
                "pixiv-status-activation-verifier-missing",
                "pixiv-status-activation-exchange-started",
                "pixiv-status-logged-out",
                "pixiv-status-logout-persist-clear-failed",
            ] {
                assert!(content.contains(key), "locale bundle missing `{key}`");
            }
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
            auth_dialog_toggle: Entity::PLACEHOLDER,
            account_menu_toggle: Entity::PLACEHOLDER,
            logout: Entity::PLACEHOLDER,
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
            auth_dialog_toggle: Entity::PLACEHOLDER,
            account_menu_toggle: Entity::PLACEHOLDER,
            logout: Entity::PLACEHOLDER,
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
        picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");
    }

    #[test]
    fn pixiv_primary_button_uses_neutral_fluent_tokens() {
        let sheet = picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");

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
        let sheet = picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");

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
    fn pixiv_card_uses_compact_spacing_tokens() {
        let sheet = picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");

        let card = sheet
            .get_class_values("pixiv.card")
            .expect("pixiv.card class should exist");

        let padding = match card.layout.padding.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.card padding should come from a theme token"),
        };
        let gap = match card.layout.gap.as_ref() {
            Some(picus_core::StyleValue::Var(token)) => token.as_str(),
            _ => panic!("pixiv.card gap should come from a theme token"),
        };

        assert_eq!(padding, "space-xs");
        assert_eq!(gap, "space-xs");
    }

    #[test]
    fn pixiv_warn_button_uses_fluent_tokens() {
        let sheet = picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");
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
    fn pixiv_auth_overlay_classes_exist() {
        let sheet = picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
            .expect("embedded pixcus stylesheet should parse");

        for class_name in [
            "pixiv.sidebar.footer",
            "pixiv.auth.dialog",
            "pixiv.auth.dialog.title",
            "pixiv.auth.menu",
            "pixiv.auth.menu.secondary",
        ] {
            assert!(
                sheet.get_class_values(class_name).is_some(),
                "{class_name} class should exist"
            );
        }
    }

    #[test]
    fn sync_font_stack_for_locale_preserves_tokenized_fields() {
        let mut sheet =
            picus_core::parse_stylesheet_ron(include_str!("../assets/themes/pixcus.ron"))
                .expect("embedded pixcus stylesheet should parse");

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
            "pixcus should use lucide icons in app UI"
        );
    }

    #[test]
    fn feed_pagination_initial_state_is_default() {
        let pagination = FeedPagination::default();
        assert!(pagination.next_url.is_none());
        assert!(!pagination.loading);
        assert_eq!(pagination.generation, 0);
    }

    #[test]
    fn feed_seen_ids_deduplicates_illusts() {
        let mut seen = FeedSeenIds::default();
        assert!(seen.0.insert(1));
        assert!(!seen.0.insert(1)); // duplicate
        assert!(seen.0.contains(&1));
    }

    #[test]
    fn network_command_fetch_next_exists() {
        let cmd = NetworkCommand::FetchNext {
            source: NavTab::Home,
            generation: 3,
            url: "https://example.com/next".to_string(),
        };
        match cmd {
            NetworkCommand::FetchNext {
                source,
                generation,
                url,
            } => {
                assert_eq!(source, NavTab::Home);
                assert_eq!(generation, 3);
                assert_eq!(url, "https://example.com/next");
            }
            _ => panic!("FetchNext variant should exist"),
        }
    }

    #[test]
    fn pixiv_response_contains_next_url_field() {
        let json = r#"{
            "illusts": [],
            "next_url": "https://example.com/next"
        }"#;
        let parsed =
            PixivApiClient::decode_json_from_body::<PixivResponse>(reqwest::StatusCode::OK, json)
                .expect("should parse next_url");
        assert_eq!(
            parsed.next_url,
            Some("https://example.com/next".to_string())
        );
    }

    #[test]
    fn pixiv_response_next_url_defaults_to_none() {
        let json = r#"{
            "illusts": []
        }"#;
        let parsed =
            PixivApiClient::decode_json_from_body::<PixivResponse>(reqwest::StatusCode::OK, json)
                .expect("should parse without next_url");
        assert!(parsed.next_url.is_none());
    }
}

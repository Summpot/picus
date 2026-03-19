use super::*;

use picus_core::bevy_math::Vec2;
use picus_core::{UiPopover, UiScrollView, spawn_popover_in_overlay_root};

use super::actions::{request_next_feed_page, sync_feed_scroll_viewport};

#[cfg(target_os = "macos")]
pub(super) fn pixiv_macos_bundle_config() -> MacosBundleConfig {
    MacosBundleConfig::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Info.plist"))
}

const ACCOUNT_MENU_WIDTH_PX: f64 = 132.0;
const ACCOUNT_MENU_HEIGHT_HINT_PX: f64 = 56.0;

pub(super) fn ensure_task_pool_initialized() {
    let _ = IoTaskPool::get_or_init(TaskPool::new);
    let _ = AsyncComputeTaskPool::get_or_init(TaskPool::new);
}

pub(super) fn register_bridge_fonts(app: &mut App) {
    app.register_xilem_font(SyncAssetSource::Bytes(include_bytes!(
        "../../../../assets/fonts/NotoSans-Regular.ttf",
    )));
    app.register_xilem_font(SyncAssetSource::Bytes(include_bytes!(
        "../../../../assets/fonts/NotoSansCJKsc-Regular.otf",
    )));
    app.register_xilem_font(SyncAssetSource::Bytes(include_bytes!(
        "../../../../assets/fonts/NotoSansCJKjp-Regular.otf",
    )));
    app.register_xilem_font(SyncAssetSource::Bytes(include_bytes!(
        "../../../../assets/fonts/NotoSansCJKtc-Regular.otf",
    )));
    app.register_xilem_font(SyncAssetSource::Bytes(include_bytes!(
        "../../../../assets/fonts/NotoSansCJKkr-Regular.otf",
    )));
}

fn spawn_ui_component_entity(commands: &mut Commands, classes: &[&str]) -> Entity {
    commands
        .spawn((StyleClass(
            classes.iter().map(|class| (*class).to_string()).collect(),
        ),))
        .id()
}

fn spawn_bound_text_input(
    commands: &mut Commands,
    parent: Entity,
    value: impl Into<String>,
    placeholder: impl Into<String>,
) -> Entity {
    commands
        .spawn((
            UiTextInput::new(value).with_placeholder(placeholder),
            StyleClass(vec!["pixiv.text-input".to_string()]),
            ChildOf(parent),
        ))
        .id()
}

fn spawn_bound_text_input_world(
    world: &mut World,
    parent: Entity,
    value: impl Into<String>,
    placeholder: impl Into<String>,
) -> Entity {
    world
        .spawn((
            UiTextInput::new(value).with_placeholder(placeholder),
            StyleClass(vec!["pixiv.text-input".to_string()]),
            ChildOf(parent),
        ))
        .id()
}

fn auth_dialog_entity(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<Entity, With<PixivAuthDialog>>();
    query.iter(world).next()
}

fn account_menu_entity(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<Entity, With<PixivAccountMenu>>();
    query.iter(world).next()
}

pub(super) fn account_menu_popover(anchor: Entity) -> UiPopover {
    UiPopover::new(anchor)
        .with_placement(OverlayPlacement::TopEnd)
        .with_auto_flip_placement(true)
        .with_fixed_size(ACCOUNT_MENU_WIDTH_PX, ACCOUNT_MENU_HEIGHT_HINT_PX)
}

pub(super) fn dismiss_account_menu_overlay(world: &mut World) {
    if let Some(entity) = account_menu_entity(world)
        && world.get_entity(entity).is_ok()
    {
        world.entity_mut(entity).despawn();
    }
}

pub(super) fn dismiss_auth_dialog_overlay(world: &mut World) {
    if let Some(entity) = auth_dialog_entity(world)
        && world.get_entity(entity).is_ok()
    {
        world.entity_mut(entity).despawn();
    }

    if let Some(mut ui_components) = world.get_resource_mut::<PixivUiComponents>() {
        ui_components.code_verifier_input = Entity::PLACEHOLDER;
        ui_components.auth_code_input = Entity::PLACEHOLDER;
        ui_components.refresh_token_input = Entity::PLACEHOLDER;
    }
}

pub(super) fn reconcile_auth_dialog_overlay_state(world: &mut World) {
    let has_dialog = auth_dialog_entity(world).is_some();
    if has_dialog {
        return;
    }

    if let Some(mut auth) = world.get_resource_mut::<AuthState>() {
        auth.login_dialog_open = false;
    }

    if let Some(mut ui_components) = world.get_resource_mut::<PixivUiComponents>() {
        ui_components.code_verifier_input = Entity::PLACEHOLDER;
        ui_components.auth_code_input = Entity::PLACEHOLDER;
        ui_components.refresh_token_input = Entity::PLACEHOLDER;
    }
}

pub(super) fn reconcile_account_menu_overlay_state(world: &mut World) {
    if account_menu_entity(world).is_some() {
        return;
    }

    if let Some(mut auth) = world.get_resource_mut::<AuthState>() {
        auth.account_menu_open = false;
    }
}

pub(super) fn ensure_auth_dialog_overlay(world: &mut World) {
    let should_show = world
        .get_resource::<AuthState>()
        .is_some_and(|auth| auth.session.is_none() && auth.login_dialog_open);
    if !should_show {
        dismiss_auth_dialog_overlay(world);
        return;
    }

    if auth_dialog_entity(world).is_some() {
        sync_bound_text_inputs(world);
        return;
    }

    let dialog = spawn_in_overlay_root(
        world,
        (
            UiDialog::new(tr(world, "pixiv.auth.title", "Pixiv Login"), ""),
            StyleClass(vec![
                "pixiv.overlay".to_string(),
                "pixiv.auth.dialog".to_string(),
            ]),
            PixivAuthDialog,
        ),
    );
    let form = world.spawn((PixivAuthDialogForm, ChildOf(dialog))).id();

    let code_verifier_input = spawn_bound_text_input_world(
        world,
        form,
        "",
        tr(world, "pixiv.auth.placeholder.pkce", "PKCE code_verifier"),
    );
    let auth_code_input = spawn_bound_text_input_world(
        world,
        form,
        "",
        tr(world, "pixiv.auth.placeholder.code", "Auth code"),
    );
    let refresh_token_seed = world
        .get_resource::<AuthState>()
        .map(|auth| auth.refresh_token_input.clone())
        .unwrap_or_default();
    let refresh_token_input = spawn_bound_text_input_world(
        world,
        form,
        refresh_token_seed,
        tr(
            world,
            "pixiv.auth.placeholder.refresh_token",
            "Refresh token",
        ),
    );

    if let Some(mut ui_components) = world.get_resource_mut::<PixivUiComponents>() {
        ui_components.code_verifier_input = code_verifier_input;
        ui_components.auth_code_input = auth_code_input;
        ui_components.refresh_token_input = refresh_token_input;
    }

    sync_bound_text_inputs(world);
}

pub(super) fn ensure_account_menu_overlay(world: &mut World) {
    let should_show = world
        .get_resource::<AuthState>()
        .is_some_and(|auth| auth.session.is_some() && auth.account_menu_open);
    if !should_show {
        dismiss_account_menu_overlay(world);
        return;
    }

    if account_menu_entity(world).is_some() {
        return;
    }

    let Some(account_toggle) = world
        .get_resource::<PixivUiComponents>()
        .map(|ui| ui.account_menu_toggle)
    else {
        return;
    };

    let _ = spawn_popover_in_overlay_root(
        world,
        (
            PixivAccountMenu,
            StyleClass(vec![
                "pixiv.overlay".to_string(),
                "pixiv.auth.menu".to_string(),
            ]),
        ),
        account_menu_popover(account_toggle),
    );
}

fn set_text_input_component_value(world: &mut World, entity: Entity, value: &str) {
    if let Some(mut input) = world.get_mut::<UiTextInput>(entity)
        && input.value != value
    {
        input.value = value.to_string();
    }
}

fn set_text_input_component_placeholder(world: &mut World, entity: Entity, placeholder: &str) {
    if let Some(mut input) = world.get_mut::<UiTextInput>(entity)
        && input.placeholder != placeholder
    {
        input.placeholder = placeholder.to_string();
    }
}

pub(super) fn sync_bound_text_inputs(world: &mut World) {
    let Some(ui_components) = world.get_resource::<PixivUiComponents>().copied() else {
        return;
    };
    let search_text = world
        .get_resource::<UiState>()
        .map(|ui| ui.search_text.clone())
        .unwrap_or_default();
    let (code_verifier_input, auth_code_input, refresh_token_input) = world
        .get_resource::<AuthState>()
        .map(|auth| {
            (
                auth.code_verifier_input.clone(),
                auth.auth_code_input.clone(),
                auth.refresh_token_input.clone(),
            )
        })
        .unwrap_or_else(|| (String::new(), String::new(), String::new()));
    let placeholders = [
        (
            ui_components.code_verifier_input,
            tr(world, "pixiv.auth.placeholder.pkce", "PKCE code_verifier"),
        ),
        (
            ui_components.auth_code_input,
            tr(world, "pixiv.auth.placeholder.code", "Auth code"),
        ),
        (
            ui_components.refresh_token_input,
            tr(
                world,
                "pixiv.auth.placeholder.refresh_token",
                "Refresh token",
            ),
        ),
        (
            ui_components.search_input,
            tr(world, "pixiv.search.placeholder", "Search illust keyword"),
        ),
    ];

    set_text_input_component_value(world, ui_components.search_input, &search_text);
    set_text_input_component_value(
        world,
        ui_components.code_verifier_input,
        &code_verifier_input,
    );
    set_text_input_component_value(world, ui_components.auth_code_input, &auth_code_input);
    set_text_input_component_value(
        world,
        ui_components.refresh_token_input,
        &refresh_token_input,
    );

    for (entity, placeholder) in placeholders {
        set_text_input_component_placeholder(world, entity, &placeholder);
    }
}

pub(super) fn setup(mut commands: Commands, i18n: Res<AppI18n>) {
    ensure_task_pool_initialized();

    let restored_auth = persistence::load_auth_state()
        .map_err(|error| {
            eprintln!("pixiv credential restore failed: {error}");
            error
        })
        .ok()
        .flatten();
    let restored_session = restored_auth.as_ref().map(|auth| auth.session.clone());
    let restored_user_summary = restored_auth
        .as_ref()
        .and_then(|auth| auth.user_summary.clone());

    let (cmd_tx, cmd_rx) = unbounded::<NetworkCommand>();
    let (result_tx, result_rx) = unbounded::<NetworkResult>();
    let (image_cmd_tx, image_cmd_rx) = unbounded::<ImageCommand>();
    let (image_result_tx, image_result_rx) = unbounded::<ImageResult>();

    commands.insert_resource(NetworkBridge {
        cmd_tx: cmd_tx.clone(),
        cmd_rx,
        result_tx,
        result_rx,
    });
    commands.insert_resource(ImageBridge {
        cmd_tx: image_cmd_tx,
        cmd_rx: image_cmd_rx,
        result_tx: image_result_tx,
        result_rx: image_result_rx,
    });

    commands.insert_resource(UiState {
        status_line: if restored_auth.is_some() {
            "Booting Pixiv MVP… restored saved credentials, refreshing token…".to_string()
        } else {
            "Booting Pixiv MVP…".to_string()
        },
        ..UiState::default()
    });
    commands.insert_resource(AuthState {
        session: restored_session.clone(),
        user_summary: restored_user_summary,
        refresh_token_input: restored_session
            .as_ref()
            .map(|session| session.refresh_token.clone())
            .unwrap_or_default(),
        ..AuthState::default()
    });
    commands.insert_resource(FeedOrder::default());
    commands.insert_resource(FeedPagination::default());
    commands.insert_resource(FeedSeenIds::default());
    commands.insert_resource(OverlayTags::default());
    commands.insert_resource(ResponsePanelState::default());
    commands.insert_resource(ViewportMetrics::default());
    commands.insert_resource(PixivApiClient::default());
    commands.insert_resource(AuthAvatarVisual::default());
    commands.insert_resource(Assets::<BevyImage>::default());

    let mut ui_components = PixivUiComponents {
        toggle_sidebar: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.sidebar"],
        ),
        locale_combo: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.sidebar"],
        ),
        auth_dialog_toggle: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        account_menu_toggle: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        logout: spawn_ui_component_entity(&mut commands, &["pixiv.button", "pixiv.button.warn"]),
        code_verifier_input: Entity::PLACEHOLDER,
        auth_code_input: Entity::PLACEHOLDER,
        refresh_token_input: Entity::PLACEHOLDER,
        search_input: Entity::PLACEHOLDER,
        home_tab: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        rankings_tab: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        manga_tab: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        novels_tab: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        search_tab: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.subtle"],
        ),
        open_browser_login: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        exchange_auth_code: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        refresh_token: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        search_submit: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        copy_response: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.primary"],
        ),
        clear_response: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.warn"],
        ),
        close_overlay: spawn_ui_component_entity(
            &mut commands,
            &["pixiv.button", "pixiv.button.warn"],
        ),
    };
    let root = commands
        .spawn((
            UiRoot,
            PixivRoot,
            StyleClass(vec!["pixiv.root".to_string()]),
        ))
        .id();

    commands.spawn((UiThemePicker::fluent(), ChildOf(root)));

    let sidebar = commands
        .spawn((
            PixivSidebar,
            StyleClass(vec!["pixiv.sidebar".to_string()]),
            ChildOf(root),
        ))
        .id();

    let locale_options = vec![
        UiComboOption::new("en-US", "English"),
        UiComboOption::new("zh-CN", "简体中文"),
        UiComboOption::new("ja-JP", "日本語"),
    ];
    let active_locale_tag = i18n.active_locale.to_string();
    let selected_locale = locale_options
        .iter()
        .position(|option| {
            option
                .value
                .eq_ignore_ascii_case(active_locale_tag.as_str())
        })
        .unwrap_or(0);

    let mut locale_combo = UiComboBox::new(locale_options).with_placeholder("Language");
    locale_combo.selected = selected_locale;

    commands
        .entity(ui_components.locale_combo)
        .insert((locale_combo, ChildOf(sidebar)));

    let _auth_panel = commands
        .spawn((
            PixivAuthPanel,
            StyleClass(vec!["pixiv.sidebar.footer".to_string()]),
            ChildOf(sidebar),
        ))
        .id();

    let main_column = commands.spawn((PixivMainColumn, ChildOf(root))).id();

    commands.spawn((PixivResponsePanel, ChildOf(main_column)));
    let search_panel = commands
        .spawn((PixivSearchPanel, ChildOf(main_column)))
        .id();

    ui_components.search_input =
        spawn_bound_text_input(&mut commands, search_panel, "", "Search illust keyword");

    commands.insert_resource(ui_components);
    commands.queue(sync_bound_text_inputs);

    let feed_scroll = commands
        .spawn((
            UiScrollView::new(Vec2::new(1100.0, 520.0), Vec2::new(1100.0, 1400.0))
                .with_vertical_scrollbar(true)
                .with_horizontal_scrollbar(false),
            ChildOf(main_column),
        ))
        .id();

    let home_feed = commands.spawn((PixivHomeFeed, ChildOf(feed_scroll))).id();

    let restored_avatar_url = restored_auth
        .as_ref()
        .and_then(|auth| auth.user_summary.as_ref())
        .and_then(|summary| summary.avatar_url.clone())
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));

    commands.queue(move |world: &mut World| {
        let detail_overlay = spawn_in_overlay_root(
            world,
            (
                PixivDetailOverlay,
                StyleClass(vec!["pixiv.overlay".to_string()]),
                OverlayState {
                    is_modal: true,
                    anchor: None,
                },
                OverlayConfig {
                    placement: OverlayPlacement::Center,
                    anchor: None,
                    auto_flip: false,
                },
            ),
        );

        let overlay_tags = world
            .spawn((PixivOverlayTags, ChildOf(detail_overlay)))
            .id();

        if let Some(url) = restored_avatar_url.clone() {
            world.resource_mut::<AuthAvatarVisual>().requested_url = Some(url.clone());
            let _ = world
                .resource::<ImageBridge>()
                .cmd_tx
                .send(ImageCommand::Download {
                    target: ImageTarget::AuthAvatar,
                    kind: ImageKind::Avatar,
                    url,
                });
        }

        world.insert_resource(PixivUiTree {
            feed_scroll,
            home_feed,
            overlay_tags,
        });
    });

    let _ = cmd_tx.send(NetworkCommand::DiscoverIdp);

    if let Some(session) = restored_session {
        let _ = cmd_tx.send(NetworkCommand::Refresh {
            refresh_token: session.refresh_token,
        });
    }
}

pub(super) fn setup_styles(mut sheet: ResMut<StyleSheet>, i18n: Option<Res<AppI18n>>) {
    let font_stack = i18n
        .as_ref()
        .map(|current| current.get_font_stack())
        .filter(|stack| !stack.is_empty());

    sync_font_stack_for_locale(&mut sheet, font_stack.as_deref());
}

picus_core::impl_ui_component_template!(PixivRoot, project_root);
picus_core::impl_ui_component_template!(PixivSidebar, project_sidebar);
picus_core::impl_ui_component_template!(PixivMainColumn, project_main_column);
picus_core::impl_ui_component_template!(PixivAuthPanel, project_auth_panel);
picus_core::impl_ui_component_template!(PixivAuthDialogForm, project_auth_dialog_form);
picus_core::impl_ui_component_template!(PixivAccountMenu, project_account_menu);
picus_core::impl_ui_component_template!(PixivResponsePanel, project_response_panel);
picus_core::impl_ui_component_template!(PixivSearchPanel, project_search_panel);
picus_core::impl_ui_component_template!(PixivHomeFeed, project_home_feed);
picus_core::impl_ui_component_template!(PixivIllustCard, project_illust_card);
picus_core::impl_ui_component_template!(PixivDetailOverlay, project_detail_overlay);
picus_core::impl_ui_component_template!(PixivOverlayTags, project_overlay_tags);
picus_core::impl_ui_component_template!(OverlayTag, project_overlay_tag);

pub(super) fn build_app(mut activation_service: Option<ActivationService>) -> App {
    ensure_task_pool_initialized();
    init_logging();

    let mut app = App::new();
    register_bridge_fonts(&mut app);

    if let Some(mut service) = activation_service.take() {
        let startup_uris = service.take_startup_uris();
        #[cfg(not(target_os = "macos"))]
        app.insert_resource(ActivationBridge {
            service: Mutex::new(service),
            startup_uris,
        });

        #[cfg(target_os = "macos")]
        app.insert_non_send_resource(ActivationBridge {
            service,
            startup_uris,
        });
    }

    app.add_plugins((
        EmbeddedAssetPlugin {
            mode: PluginMode::ReplaceDefault,
        },
        AssetPlugin::default(),
        TextPlugin,
        PicusPlugin,
    ))
    .load_style_sheet_ron(include_str!("../../assets/themes/pixiv_client.ron"))
    .insert_resource(AppI18n::new(parse_locale("en-US")))
    .register_i18n_bundle(
        "en-US",
        SyncTextSource::String(include_str!("../../assets/locales/en-US/main.ftl")),
        vec![
            "Inter",
            "Noto Sans CJK SC",
            "Noto Sans CJK JP",
            "Noto Sans CJK TC",
            "Noto Sans CJK KR",
            "sans-serif",
        ],
    )
    .register_i18n_bundle(
        "zh-CN",
        SyncTextSource::String(include_str!("../../assets/locales/zh-CN/main.ftl")),
        vec![
            "Inter",
            "Noto Sans CJK SC",
            "Noto Sans CJK JP",
            "Noto Sans CJK TC",
            "Noto Sans CJK KR",
            "sans-serif",
        ],
    )
    .register_i18n_bundle(
        "ja-JP",
        SyncTextSource::String(include_str!("../../assets/locales/ja-JP/main.ftl")),
        vec![
            "Inter",
            "Noto Sans CJK JP",
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "Noto Sans CJK KR",
            "sans-serif",
        ],
    )
    .register_ui_component::<PixivRoot>()
    .register_ui_component::<PixivSidebar>()
    .register_ui_component::<PixivMainColumn>()
    .register_ui_component::<PixivAuthPanel>()
    .register_ui_component::<PixivAuthDialogForm>()
    .register_ui_component::<PixivAccountMenu>()
    .register_ui_component::<PixivResponsePanel>()
    .register_ui_component::<PixivSearchPanel>()
    .register_ui_component::<PixivHomeFeed>()
    .register_ui_component::<PixivIllustCard>()
    .register_ui_component::<PixivDetailOverlay>()
    .register_ui_component::<PixivOverlayTags>()
    .register_ui_component::<OverlayTag>()
    .add_tween_systems(Update, component_tween_system::<CardAnimLens>())
    .add_systems(Startup, (setup_styles, setup))
    .add_systems(
        Update,
        (
            drain_ui_actions_and_dispatch
                .after(picus_core::handle_widget_actions)
                .after(picus_core::handle_overlay_actions),
            poll_activation_messages,
            track_viewport_metrics,
            sync_feed_scroll_viewport,
            request_next_feed_page,
            spawn_network_tasks,
            apply_network_results,
            spawn_image_tasks,
            apply_image_results,
            reconcile_auth_dialog_overlay_state,
            reconcile_account_menu_overlay_state,
        )
            .chain(),
    );
    app
}

pub fn run() -> std::result::Result<(), EventLoopError> {
    let mut protocol = ProtocolRegistration::new("pixiv", "Pixiv OAuth callback", None);
    #[cfg(target_os = "macos")]
    {
        protocol = protocol.with_macos_bundle(pixiv_macos_bundle_config());
    }

    let activation_config = ActivationConfig::new(PIXIV_ACTIVATION_APP_ID).with_protocol(protocol);

    let activation_service = match bootstrap(activation_config) {
        Ok(BootstrapOutcome::Primary(service)) => Some(service),
        Ok(BootstrapOutcome::SecondaryForwarded) => return Ok(()),
        Err(error) => {
            eprintln!("activation bootstrap failed: {error}");
            None
        }
    };

    run_app_with_window_options(build_app(activation_service), "Pixiv Desktop", |options| {
        options.with_initial_inner_size(LogicalSize::new(1360.0, 860.0))
    })
}

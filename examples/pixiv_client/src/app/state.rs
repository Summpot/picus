use super::*;

pub(super) const CARD_MIN_WIDTH: f64 = 260.0;
pub(super) const CARD_ROW_GAP: f64 = 10.0;
pub(super) const MAX_CARD_COLUMNS: usize = 6;
pub(super) const SIDEBAR_EXPANDED_WIDTH: f64 = 208.0;
pub(super) const SIDEBAR_COLLAPSED_WIDTH: f64 = 100.0;
pub(super) const RESPONSE_PANEL_HEIGHT: f64 = 180.0;
pub(super) const PIXIV_AUTH_TOKEN_FALLBACK: &str = "https://oauth.secure.pixiv.net/auth/token";
pub(super) const PIXIV_WEB_REDIRECT_FALLBACK: &str =
    "https://app-api.pixiv.net/web/v1/users/auth/pixiv/callback";
pub(super) const PIXIV_ACTIVATION_APP_ID: &str = "bevy-xilem-example-pixiv-client";

pub(super) fn parse_locale(tag: &str) -> LanguageIdentifier {
    tag.parse()
        .unwrap_or_else(|_| panic!("locale `{tag}` should parse"))
}

pub(super) fn locale_badge(locale: &LanguageIdentifier) -> &'static str {
    if locale.language.as_str() == "ja" {
        "日本語"
    } else if locale.language.as_str() == "zh"
        && locale
            .region
            .is_some_and(|region| region.as_str().eq_ignore_ascii_case("CN"))
    {
        "简体中文"
    } else {
        "English"
    }
}

pub(super) fn tr(world: &World, key: &str, fallback: &str) -> String {
    let Some(i18n) = world.get_resource::<AppI18n>() else {
        return fallback.to_string();
    };

    let translated = i18n.translate(key);
    if translated != key {
        return translated;
    }

    if key.contains('.') {
        let normalized = key.replace('.', "-");
        let normalized_translated = i18n.translate(normalized.as_str());
        if normalized_translated != normalized {
            return normalized_translated;
        }
    }

    fallback.to_string()
}

pub(super) fn set_status(world: &mut World, message: impl Into<String>) {
    world.resource_mut::<UiState>().status_line = message.into();
}

pub(super) fn set_status_key(world: &mut World, key: &str, fallback: &str) {
    let message = {
        let world_ref: &World = world;
        tr(world_ref, key, fallback)
    };
    set_status(world, message);
}

pub(super) fn sync_font_stack_for_locale(sheet: &mut StyleSheet, stack: Option<&[String]>) {
    for class_name in [
        "pixiv.root",
        "pixiv.sidebar",
        "pixiv.sidebar.title",
        "pixiv.sidebar.button",
        "pixiv.auth-panel",
        "pixiv.text-input",
        "pixiv.button",
        "pixiv.primary-btn",
        "pixiv.card",
        "pixiv.tag",
        "pixiv.overlay",
    ] {
        if let Some(existing) = sheet.get_class_values(class_name).cloned() {
            let mut updated = existing;
            updated.font_family = stack.map(|stack| StyleValue::value(stack.to_vec()));
            sheet.set_class_values(class_name, updated);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum NavTab {
    #[default]
    Home,
    Rankings,
    Manga,
    Novels,
    Search,
}

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct UiState {
    pub active_tab: NavTab,
    pub sidebar_collapsed: bool,
    pub search_text: String,
    pub selected_illust: Option<Entity>,
    pub status_line: String,
}

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct AuthState {
    pub idp_urls: Option<IdpUrlResponse>,
    pub session: Option<AuthSession>,
    pub code_verifier_input: String,
    pub auth_code_input: String,
    pub refresh_token_input: String,
}

#[derive(Resource, Default)]
pub(super) struct FeedOrder(pub Vec<Entity>);

#[derive(Resource, Default)]
pub(super) struct OverlayTags(pub Vec<Entity>);

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct ResponsePanelState {
    pub title: String,
    pub content: String,
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct ViewportMetrics {
    pub width: f32,
    pub height: f32,
}

impl Default for ViewportMetrics {
    fn default() -> Self {
        Self {
            width: 1360.0,
            height: 860.0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct PixivUiComponents {
    pub toggle_sidebar: Entity,
    pub locale_combo: Entity,
    pub code_verifier_input: Entity,
    pub auth_code_input: Entity,
    pub refresh_token_input: Entity,
    pub search_input: Entity,
    pub home_tab: Entity,
    pub rankings_tab: Entity,
    pub manga_tab: Entity,
    pub novels_tab: Entity,
    pub search_tab: Entity,
    pub open_browser_login: Entity,
    pub exchange_auth_code: Entity,
    pub refresh_token: Entity,
    pub search_submit: Entity,
    pub copy_response: Entity,
    pub clear_response: Entity,
    pub close_overlay: Entity,
}

#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct PixivUiTree {
    pub feed_scroll: Entity,
    pub home_feed: Entity,
    pub overlay_tags: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivRoot;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivSidebar;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivMainColumn;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivAuthPanel;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivResponsePanel;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivSearchPanel;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivHomeFeed;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivIllustCard;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivDetailOverlay;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PixivOverlayTags;

#[derive(Component, Debug, Clone)]
pub(super) struct OverlayTag {
    pub text: String,
}

#[derive(Component, Debug, Clone, Default)]
pub(super) struct IllustVisual {
    pub thumb_ui: Option<ImageData>,
    pub avatar_ui: Option<ImageData>,
    pub high_res_ui: Option<ImageData>,
    pub thumb_handle: Option<Handle<BevyImage>>,
    pub avatar_handle: Option<Handle<BevyImage>>,
    pub high_res_handle: Option<Handle<BevyImage>>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(super) struct CardAnimState {
    pub card_scale: f32,
    pub image_brightness: f32,
    pub heart_scale: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct IllustActionEntities {
    pub open_thumbnail: Entity,
    pub bookmark: Entity,
}

impl Default for CardAnimState {
    fn default() -> Self {
        Self {
            card_scale: 1.0,
            image_brightness: 1.0,
            heart_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ImageKind {
    Thumb,
    Avatar,
    HighRes,
}

#[derive(Debug, Clone)]
pub(super) enum AppAction {
    ToggleSidebar,
    SetTab(NavTab),
    #[allow(dead_code)]
    SetSearchText(String),
    SubmitSearch,
    OpenIllust(Entity),
    CloseIllust,
    Bookmark(Entity),
    SearchByTag(String),
    CopyResponseBody,
    ClearResponseBody,
    OpenBrowserLogin,
    ExchangeAuthCode,
    RefreshToken,
}

#[derive(Debug, Clone)]
pub(super) enum NetworkCommand {
    DiscoverIdp,
    ExchangeCode { code: String, code_verifier: String },
    Refresh { refresh_token: String },
    FetchHome,
    FetchRanking,
    FetchManga,
    FetchNovels,
    Search { word: String },
    Bookmark { illust_id: u64 },
}

#[derive(Debug, Clone)]
pub(super) enum NetworkResult {
    IdpDiscovered(IdpUrlResponse),
    Authenticated(AuthSession),
    FeedLoaded {
        source: NavTab,
        payload: PixivResponse,
    },
    BookmarkDone {
        illust_id: u64,
    },
    Error {
        summary: String,
        details: String,
    },
}

#[derive(Debug, Clone)]
pub(super) enum ImageCommand {
    Download {
        entity: Entity,
        kind: ImageKind,
        url: String,
    },
}

#[derive(Debug, Clone)]
pub(super) enum ImageResult {
    Loaded {
        entity: Entity,
        kind: ImageKind,
        decoded: DecodedImageRgba,
    },
    Failed {
        entity: Entity,
        kind: ImageKind,
        error: String,
    },
}

#[derive(Resource)]
pub(super) struct NetworkBridge {
    pub cmd_tx: Sender<NetworkCommand>,
    pub cmd_rx: Receiver<NetworkCommand>,
    pub result_tx: Sender<NetworkResult>,
    pub result_rx: Receiver<NetworkResult>,
}

#[derive(Resource)]
pub(super) struct ImageBridge {
    pub cmd_tx: Sender<ImageCommand>,
    pub cmd_rx: Receiver<ImageCommand>,
    pub result_tx: Sender<ImageResult>,
    pub result_rx: Receiver<ImageResult>,
}

#[cfg(not(target_os = "macos"))]
#[derive(Resource)]
pub(super) struct ActivationBridge {
    pub service: Mutex<ActivationService>,
    pub startup_uris: Vec<String>,
}

#[cfg(target_os = "macos")]
pub(super) struct ActivationBridge {
    pub service: ActivationService,
    pub startup_uris: Vec<String>,
}

#[derive(Clone, Copy)]
pub(super) struct CardAnimLens {
    pub start: CardAnimState,
    pub end: CardAnimState,
}

impl Interpolator for CardAnimLens {
    type Item = CardAnimState;

    fn interpolate(&self, target: &mut Self::Item, ratio: f32, _previous_value: f32) {
        target.card_scale =
            self.start.card_scale + (self.end.card_scale - self.start.card_scale) * ratio;
        target.image_brightness = self.start.image_brightness
            + (self.end.image_brightness - self.start.image_brightness) * ratio;
        target.heart_scale =
            self.start.heart_scale + (self.end.heart_scale - self.start.heart_scale) * ratio;
    }
}

pub(super) fn spawn_card_tween(
    world: &mut World,
    entity: Entity,
    start: CardAnimState,
    end: CardAnimState,
    duration_ms: u64,
    ease: EaseKind,
) {
    let duration = Duration::from_millis(duration_ms);
    world.entity_mut(entity).insert((
        TimeSpan::try_from(Duration::ZERO..duration)
            .expect("card tween duration range should be valid"),
        ease,
        ComponentTween::new_target(entity, CardAnimLens { start, end }),
        TimeRunner::new(duration),
        TimeContext::<()>::default(),
    ));
}

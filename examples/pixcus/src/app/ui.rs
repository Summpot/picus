use super::*;

use picus_core::{
    UiScrollView,
    bevy_math::Vec2,
    opaque_hitbox_for_entity,
    xilem::{
        masonry::layout::UnitPoint,
        view::{transformed, zstack},
    },
};

const FEED_OVERSCAN_Y: f64 = 240.0;
const FEED_BASE_CHROME_HEIGHT: f64 = 164.0;
const FEED_SEARCH_PANEL_HEIGHT: f64 = 56.0;
const FEED_RESPONSE_PANEL_SPACING: f64 = 18.0;
const DETAIL_DIALOG_MIN_WIDTH: f64 = 900.0;
const DETAIL_DIALOG_MAX_WIDTH: f64 = 1440.0;
const DETAIL_DIALOG_MIN_HEIGHT: f64 = 620.0;
const DETAIL_DIALOG_MAX_HEIGHT: f64 = 1040.0;
const DETAIL_DIALOG_VIEWPORT_WIDTH_RATIO: f64 = 0.92;
const DETAIL_DIALOG_VIEWPORT_HEIGHT_RATIO: f64 = 0.9;
const DETAIL_DIALOG_VIEWPORT_MARGIN_X: f64 = 28.0;
const DETAIL_DIALOG_VIEWPORT_MARGIN_Y: f64 = 24.0;
const DETAIL_RAIL_MIN_WIDTH: f64 = 280.0;
const DETAIL_RAIL_COMPACT_MIN_WIDTH: f64 = 220.0;
const DETAIL_RAIL_MAX_WIDTH: f64 = 420.0;
const DETAIL_RAIL_WIDTH_RATIO: f64 = 0.29;
const DETAIL_RAIL_VIEWPORT_CHROME_HEIGHT: f64 = 88.0;
const DETAIL_RAIL_VIEWPORT_MIN_HEIGHT: f64 = 180.0;

fn empty_ui() -> UiView {
    Arc::new(label(""))
}

pub(super) fn compute_detail_dialog_size(viewport_width: f64, viewport_height: f64) -> (f64, f64) {
    let available_width = (viewport_width - DETAIL_DIALOG_VIEWPORT_MARGIN_X * 2.0).max(320.0);
    let available_height = (viewport_height - DETAIL_DIALOG_VIEWPORT_MARGIN_Y * 2.0).max(240.0);

    let preferred_width = (viewport_width * DETAIL_DIALOG_VIEWPORT_WIDTH_RATIO)
        .clamp(DETAIL_DIALOG_MIN_WIDTH, DETAIL_DIALOG_MAX_WIDTH);
    let preferred_height = (viewport_height * DETAIL_DIALOG_VIEWPORT_HEIGHT_RATIO)
        .clamp(DETAIL_DIALOG_MIN_HEIGHT, DETAIL_DIALOG_MAX_HEIGHT);

    let width = if available_width >= DETAIL_DIALOG_MIN_WIDTH {
        preferred_width.min(available_width)
    } else {
        available_width
    };
    let height = if available_height >= DETAIL_DIALOG_MIN_HEIGHT {
        preferred_height.min(available_height)
    } else {
        available_height
    };

    (width, height)
}

pub(super) fn compute_detail_meta_rail_width(dialog_width: f64) -> f64 {
    let max_allowed = (dialog_width * 0.4).max(DETAIL_RAIL_COMPACT_MIN_WIDTH);
    (dialog_width * DETAIL_RAIL_WIDTH_RATIO).clamp(
        DETAIL_RAIL_MIN_WIDTH.min(max_allowed),
        DETAIL_RAIL_MAX_WIDTH.min(max_allowed),
    )
}

pub(super) fn compute_detail_meta_rail_viewport_height(dialog_height: f64) -> f64 {
    (dialog_height - DETAIL_RAIL_VIEWPORT_CHROME_HEIGHT).max(DETAIL_RAIL_VIEWPORT_MIN_HEIGHT)
}

fn detail_dialog_size_for_world(world: &World) -> (f64, f64) {
    let viewport = world
        .get_resource::<ViewportMetrics>()
        .copied()
        .unwrap_or_default();
    compute_detail_dialog_size(viewport.width as f64, viewport.height as f64)
}

fn detail_dialog_size_for_overlay(world: &World, entity: Entity) -> (f64, f64) {
    let parent = world.get::<ChildOf>(entity).map(|child| child.parent());
    if let Some(parent) = parent
        && let Some(dialog) = world.get::<UiDialog>(parent)
        && let (Some(width), Some(height)) = (dialog.width, dialog.height)
    {
        return (width, height);
    }

    detail_dialog_size_for_world(world)
}

#[derive(Debug, Clone)]
pub(super) struct JustifiedRowItem {
    pub index: usize,
    pub x: f64,
    pub width: f64,
    #[allow(dead_code)]
    pub aspect_ratio: f64,
}

#[derive(Debug, Clone)]
pub(super) struct JustifiedRow {
    pub y: f64,
    pub height: f64,
    #[allow(dead_code)]
    pub width: f64,
    #[allow(dead_code)]
    pub justified: bool,
    pub items: Vec<JustifiedRowItem>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct JustifiedLayout {
    pub rows: Vec<JustifiedRow>,
    pub content_width: f64,
    pub content_height: f64,
}

fn normalize_aspect_ratio(aspect_ratio: f64) -> f64 {
    if aspect_ratio.is_finite() && aspect_ratio > 0.0 {
        aspect_ratio.clamp(0.35, 3.2)
    } else {
        0.62
    }
}

pub(super) fn illust_aspect_ratio(world: &World, entity: Entity) -> f64 {
    if let Some(illust) = world.get::<Illust>(entity)
        && illust.width > 0
        && illust.height > 0
    {
        return normalize_aspect_ratio(illust.width as f64 / illust.height as f64);
    }

    if let Some(visual) = world.get::<IllustVisual>(entity)
        && let Some(thumb) = visual.thumb_ui.as_ref()
        && thumb.width > 0
        && thumb.height > 0
    {
        return normalize_aspect_ratio(thumb.width as f64 / thumb.height as f64);
    }

    0.62
}

pub(super) fn compute_justified_layout(
    aspect_ratios: &[f64],
    available_width: f64,
    target_row_height: f64,
) -> JustifiedLayout {
    let available_width = available_width.max(CARD_MIN_WIDTH);
    let target_row_height = target_row_height.max(1.0);

    let mut rows = Vec::new();
    let mut row_indices = Vec::<usize>::new();
    let mut row_aspects = Vec::<f64>::new();
    let mut row_sum = 0.0_f64;
    let mut y = 0.0_f64;

    let push_row = |rows: &mut Vec<JustifiedRow>,
                    indices: &[usize],
                    aspects: &[f64],
                    row_sum: f64,
                    y: f64,
                    justify: bool| {
        if indices.is_empty() {
            return 0.0;
        }

        let gaps = CARD_ROW_GAP * indices.len().saturating_sub(1) as f64;
        let natural_width = row_sum * target_row_height + gaps;
        let row_height = if justify {
            ((available_width - gaps).max(1.0) / row_sum.max(f64::EPSILON)).max(1.0)
        } else {
            target_row_height
        };
        let row_width = if justify {
            available_width
        } else {
            natural_width
        };

        let mut x = 0.0_f64;
        let items = indices
            .iter()
            .zip(aspects.iter())
            .map(|(&index, &aspect_ratio)| {
                let width = row_height * aspect_ratio;
                let item = JustifiedRowItem {
                    index,
                    x,
                    width,
                    aspect_ratio,
                };
                x += width + CARD_ROW_GAP;
                item
            })
            .collect::<Vec<_>>();

        rows.push(JustifiedRow {
            y,
            height: row_height,
            width: row_width,
            justified: justify,
            items,
        });

        row_height
    };

    for (index, aspect_ratio) in aspect_ratios.iter().copied().enumerate() {
        let aspect_ratio = normalize_aspect_ratio(aspect_ratio);
        row_indices.push(index);
        row_aspects.push(aspect_ratio);
        row_sum += aspect_ratio;

        let gaps = CARD_ROW_GAP * row_indices.len().saturating_sub(1) as f64;
        let natural_width = row_sum * target_row_height + gaps;
        if row_indices.len() > 1 && natural_width >= available_width {
            let row_height = push_row(&mut rows, &row_indices, &row_aspects, row_sum, y, true);
            y += row_height + CARD_ROW_GAP;
            row_indices.clear();
            row_aspects.clear();
            row_sum = 0.0;
        }
    }

    if !row_indices.is_empty() {
        let gaps = CARD_ROW_GAP * row_indices.len().saturating_sub(1) as f64;
        let natural_width = row_sum * target_row_height + gaps;
        let fill_ratio = natural_width / available_width;
        let justify_last = row_indices.len() > 1 && fill_ratio >= FEED_ORPHAN_ROW_WIDTH_THRESHOLD;
        let row_height = push_row(
            &mut rows,
            &row_indices,
            &row_aspects,
            row_sum,
            y,
            justify_last,
        );
        y += row_height;
    } else if y > 0.0 {
        y -= CARD_ROW_GAP;
    }

    JustifiedLayout {
        rows,
        content_width: available_width,
        content_height: y.max(1.0),
    }
}

fn feed_available_width(viewport_width: f64, sidebar_collapsed: bool) -> f64 {
    let sidebar_width = if sidebar_collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        SIDEBAR_EXPANDED_WIDTH
    };

    (viewport_width - sidebar_width - 64.0).max(CARD_MIN_WIDTH)
}

pub(super) fn compute_feed_scroll_viewport_size(
    viewport_width: f64,
    viewport_height: f64,
    sidebar_collapsed: bool,
    search_visible: bool,
    response_visible: bool,
) -> (f64, f64) {
    let feed_width = feed_available_width(viewport_width, sidebar_collapsed);

    let mut feed_height = viewport_height - FEED_BASE_CHROME_HEIGHT;
    if search_visible {
        feed_height -= FEED_SEARCH_PANEL_HEIGHT;
    }
    if response_visible {
        feed_height -= RESPONSE_PANEL_HEIGHT + FEED_RESPONSE_PANEL_SPACING;
    }

    (feed_width, feed_height.max(240.0))
}

fn feed_ancestor_scroll_view(world: &World, mut entity: Entity) -> Option<UiScrollView> {
    loop {
        let parent = world.get::<ChildOf>(entity)?.parent();
        if let Some(scroll_view) = world.get::<UiScrollView>(parent) {
            return Some(*scroll_view);
        }
        entity = parent;
    }
}

#[cfg(test)]
pub(super) fn compute_feed_layout_for_width(available_width: f64) -> (usize, f64) {
    let available_width = available_width.max(CARD_MIN_WIDTH);
    // Compute columns accounting for gaps: n <= (W + G) / (C + G)
    let columns = ((available_width + CARD_ROW_GAP) / (CARD_MIN_WIDTH + CARD_ROW_GAP))
        .floor()
        .clamp(1.0, MAX_CARD_COLUMNS as f64) as usize;
    let spacing = CARD_ROW_GAP * columns.saturating_sub(1) as f64;
    let card_width = ((available_width - spacing) / columns as f64).max(CARD_MIN_WIDTH);

    (columns, card_width)
}

#[cfg(test)]
pub(super) fn compute_feed_layout(viewport_width: f64, sidebar_collapsed: bool) -> (usize, f64) {
    compute_feed_layout_for_width(feed_available_width(viewport_width, sidebar_collapsed))
}

pub(super) fn feed_layout_width(world: &World, entity: Entity) -> f64 {
    feed_ancestor_scroll_view(world, entity)
        .map(|scroll_view| (scroll_view.viewport_size.x as f64).max(CARD_MIN_WIDTH))
        .unwrap_or_else(|| {
            let viewport_width = world
                .get_resource::<ViewportMetrics>()
                .map(|viewport| viewport.width as f64)
                .unwrap_or(1360.0);
            let sidebar_collapsed = world
                .get_resource::<UiState>()
                .map(|ui| ui.sidebar_collapsed)
                .unwrap_or(false);
            feed_available_width(viewport_width, sidebar_collapsed)
        })
}

#[cfg(test)]
pub(super) fn estimate_illust_card_height(
    world: &World,
    card_entity: Entity,
    card_width: f64,
) -> f64 {
    let fallback_ratio = 0.62;

    let image_ratio = world
        .get::<IllustVisual>(card_entity)
        .and_then(|visual| visual.thumb_ui.as_ref())
        .map(|thumb| {
            if thumb.width == 0 {
                fallback_ratio
            } else {
                (thumb.height as f64 / thumb.width as f64).clamp(0.45, 1.45)
            }
        })
        .unwrap_or(fallback_ratio);

    let image_height = (card_width * image_ratio).max(120.0);
    let title_chars = world
        .get::<Illust>(card_entity)
        .map(|illust| illust.title.chars().count())
        .unwrap_or(24);
    let title_lines = (title_chars as f64 / 18.0).ceil().max(1.0);

    image_height + 64.0 + title_lines * 18.0
}

fn button_from_style(
    entity: Entity,
    action: AppAction,
    label_text: impl Into<String>,
    style: &ResolvedStyle,
) -> UiView {
    let label_text = label_text.into();
    Arc::new(apply_direct_widget_style(
        button(entity, action, label_text),
        style,
    ))
}

fn lucide_icon(icon: LucideIcon, size_px: f64, color: Color) -> UiView {
    let mut icon_style = ResolvedStyle::default();
    icon_style.colors.text = Some(color);
    icon_style.text.size = (size_px * 0.90) as f32;
    icon_style.font_family = Some(vec![LUCIDE_FONT_FAMILY.to_string()]);

    Arc::new(
        sized_box(apply_label_style(
            label(char::from(icon).to_string()),
            &icon_style,
        ))
        .width(Dim::Fixed(Length::px(size_px)))
        .height(Dim::Fixed(Length::px(size_px))),
    )
}

fn action_button(
    world: &World,
    entity: Entity,
    action: AppAction,
    label_text: impl Into<String>,
) -> UiView {
    let style = resolve_style(world, entity);
    button_from_style(entity, action, label_text, &style)
}

fn sidebar_button_view(
    world: &World,
    entity: Entity,
    action: AppAction,
    label_text: impl Into<String>,
    active: bool,
) -> UiView {
    let style = if active {
        resolve_style_for_classes(
            world,
            ["pixiv.sidebar.button", "pixiv.sidebar.button.active"],
        )
    } else {
        resolve_style_for_classes(world, ["pixiv.sidebar.button"])
    };
    button_from_style(entity, action, label_text, &style)
}

fn sidebar_toggle_button_view(world: &World, entity: Entity, sidebar_collapsed: bool) -> UiView {
    let style = resolve_style_for_classes(world, ["pixiv.sidebar.button"]);
    let text_color = style.colors.text.unwrap_or(Color::WHITE);

    let (toggle_text, toggle_icon, icon_first) = if sidebar_collapsed {
        (
            tr(world, "pixiv.sidebar.expand", "Expand"),
            LucideIcon::ChevronRight,
            false,
        )
    } else {
        (
            tr(world, "pixiv.sidebar.collapse", "Collapse"),
            LucideIcon::ChevronLeft,
            true,
        )
    };

    let content = if icon_first {
        flex_row((
            lucide_icon(toggle_icon, 14.0, text_color).into_any_flex(),
            apply_label_style(label(toggle_text), &style).into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0))
    } else {
        flex_row((
            apply_label_style(label(toggle_text), &style).into_any_flex(),
            lucide_icon(toggle_icon, 14.0, text_color).into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0))
    };

    Arc::new(apply_direct_widget_style(
        button_with_child(entity, AppAction::ToggleSidebar, content),
        &style,
    ))
}

pub(super) fn project_root(_: &PixivRoot, ctx: ProjectionCtx<'_>) -> UiView {
    let root_style = resolve_style(ctx.world, ctx.entity);
    let ui = ctx.world.resource::<UiState>();
    let sidebar_width = if ui.sidebar_collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        SIDEBAR_EXPANDED_WIDTH
    };

    let mut children = ctx.children.into_iter();
    let theme_picker = children.next().unwrap_or_else(empty_ui);
    let sidebar = children.next().unwrap_or_else(empty_ui);
    let main_content = children.next().unwrap_or_else(empty_ui);

    Arc::new(apply_widget_style(
        flex_col(vec![
            theme_picker.into_any_flex(),
            flex_row((
                sized_box(sidebar)
                    .dims((Length::px(sidebar_width), Dim::Stretch))
                    .into_any_flex(),
                main_content.flex(1.0).into_any_flex(),
            ))
            .main_axis_alignment(MainAxisAlignment::Start)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .flex(1.0)
            .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .dims(Dim::Stretch),
        &root_style,
    ))
}

pub(super) fn project_sidebar(_: &PixivSidebar, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let ui = ctx.world.resource::<UiState>();
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let section_style = resolve_style_for_classes(ctx.world, ["pixiv.sidebar.section"]);
    let title_style = resolve_style_for_classes(ctx.world, ["pixiv.sidebar.title"]);
    let mut sidebar_children = ctx.children.into_iter();
    let locale_combo_view = sidebar_children.next().unwrap_or_else(empty_ui);
    let auth_panel_view = sidebar_children.next().unwrap_or_else(empty_ui);

    let mut items = Vec::new();
    items.push(
        apply_widget_style(
            apply_label_style(label("Navigation"), &title_style),
            &section_style,
        )
        .into_any_flex(),
    );

    items.push(
        sidebar_toggle_button_view(
            ctx.world,
            ui_components.toggle_sidebar,
            ui.sidebar_collapsed,
        )
        .into_any_flex(),
    );

    if !ui.sidebar_collapsed {
        items.push(
            sidebar_button_view(
                ctx.world,
                ui_components.home_tab,
                AppAction::SetTab(NavTab::Home),
                tr(ctx.world, "pixiv.sidebar.home", "Home"),
                ui.active_tab == NavTab::Home,
            )
            .into_any_flex(),
        );
        items.push(
            sidebar_button_view(
                ctx.world,
                ui_components.rankings_tab,
                AppAction::SetTab(NavTab::Rankings),
                tr(ctx.world, "pixiv.sidebar.rankings", "Rankings"),
                ui.active_tab == NavTab::Rankings,
            )
            .into_any_flex(),
        );
        items.push(
            sidebar_button_view(
                ctx.world,
                ui_components.manga_tab,
                AppAction::SetTab(NavTab::Manga),
                tr(ctx.world, "pixiv.sidebar.manga", "Manga"),
                ui.active_tab == NavTab::Manga,
            )
            .into_any_flex(),
        );
        items.push(
            sidebar_button_view(
                ctx.world,
                ui_components.novels_tab,
                AppAction::SetTab(NavTab::Novels),
                tr(ctx.world, "pixiv.sidebar.novels", "Novels"),
                ui.active_tab == NavTab::Novels,
            )
            .into_any_flex(),
        );
        items.push(
            sidebar_button_view(
                ctx.world,
                ui_components.search_tab,
                AppAction::SetTab(NavTab::Search),
                tr(ctx.world, "pixiv.sidebar.search", "Search"),
                ui.active_tab == NavTab::Search,
            )
            .into_any_flex(),
        );

        items.push(
            apply_widget_style(
                apply_label_style(
                    label(tr(ctx.world, "pixiv.sidebar.language", "Language")),
                    &title_style,
                ),
                &section_style,
            )
            .into_any_flex(),
        );
        items.push(locale_combo_view.into_any_flex());
    }

    Arc::new(apply_widget_style(
        flex_col(vec![
            flex_col(items)
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .width(Dim::Stretch)
                .into_any_flex(),
            empty_ui().flex(1.0).into_any_flex(),
            auth_panel_view.into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch)
        .height(Dim::Stretch),
        &style,
    ))
}

pub(super) fn project_main_column(_: &PixivMainColumn, ctx: ProjectionCtx<'_>) -> UiView {
    let root_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);

    let mut children = Vec::new();

    let mut projected_children = ctx.children.into_iter().collect::<Vec<_>>();
    let feed_scroll = projected_children.pop();

    children.extend(
        projected_children
            .into_iter()
            .map(|child| child.into_any_flex()),
    );

    if let Some(feed_scroll) = feed_scroll {
        children.push(feed_scroll.flex(1.0).into_any_flex());
    }

    Arc::new(apply_widget_style(
        flex_col(children)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch)
            .height(Dim::Stretch),
        &root_style,
    ))
}

pub(super) fn project_auth_panel(_: &PixivAuthPanel, ctx: ProjectionCtx<'_>) -> UiView {
    let auth = ctx.world.resource::<AuthState>();
    let ui = ctx.world.resource::<UiState>();
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let panel_style = resolve_style_for_classes(ctx.world, ["pixiv.sidebar.section"]);
    let compact = ui.sidebar_collapsed;

    let trigger: UiView = if auth.session.is_some() {
        let button_style = resolve_style(ctx.world, ui_components.account_menu_toggle);
        let label_text = auth
            .user_summary
            .as_ref()
            .map(|summary| summary.name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| tr(ctx.world, "pixiv.auth.account", "Account"));
        let content: UiView = if compact {
            auth_avatar_view(ctx.world, 24.0, &button_style)
        } else {
            Arc::new(
                flex_row((
                    auth_avatar_view(ctx.world, 24.0, &button_style).into_any_flex(),
                    apply_label_style(label(label_text), &button_style).into_any_flex(),
                    lucide_icon(
                        LucideIcon::ChevronDown,
                        14.0,
                        button_style.colors.text.unwrap_or(Color::WHITE),
                    )
                    .into_any_flex(),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .gap(Length::px(8.0)),
            )
        };

        Arc::new(apply_direct_widget_style(
            button_with_child(
                ui_components.account_menu_toggle,
                AppAction::ToggleAccountMenu,
                content,
            ),
            &button_style,
        ))
    } else {
        action_button(
            ctx.world,
            ui_components.auth_dialog_toggle,
            AppAction::OpenLoginDialog,
            tr(ctx.world, "pixiv.auth.show_login", "Login"),
        )
    };

    Arc::new(apply_widget_style(
        flex_col(vec![trigger.into_any_flex()])
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch),
        &panel_style,
    ))
}

pub(super) fn project_auth_dialog_form(_: &PixivAuthDialogForm, ctx: ProjectionCtx<'_>) -> UiView {
    let auth = ctx.world.resource::<AuthState>();
    let text_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);
    let mut children = ctx.children.into_iter();
    let code_verifier_input = children.next().unwrap_or_else(empty_ui);
    let auth_code_input = children.next().unwrap_or_else(empty_ui);
    let refresh_token_input = children.next().unwrap_or_else(empty_ui);
    let auth_endpoint = auth
        .idp_urls
        .as_ref()
        .map(|i| i.auth_token_url.as_str())
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| tr(ctx.world, "pixiv.auth.loading", "loading…"));
    let ui_components = *ctx.world.resource::<PixivUiComponents>();

    Arc::new(
        sized_box(
            flex_col(vec![
                apply_label_style(
                    label(format!(
                        "{} {}",
                        tr(ctx.world, "pixiv.auth.endpoint", "Auth endpoint:"),
                        auth_endpoint
                    )),
                    &text_style,
                )
                .into_any_flex(),
                sized_box(code_verifier_input)
                    .width(Dim::Stretch)
                    .into_any_flex(),
                sized_box(auth_code_input)
                    .width(Dim::Stretch)
                    .into_any_flex(),
                flex_row((
                    action_button(
                        ctx.world,
                        ui_components.open_browser_login,
                        AppAction::OpenBrowserLogin,
                        tr(
                            ctx.world,
                            "pixiv.auth.open_browser_login",
                            "Open Browser Login",
                        ),
                    )
                    .flex(1.0),
                    action_button(
                        ctx.world,
                        ui_components.exchange_auth_code,
                        AppAction::ExchangeAuthCode,
                        tr(ctx.world, "pixiv.auth.login_auth_code", "Login (auth_code)"),
                    )
                    .flex(1.0),
                ))
                .gap(Length::px(10.0))
                .into_any_flex(),
                sized_box(refresh_token_input)
                    .width(Dim::Stretch)
                    .into_any_flex(),
                action_button(
                    ctx.world,
                    ui_components.refresh_token,
                    AppAction::RefreshToken,
                    tr(ctx.world, "pixiv.auth.refresh_token", "Refresh Token"),
                )
                .into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch)
            .gap(Length::px(10.0)),
        )
        .fixed_width(Length::px(520.0)),
    )
}

pub(super) fn project_account_menu(_: &PixivAccountMenu, ctx: ProjectionCtx<'_>) -> UiView {
    let auth = ctx.world.resource::<AuthState>();
    if auth.session.is_none() {
        return empty_ui();
    }

    let style = resolve_style(ctx.world, ctx.entity);
    let computed_position = ctx
        .world
        .get::<OverlayComputedPosition>(ctx.entity)
        .copied()
        .unwrap_or_default();

    if !computed_position.is_positioned {
        return empty_ui();
    }

    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let menu_width = if computed_position.width > 1.0 {
        computed_position.width
    } else {
        132.0
    };

    let menu_surface = sized_box(apply_widget_style(
        action_button(
            ctx.world,
            ui_components.logout,
            AppAction::Logout,
            tr(ctx.world, "pixiv.auth.logout", "Logout"),
        ),
        &style,
    ))
    .width(Dim::Stretch)
    .height(Dim::Stretch);

    let scrollable_menu = picus_core::xilem::view::portal(menu_surface).dims((
        Length::px(menu_width),
        Length::px(computed_position.height.max(56.0)),
    ));

    let dropdown_panel = transformed(opaque_hitbox_for_entity(ctx.entity, scrollable_menu))
        .translate((computed_position.x, computed_position.y));

    Arc::new(dropdown_panel)
}

pub(super) fn project_response_panel(_: &PixivResponsePanel, ctx: ProjectionCtx<'_>) -> UiView {
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let panel = ctx.world.resource::<ResponsePanelState>();
    let text_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);

    if panel.content.trim().is_empty() {
        return empty_ui();
    }

    let lines = panel
        .content
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let lines = Arc::new(lines);
    let line_style = text_style.clone();
    let line_count = i64::try_from(lines.len()).unwrap_or(i64::MAX);

    Arc::new(
        flex_col((
            apply_label_style(label(panel.title.clone()), &text_style).into_any_flex(),
            flex_row((
                action_button(
                    ctx.world,
                    ui_components.copy_response,
                    AppAction::CopyResponseBody,
                    tr(ctx.world, "pixiv.response.copy", "Copy Response Body"),
                )
                .into_any_flex(),
                action_button(
                    ctx.world,
                    ui_components.clear_response,
                    AppAction::ClearResponseBody,
                    tr(ctx.world, "pixiv.response.clear", "Clear"),
                )
                .into_any_flex(),
            ))
            .into_any_flex(),
            sized_box(virtual_scroll(0..line_count, {
                let lines = Arc::clone(&lines);
                let line_style = line_style.clone();
                move |_, idx| {
                    let row_idx = usize::try_from(idx).unwrap_or(0);
                    Arc::new(apply_label_style(
                        label(lines.get(row_idx).cloned().unwrap_or_default()),
                        &line_style,
                    )) as UiView
                }
            }))
            .dims((Dim::Stretch, Length::px(RESPONSE_PANEL_HEIGHT)))
            .into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch),
    )
}

pub(super) fn project_search_panel(_: &PixivSearchPanel, ctx: ProjectionCtx<'_>) -> UiView {
    let ui = ctx.world.resource::<UiState>();
    if ui.active_tab != NavTab::Search {
        return empty_ui();
    }

    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let search_input = ctx.children.into_iter().next().unwrap_or_else(empty_ui);

    Arc::new(
        flex_row((
            search_input.flex(1.0),
            action_button(
                ctx.world,
                ui_components.search_submit,
                AppAction::SubmitSearch,
                tr(ctx.world, "pixiv.search.submit", "Search"),
            )
            .into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch),
    )
}

pub(super) fn project_home_feed(_: &PixivHomeFeed, ctx: ProjectionCtx<'_>) -> UiView {
    if ctx.children.is_empty() {
        let style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);
        return Arc::new(apply_label_style(
            label(tr(
                ctx.world,
                "pixiv.feed.empty",
                "No data yet. Login first, then switch tabs.",
            )),
            &style,
        ));
    }

    let available_width = feed_layout_width(ctx.world, ctx.entity);
    let scroll_view = feed_ancestor_scroll_view(ctx.world, ctx.entity);
    let (visible_start, visible_end) = scroll_view
        .map(UiScrollView::visible_rect)
        .unwrap_or((Vec2::ZERO, Vec2::new(f32::MAX, f32::MAX)));
    let visible_min_y = visible_start.y as f64 - FEED_OVERSCAN_Y;
    let visible_max_y = visible_end.y as f64 + FEED_OVERSCAN_Y;

    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.to_vec())
        .unwrap_or_default();

    let child_views = child_entities
        .into_iter()
        .zip(ctx.children)
        .collect::<Vec<_>>();
    let aspect_ratios = child_views
        .iter()
        .map(|(entity, _)| illust_aspect_ratio(ctx.world, *entity))
        .collect::<Vec<_>>();
    let layout = compute_justified_layout(&aspect_ratios, available_width, FEED_TARGET_ROW_HEIGHT);

    let mut visible_cards = Vec::<UiView>::new();
    for row in &layout.rows {
        let row_bottom = row.y + row.height;
        if row_bottom < visible_min_y || row.y > visible_max_y {
            continue;
        }

        for item in &row.items {
            let (card_entity, child_view) = &child_views[item.index];
            let tile: UiView = Arc::new(
                sized_box(child_view.clone())
                    .width(Dim::Fixed(Length::px(item.width)))
                    .height(Dim::Fixed(Length::px(row.height))),
            );
            visible_cards.push(Arc::new(
                transformed(opaque_hitbox_for_entity(*card_entity, tile))
                    .translate((item.x, row.y)),
            ));
        }
    }

    Arc::new(
        sized_box(
            zstack(visible_cards)
                .alignment(UnitPoint::TOP_LEFT)
                .width(Dim::Fixed(Length::px(layout.content_width)))
                .height(Dim::Fixed(Length::px(layout.content_height))),
        )
        .width(Dim::Fixed(Length::px(layout.content_width)))
        .height(Dim::Fixed(Length::px(layout.content_height))),
    )
}

fn illust_thumbnail_view(world: &World, illust: &Illust, visual: &IllustVisual) -> UiView {
    if let Some(image_data) = visual.thumb_ui.clone() {
        Arc::new(image(image_data))
    } else {
        match illust.content_kind {
            PixivContentKind::Novel => Arc::new(
                flex_col((
                    lucide_icon(LucideIcon::BookOpen, 24.0, Color::WHITE).into_any_flex(),
                    label(tr(
                        world,
                        "pixiv.feed.novel_placeholder",
                        "Novel cover unavailable",
                    ))
                    .into_any_flex(),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .main_axis_alignment(MainAxisAlignment::Center)
                .width(Dim::Stretch)
                .height(Dim::Stretch),
            ),
            _ => Arc::new(label(tr(
                world,
                "pixiv.feed.thumbnail_loading",
                "thumbnail loading…",
            ))),
        }
    }
}

fn illust_avatar_view(visual: &IllustVisual, style: &ResolvedStyle) -> UiView {
    if let Some(image_data) = visual.avatar_ui.clone() {
        Arc::new(
            sized_box(image(image_data))
                .fixed_height(Length::px(28.0))
                .fixed_width(Length::px(28.0)),
        )
    } else {
        lucide_icon(
            LucideIcon::User,
            18.0,
            style.colors.text.unwrap_or(Color::WHITE),
        )
    }
}

fn auth_avatar_view(world: &World, size_px: f64, style: &ResolvedStyle) -> UiView {
    if let Some(image_data) = world
        .get_resource::<AuthAvatarVisual>()
        .and_then(|visual| visual.avatar_ui.clone())
    {
        Arc::new(
            sized_box(image(image_data))
                .fixed_height(Length::px(size_px))
                .fixed_width(Length::px(size_px)),
        )
    } else {
        lucide_icon(
            LucideIcon::User,
            size_px * 0.72,
            style.colors.text.unwrap_or(Color::WHITE),
        )
    }
}

fn detail_meta_value(world: &World, value: impl Into<String>) -> UiView {
    let style = resolve_style_for_classes(world, ["pixiv.detail.meta.value"]);
    Arc::new(apply_label_style(label(value.into()), &style))
}

fn detail_description_value(world: &World, value: impl Into<String>) -> UiView {
    let style = resolve_style_for_classes(world, ["pixiv.detail.description"]);
    Arc::new(apply_label_style(label(value.into()), &style))
}

fn detail_section(world: &World, title: String, body: UiView) -> UiView {
    let section_style = resolve_style_for_classes(world, ["pixiv.detail.section"]);
    let title_style = resolve_style_for_classes(world, ["pixiv.detail.section.title"]);

    Arc::new(apply_widget_style(
        flex_col((
            apply_label_style(label(title), &title_style).into_any_flex(),
            body.into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch),
        &section_style,
    ))
}

fn detail_description_text(illust: &Illust) -> Option<String> {
    illust
        .description
        .as_ref()
        .map(|text| text.replace("<br />", "\n").replace("<br/>", "\n"))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn detail_image_info_text(world: &World, illust: &Illust) -> String {
    let dimensions = if illust.width > 0 && illust.height > 0 {
        format!("{}×{} px", illust.width, illust.height,)
    } else {
        tr(world, "pixiv.overlay.image-info-unknown", "Unknown size")
    };

    format!(
        "{} · {} {}",
        dimensions,
        illust.page_count.max(1),
        tr(world, "pixiv.overlay.pages", "pages")
    )
}

pub(super) fn project_illust_card(_: &PixivIllustCard, ctx: ProjectionCtx<'_>) -> UiView {
    let Some(illust) = ctx.world.get::<Illust>(ctx.entity) else {
        return empty_ui();
    };

    let visual = ctx
        .world
        .get::<IllustVisual>(ctx.entity)
        .cloned()
        .unwrap_or_default();
    let anim = ctx
        .world
        .get::<CardAnimState>(ctx.entity)
        .copied()
        .unwrap_or_default();
    let action_entities = ctx
        .world
        .get::<IllustActionEntities>(ctx.entity)
        .copied()
        .unwrap_or(IllustActionEntities {
            open_thumbnail: ctx.entity,
            bookmark: ctx.entity,
        });
    let subtle_button_style = if illust.is_bookmarked {
        resolve_style_for_entity_classes(
            ctx.world,
            action_entities.bookmark,
            [
                "pixiv.button",
                "pixiv.button.subtle",
                "pixiv.button.subtle.active-bookmark",
            ],
        )
    } else {
        resolve_style_for_entity_classes(
            ctx.world,
            action_entities.bookmark,
            ["pixiv.button", "pixiv.button.subtle"],
        )
    };
    let heart_icon_color = subtle_button_style.colors.text.unwrap_or(Color::WHITE);
    let heart_icon = LucideIcon::Heart;

    let heart_button = sized_box(Arc::new(
        button_with_child(
            action_entities.bookmark,
            AppAction::Bookmark(ctx.entity),
            lucide_icon(heart_icon, 16.0 * anim.heart_scale as f64, heart_icon_color),
        )
        .padding(0.0)
        .border(Color::TRANSPARENT, 0.0)
        .background_color(Color::TRANSPARENT),
    ))
    .fixed_width(Length::px(40.0))
    .fixed_height(Length::px(32.0));

    let open_button_view: UiView = Arc::new(
        button_with_child(
            action_entities.open_thumbnail,
            AppAction::OpenIllust(ctx.entity),
            illust_thumbnail_view(ctx.world, illust, &visual),
        )
        .padding(0.0)
        .border(Color::TRANSPARENT, 0.0)
        .background_color(Color::TRANSPARENT),
    );

    Arc::new(
        sized_box(
            zstack(vec![open_button_view, Arc::new(heart_button)])
                .alignment(UnitPoint::TOP_RIGHT)
                .dims(Dim::Stretch),
        )
        .width(Dim::Stretch)
        .height(Dim::Stretch),
    )
}

pub(super) fn project_detail_overlay(_: &PixivDetailOverlay, ctx: ProjectionCtx<'_>) -> UiView {
    let ui = ctx.world.resource::<UiState>();
    let Some(entity) = ui.selected_illust else {
        return empty_ui();
    };

    let Some(_illust) = ctx.world.get::<Illust>(entity) else {
        return empty_ui();
    };
    let visual = ctx
        .world
        .get::<IllustVisual>(entity)
        .cloned()
        .unwrap_or_default();
    let text_style = resolve_style(ctx.world, ctx.entity);
    let hero_style = resolve_style_for_classes(ctx.world, ["pixiv.detail.hero"]);
    let meta_style = resolve_style_for_classes(ctx.world, ["pixiv.detail.meta"]);
    let (dialog_width, _dialog_height) = detail_dialog_size_for_overlay(ctx.world, ctx.entity);
    let rail_width = compute_detail_meta_rail_width(dialog_width);

    let hero: UiView = if let Some(high_res) = visual.high_res_ui.clone() {
        Arc::new(apply_widget_style(
            sized_box(image(high_res))
                .width(Dim::Stretch)
                .height(Dim::Stretch),
            &hero_style,
        ))
    } else {
        Arc::new(apply_widget_style(
            flex_col((
                lucide_icon(
                    LucideIcon::Image,
                    22.0,
                    hero_style.colors.text.unwrap_or(Color::WHITE),
                )
                .into_any_flex(),
                apply_label_style(
                    label(tr(
                        ctx.world,
                        "pixiv.feed.high_res_loading",
                        "high-res loading…",
                    )),
                    &text_style,
                )
                .into_any_flex(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .main_axis_alignment(MainAxisAlignment::Center)
            .width(Dim::Stretch)
            .height(Dim::Stretch),
            &hero_style,
        ))
    };

    let scrollable_info = Arc::new(apply_widget_style(
        sized_box(ctx.children.into_iter().next().unwrap_or_else(empty_ui))
            .width(Dim::Stretch)
            .height(Dim::Stretch),
        &meta_style,
    ));

    Arc::new(
        sized_box(
            flex_row((
                sized_box(hero)
                    .width(Dim::Stretch)
                    .height(Dim::Stretch)
                    .flex(1.65)
                    .into_any_flex(),
                sized_box(scrollable_info)
                    .width(Length::px(rail_width))
                    .height(Dim::Stretch)
                    .into_any_flex(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch)
            .height(Dim::Stretch)
            .gap(Length::px(18.0)),
        )
        .width(Dim::Stretch)
        .height(Dim::Stretch),
    )
}

pub(super) fn project_detail_meta_rail(_: &PixivDetailMetaRail, ctx: ProjectionCtx<'_>) -> UiView {
    let ui = ctx.world.resource::<UiState>();
    let Some(entity) = ui.selected_illust else {
        return empty_ui();
    };

    let Some(illust) = ctx.world.get::<Illust>(entity) else {
        return empty_ui();
    };

    let visual = ctx
        .world
        .get::<IllustVisual>(entity)
        .cloned()
        .unwrap_or_default();
    let text_style = resolve_style(ctx.world, ctx.entity);
    let meta_style = resolve_style_for_classes(ctx.world, ["pixiv.detail.meta"]);
    let tags = ctx.children.into_iter().next().unwrap_or_else(empty_ui);
    let author_account = illust.user.account.clone().unwrap_or_else(|| {
        tr(
            ctx.world,
            "pixiv.overlay.account-unknown",
            "No public account",
        )
    });

    let artwork_info = detail_section(
        ctx.world,
        tr(ctx.world, "pixiv.overlay.artwork-info", "Artwork info"),
        Arc::new(
            flex_col((
                detail_meta_value(ctx.world, illust.title.clone()).into_any_flex(),
                detail_meta_value(
                    ctx.world,
                    format!(
                        "{} {} · {} {} · {} {}",
                        tr(ctx.world, "pixiv.overlay.views", "Views"),
                        illust.total_view,
                        tr(ctx.world, "pixiv.overlay.bookmarks", "Bookmarks"),
                        illust.total_bookmarks,
                        tr(ctx.world, "pixiv.overlay.comments", "Comments"),
                        illust.total_comments
                    ),
                )
                .into_any_flex(),
                detail_meta_value(
                    ctx.world,
                    match illust.content_kind {
                        PixivContentKind::Illust => {
                            tr(ctx.world, "pixiv.overlay.type-illust", "Illustration")
                        }
                        PixivContentKind::Manga => {
                            tr(ctx.world, "pixiv.overlay.type-manga", "Manga")
                        }
                        PixivContentKind::Novel => {
                            tr(ctx.world, "pixiv.overlay.type-novel", "Novel")
                        }
                    },
                )
                .into_any_flex(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch),
        ),
    );

    let author_info = detail_section(
        ctx.world,
        tr(ctx.world, "pixiv.overlay.author-info", "Author info"),
        Arc::new(apply_widget_style(
            flex_row((
                sized_box(illust_avatar_view(&visual, &text_style))
                    .fixed_width(Length::px(40.0))
                    .fixed_height(Length::px(40.0))
                    .into_any_flex(),
                flex_col((
                    detail_meta_value(ctx.world, illust.user.name.clone()).into_any_flex(),
                    detail_meta_value(ctx.world, author_account).into_any_flex(),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .flex(1.0)
                .into_any_flex(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .width(Dim::Stretch),
            &meta_style,
        )),
    );

    let image_info = detail_section(
        ctx.world,
        tr(ctx.world, "pixiv.overlay.image-info", "Image info"),
        detail_meta_value(ctx.world, detail_image_info_text(ctx.world, illust)),
    );

    let description = detail_section(
        ctx.world,
        tr(ctx.world, "pixiv.overlay.caption", "Caption"),
        detail_description_value(
            ctx.world,
            detail_description_text(illust).unwrap_or_else(|| {
                tr(
                    ctx.world,
                    "pixiv.overlay.description-empty",
                    "No caption was provided for this artwork.",
                )
            }),
        ),
    );

    let tags_section = detail_section(ctx.world, tr(ctx.world, "pixiv.overlay.tags", "Tags"), tags);

    Arc::new(
        flex_col((
            artwork_info.into_any_flex(),
            author_info.into_any_flex(),
            image_info.into_any_flex(),
            description.into_any_flex(),
            tags_section.into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .width(Dim::Stretch)
        .gap(Length::px(meta_style.layout.gap.max(12.0))),
    )
}

pub(super) fn project_overlay_tags(_: &PixivOverlayTags, ctx: ProjectionCtx<'_>) -> UiView {
    if ctx.children.is_empty() {
        return empty_ui();
    }

    let rows = ctx
        .children
        .chunks(4)
        .map(|chunk| {
            flex_row(
                chunk
                    .iter()
                    .cloned()
                    .map(|child| child.into_any_flex())
                    .collect::<Vec<_>>(),
            )
            .into_any_flex()
        })
        .collect::<Vec<_>>();

    Arc::new(flex_col(rows).cross_axis_alignment(CrossAxisAlignment::Stretch))
}

pub(super) fn project_overlay_tag(tag: &OverlayTag, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    button_from_style(
        ctx.entity,
        AppAction::SearchByTag(tag.text.clone()),
        tag.text.clone(),
        &style,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_feed_layout_for_width_thresholds() {
        // Test with width exactly CARD_MIN_WIDTH
        let (cols, card_w) = compute_feed_layout_for_width(CARD_MIN_WIDTH);
        assert_eq!(cols, 1);
        assert!(
            (card_w - CARD_MIN_WIDTH).abs() < 1e-6,
            "card_width should be CARD_MIN_WIDTH, got {}",
            card_w
        );

        // Compute threshold for 2 columns: 2*CARD_MIN_WIDTH + CARD_ROW_GAP
        let threshold_2 = 2.0 * CARD_MIN_WIDTH + CARD_ROW_GAP;

        // Just below threshold should give 1 column
        let (cols, card_w) = compute_feed_layout_for_width(threshold_2 - 0.1);
        assert_eq!(cols, 1);
        assert!(card_w >= CARD_MIN_WIDTH);

        // Exactly at threshold should give 2 columns
        let (cols, card_w) = compute_feed_layout_for_width(threshold_2);
        assert_eq!(cols, 2);
        assert!(card_w >= CARD_MIN_WIDTH);

        // Just above threshold
        let (cols, card_w) = compute_feed_layout_for_width(threshold_2 + 50.0);
        assert_eq!(cols, 2);
        assert!(card_w >= CARD_MIN_WIDTH);

        // Threshold for 3 columns: 3*CARD_MIN_WIDTH + 2*CARD_ROW_GAP
        let threshold_3 = 3.0 * CARD_MIN_WIDTH + 2.0 * CARD_ROW_GAP;
        let (cols, card_w) = compute_feed_layout_for_width(threshold_3);
        assert_eq!(cols, 3);
        assert!(card_w >= CARD_MIN_WIDTH);

        // Test max columns clamp
        let huge_width = 10000.0;
        let (cols, card_w) = compute_feed_layout_for_width(huge_width);
        assert_eq!(cols, MAX_CARD_COLUMNS);
        assert!(card_w >= CARD_MIN_WIDTH);
    }

    #[test]
    fn test_compute_feed_layout_integration() {
        // Test that feed_available_width and compute_feed_layout work together
        let viewport_width = 1360.0;
        let sidebar_collapsed = false;
        let (cols, card_w) = compute_feed_layout(viewport_width, sidebar_collapsed);
        // Expected: feed_available_width = viewport_width - SIDEBAR_EXPANDED_WIDTH - 64.0
        // = 1360 - 208 - 64 = 1088
        // Then columns = floor((1088 + 6) / (260 + 6)) = floor(1094/266) = floor(4.11) = 4
        // So expect 4 columns
        assert_eq!(cols, 4);
        assert!(card_w >= CARD_MIN_WIDTH);
    }

    #[test]
    fn justified_layout_fills_non_orphan_rows() {
        let layout = compute_justified_layout(&[1.4, 0.9, 1.2], 960.0, FEED_TARGET_ROW_HEIGHT);

        assert_eq!(layout.rows.len(), 1);
        assert!(layout.rows[0].justified);
        assert!((layout.rows[0].width - 960.0).abs() < 1e-6);
        assert!(layout.rows[0].height < FEED_TARGET_ROW_HEIGHT);
    }

    #[test]
    fn justified_layout_keeps_sparse_last_row_ragged() {
        let layout = compute_justified_layout(&[1.4, 1.1, 0.55], 700.0, FEED_TARGET_ROW_HEIGHT);

        assert_eq!(layout.rows.len(), 2);
        assert!(layout.rows[0].justified);
        assert!(!layout.rows[1].justified);
        assert!(layout.rows[1].width < 700.0 * FEED_ORPHAN_ROW_WIDTH_THRESHOLD);
        assert!((layout.rows[1].height - FEED_TARGET_ROW_HEIGHT).abs() < 1e-6);
    }

    #[test]
    fn justified_layout_uses_target_height_for_non_justified_tail() {
        let layout = compute_justified_layout(&[0.8, 0.7], 1200.0, FEED_TARGET_ROW_HEIGHT);

        assert_eq!(layout.rows.len(), 1);
        assert!(!layout.rows[0].justified);
        assert!((layout.rows[0].height - FEED_TARGET_ROW_HEIGHT).abs() < 1e-6);
    }

    #[test]
    fn detail_dialog_size_scales_with_viewport_and_stays_clamped() {
        let small = compute_detail_dialog_size(1024.0, 720.0);
        let large = compute_detail_dialog_size(2200.0, 1400.0);
        let compact = compute_detail_dialog_size(640.0, 480.0);

        assert!(small.0 >= DETAIL_DIALOG_MIN_WIDTH);
        assert!(small.1 >= DETAIL_DIALOG_MIN_HEIGHT);
        assert!(large.0 > small.0);
        assert!(large.1 > small.1);
        assert!(large.0 <= DETAIL_DIALOG_MAX_WIDTH);
        assert!(large.1 <= DETAIL_DIALOG_MAX_HEIGHT);
        assert!(compact.0 < DETAIL_DIALOG_MIN_WIDTH);
        assert!(compact.1 < DETAIL_DIALOG_MIN_HEIGHT);
        assert!(compact.0 <= 640.0 - DETAIL_DIALOG_VIEWPORT_MARGIN_X * 2.0);
        assert!(compact.1 <= 480.0 - DETAIL_DIALOG_VIEWPORT_MARGIN_Y * 2.0);
    }

    #[test]
    fn detail_meta_rail_width_tracks_dialog_width_with_bounds() {
        assert_eq!(compute_detail_meta_rail_width(560.0), 224.0);
        assert_eq!(compute_detail_meta_rail_width(900.0), DETAIL_RAIL_MIN_WIDTH);
        assert!(compute_detail_meta_rail_width(1320.0) > DETAIL_RAIL_MIN_WIDTH);
        assert_eq!(
            compute_detail_meta_rail_width(1800.0),
            DETAIL_RAIL_MAX_WIDTH
        );
    }
}

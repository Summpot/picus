use super::*;

use picus_core::{
    InteractionState, UiScrollView,
    bevy_math::Vec2,
    xilem::{
        masonry::layout::UnitPoint,
        view::{transformed, zstack},
    },
};

const FEED_OVERSCAN_Y: f64 = 240.0;
const FEED_BASE_CHROME_HEIGHT: f64 = 332.0;
const FEED_SEARCH_PANEL_HEIGHT: f64 = 56.0;
const FEED_RESPONSE_PANEL_SPACING: f64 = 18.0;

fn empty_ui() -> UiView {
    Arc::new(label(""))
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
        button_with_child(entity, action, apply_label_style(label(label_text), style)),
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
            .dims(Dim::Stretch)
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
        flex_col(items)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .width(Dim::Stretch),
        &style,
    ))
}

pub(super) fn project_main_column(_: &PixivMainColumn, ctx: ProjectionCtx<'_>) -> UiView {
    let ui = ctx.world.resource::<UiState>();
    let root_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);

    let mut children = Vec::new();
    children.push(apply_label_style(label(ui.status_line.clone()), &root_style).into_any_flex());

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
    let style = resolve_style(ctx.world, ctx.entity);
    let text_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);
    let auth = ctx.world.resource::<AuthState>();
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
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

    let rows = vec![
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
        .into_any_flex(),
        action_button(
            ctx.world,
            ui_components.exchange_auth_code,
            AppAction::ExchangeAuthCode,
            tr(ctx.world, "pixiv.auth.login_auth_code", "Login (auth_code)"),
        )
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
    ];

    Arc::new(apply_widget_style(
        flex_col(rows)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch),
        &style,
    ))
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
    let (columns, card_width) = compute_feed_layout_for_width(available_width);
    let columns = columns.max(1);
    let content_width =
        columns as f64 * card_width + CARD_ROW_GAP * columns.saturating_sub(1) as f64;
    let scroll_view = feed_ancestor_scroll_view(ctx.world, ctx.entity);
    let (visible_start, visible_end) = scroll_view
        .map(UiScrollView::visible_rect)
        .unwrap_or((Vec2::ZERO, Vec2::new(f32::MAX, f32::MAX)));
    let visible_min_y = visible_start.y as f64 - FEED_OVERSCAN_Y;
    let visible_max_y = visible_end.y as f64 + FEED_OVERSCAN_Y;

    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut column_heights = vec![0.0_f64; columns];
    let mut visible_cards = Vec::<UiView>::new();

    for (entity, child_view) in child_entities.into_iter().zip(ctx.children.into_iter()) {
        let estimated_height = estimate_illust_card_height(ctx.world, entity, card_width);
        let target_column = column_heights
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap_or(0);

        let x = target_column as f64 * (card_width + CARD_ROW_GAP);
        let y = column_heights[target_column];

        column_heights[target_column] += estimated_height + CARD_ROW_GAP;

        if y + estimated_height >= visible_min_y && y <= visible_max_y {
            visible_cards.push(Arc::new(transformed(child_view).translate((x, y))));
        }
    }

    let content_height = column_heights.into_iter().fold(0.0_f64, f64::max);
    let content_height = (content_height - CARD_ROW_GAP).max(1.0);

    Arc::new(
        sized_box(
            zstack(visible_cards)
                .alignment(UnitPoint::TOP_LEFT)
                .width(Dim::Fixed(Length::px(content_width.max(card_width))))
                .height(Dim::Fixed(Length::px(content_height))),
        )
        .width(Dim::Fixed(Length::px(content_width.max(card_width))))
        .height(Dim::Fixed(Length::px(content_height))),
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

fn illust_author_overlay(
    author: &str,
    avatar: UiView,
    style: &ResolvedStyle,
    hovered: bool,
) -> UiView {
    if !hovered {
        return empty_ui();
    }

    let overlay_colors = picus_core::ResolvedColorStyle {
        bg: Some(Color::from_rgba8(0, 0, 0, 180)),
        text: style.colors.text,
        border: None,
    };

    let overlay_style = picus_core::ResolvedStyle {
        colors: overlay_colors,
        layout: style.layout,
        text: style.text,
        font_family: style.font_family.clone(),
        box_shadow: None,
        transition: None,
    };

    Arc::new(apply_widget_style(
        flex_row((
            sized_box(avatar)
                .fixed_width(Length::px(20.0))
                .fixed_height(Length::px(20.0))
                .into_any_flex(),
            apply_label_style(label(author.to_string()), &overlay_style).into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .padding(4.0)
        .dims((Dim::Stretch, Dim::Fixed(Length::px(32.0)))),
        &overlay_style,
    ))
}

fn illust_stats_view(illust: &Illust, style: &ResolvedStyle) -> UiView {
    let icon_color = style.colors.text.unwrap_or(Color::WHITE);
    Arc::new(
        flex_row((
            lucide_icon(LucideIcon::Eye, 14.0, icon_color).into_any_flex(),
            apply_label_style(label(illust.total_view.to_string()), style).into_any_flex(),
            lucide_icon(LucideIcon::Heart, 14.0, icon_color).into_any_flex(),
            apply_label_style(label(illust.total_bookmarks.to_string()), style).into_any_flex(),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0)),
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
    let style = resolve_style(ctx.world, ctx.entity);
    let action_entities = ctx
        .world
        .get::<IllustActionEntities>(ctx.entity)
        .copied()
        .unwrap_or(IllustActionEntities {
            open_thumbnail: ctx.entity,
            bookmark: ctx.entity,
        });
    let subtle_button_style = resolve_style_for_entity_classes(
        ctx.world,
        action_entities.bookmark,
        ["pixiv.button", "pixiv.button.subtle"],
    );

    let (_, card_width) = compute_feed_layout_for_width(feed_layout_width(ctx.world, ctx.entity));

    let image_ratio = visual
        .thumb_ui
        .as_ref()
        .map(|thumb| {
            if thumb.width == 0 {
                0.58
            } else {
                (thumb.height as f64 / thumb.width as f64).clamp(0.45, 1.45)
            }
        })
        .unwrap_or(0.58);
    let image_height = (card_width * image_ratio * anim.card_scale as f64).max(120.0);
    let heart_icon_color = subtle_button_style.colors.text.unwrap_or(Color::WHITE);
    let heart_icon = if illust.is_bookmarked {
        LucideIcon::Heart
    } else {
        LucideIcon::HeartOff
    };

    let heart_button = sized_box(Arc::new(apply_direct_widget_style(
        button_with_child(
            action_entities.bookmark,
            AppAction::Bookmark(ctx.entity),
            lucide_icon(heart_icon, 16.0 * anim.heart_scale as f64, heart_icon_color),
        ),
        &subtle_button_style,
    )))
    .fixed_width(Length::px(40.0));

    let author_avatar = illust_avatar_view(&visual, &style);
    let hovered = ctx
        .world
        .get::<InteractionState>(action_entities.open_thumbnail)
        .map(|state| state.hovered)
        .unwrap_or(false);
    let author_overlay = illust_author_overlay(&illust.user.name, author_avatar, &style, hovered);

    let open_button_view = button_with_child(
        action_entities.open_thumbnail,
        AppAction::OpenIllust(ctx.entity),
        zstack(vec![
            illust_thumbnail_view(ctx.world, illust, &visual),
            author_overlay,
        ])
        .alignment(UnitPoint::BOTTOM_LEFT)
        .dims((Dim::Stretch, Length::px(image_height))),
    )
    .padding(0.0)
    .border(Color::TRANSPARENT, 0.0)
    .background_color(Color::TRANSPARENT);

    Arc::new(
        sized_box(apply_widget_style(
            flex_col(vec![
                open_button_view.into_any_flex(),
                apply_label_style(label(illust.title.clone()), &style).into_any_flex(),
                flex_row((
                    illust_stats_view(illust, &style).flex(1.0),
                    heart_button.into_any_flex(),
                ))
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .into_any_flex(),
            ]),
            &style,
        ))
        .fixed_width(Length::px(
            (card_width * anim.card_scale as f64).max(CARD_MIN_WIDTH),
        )),
    )
}

pub(super) fn project_detail_overlay(_: &PixivDetailOverlay, ctx: ProjectionCtx<'_>) -> UiView {
    let ui = ctx.world.resource::<UiState>();
    let Some(entity) = ui.selected_illust else {
        return empty_ui();
    };

    let Some(illust) = ctx.world.get::<Illust>(entity) else {
        return empty_ui();
    };
    let style = resolve_style(ctx.world, ctx.entity);
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
    let visual = ctx
        .world
        .get::<IllustVisual>(entity)
        .cloned()
        .unwrap_or_default();

    let hero: UiView = if let Some(high_res) = visual.high_res_ui {
        Arc::new(sized_box(image(high_res)).fixed_height(Length::px(280.0)))
    } else {
        Arc::new(label(tr(
            ctx.world,
            "pixiv.feed.high_res_loading",
            "high-res loading…",
        )))
    };

    let tags = ctx.children.into_iter().next().unwrap_or_else(empty_ui);

    Arc::new(
        sized_box(apply_widget_style(
            flex_col((
                flex_row((
                    apply_label_style(
                        label(tr(ctx.world, "pixiv.overlay.title", "Illustration Details")),
                        &style,
                    )
                    .flex(1.0),
                    action_button(
                        ctx.world,
                        ui_components.close_overlay,
                        AppAction::CloseIllust,
                        tr(ctx.world, "pixiv.overlay.close", "Close"),
                    )
                    .into_any_flex(),
                ))
                .into_any_flex(),
                hero.into_any_flex(),
                apply_label_style(label(illust.title.clone()), &style).into_any_flex(),
                apply_label_style(
                    label(format!(
                        "{} {}",
                        tr(ctx.world, "pixiv.overlay.author", "Author:"),
                        illust.user.name
                    )),
                    &style,
                )
                .into_any_flex(),
                apply_label_style(
                    label(format!(
                        "{} {}  {} {}  {} {}",
                        tr(ctx.world, "pixiv.overlay.views", "Views"),
                        illust.total_view,
                        tr(ctx.world, "pixiv.overlay.bookmarks", "Bookmarks"),
                        illust.total_bookmarks,
                        tr(ctx.world, "pixiv.overlay.comments", "Comments"),
                        illust.total_comments
                    )),
                    &style,
                )
                .into_any_flex(),
                apply_label_style(label(tr(ctx.world, "pixiv.overlay.tags", "Tags")), &style)
                    .into_any_flex(),
                tags.into_any_flex(),
            ))
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .width(Dim::Stretch),
            &style,
        ))
        .fixed_width(Length::px(760.0)),
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
}

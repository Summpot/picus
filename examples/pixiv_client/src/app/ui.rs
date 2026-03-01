use super::*;

fn empty_ui() -> UiView {
    Arc::new(label(""))
}

pub(super) fn compute_feed_layout(viewport_width: f64, sidebar_collapsed: bool) -> (usize, f64) {
    let sidebar_width = if sidebar_collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        SIDEBAR_EXPANDED_WIDTH
    };

    let available_width = (viewport_width - sidebar_width - 64.0).max(CARD_MIN_WIDTH);
    let columns = ((available_width / CARD_MIN_WIDTH).floor() as usize).clamp(1, MAX_CARD_COLUMNS);
    let spacing = CARD_ROW_GAP * columns.saturating_sub(1) as f64;
    let card_width = ((available_width - spacing) / columns as f64).max(180.0);

    (columns, card_width)
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

    image_height + 96.0 + title_lines * 18.0
}

fn button_from_style(
    entity: Entity,
    action: AppAction,
    label_text: impl Into<String>,
    style: &ResolvedStyle,
) -> UiView {
    let text_color = style.colors.text.unwrap_or(Color::WHITE);
    Arc::new(apply_direct_widget_style(
        button(entity, action, label_text.into()).color(text_color),
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
    let sidebar = children.next().unwrap_or_else(empty_ui);
    let main_content = children.next().unwrap_or_else(empty_ui);

    Arc::new(apply_widget_style(
        flex_row((
            sized_box(sidebar)
                .dims((Length::px(sidebar_width), Dim::Stretch))
                .into_any_flex(),
            main_content.flex(1.0).into_any_flex(),
        ))
        .main_axis_alignment(MainAxisAlignment::Start)
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
    children.extend(ctx.children.into_iter().map(|child| child.into_any_flex()));

    Arc::new(
        portal(
            flex_col(children)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .width(Dim::Stretch),
        )
        .dims(Dim::Stretch),
    )
}

pub(super) fn project_auth_panel(_: &PixivAuthPanel, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let input_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);
    let auth = ctx.world.resource::<AuthState>();
    let ui_components = *ctx.world.resource::<PixivUiComponents>();
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
            &input_style,
        )
        .into_any_flex(),
        sized_box(apply_text_input_style(
            text_input(
                ctx.entity,
                auth.code_verifier_input.clone(),
                AppAction::SetCodeVerifier,
            )
            .placeholder(tr(
                ctx.world,
                "pixiv.auth.placeholder.pkce",
                "PKCE code_verifier",
            )),
            &input_style,
        ))
        .width(Dim::Stretch)
        .into_any_flex(),
        sized_box(apply_text_input_style(
            text_input(
                ctx.entity,
                auth.auth_code_input.clone(),
                AppAction::SetAuthCode,
            )
            .placeholder(tr(ctx.world, "pixiv.auth.placeholder.code", "Auth code")),
            &input_style,
        ))
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
        sized_box(apply_text_input_style(
            text_input(
                ctx.entity,
                auth.refresh_token_input.clone(),
                AppAction::SetRefreshToken,
            )
            .placeholder(tr(
                ctx.world,
                "pixiv.auth.placeholder.refresh_token",
                "Refresh token",
            )),
            &input_style,
        ))
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

    if panel.content.trim().is_empty() {
        return empty_ui();
    }

    let lines = panel
        .content
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let lines = Arc::new(lines);
    let line_count = i64::try_from(lines.len()).unwrap_or(i64::MAX);

    Arc::new(
        flex_col((
            label(panel.title.clone()).into_any_flex(),
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
                move |_, idx| {
                    let row_idx = usize::try_from(idx).unwrap_or(0);
                    Arc::new(label(lines.get(row_idx).cloned().unwrap_or_default())) as UiView
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
    let input_style = resolve_style_for_classes(ctx.world, ["pixiv.root"]);

    Arc::new(
        flex_row((
            apply_text_input_style(
                text_input(ctx.entity, ui.search_text.clone(), AppAction::SetSearchText)
                    .placeholder(tr(
                        ctx.world,
                        "pixiv.search.placeholder",
                        "Search illust keyword",
                    )),
                &input_style,
            )
            .flex(1.0),
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
        return Arc::new(label(tr(
            ctx.world,
            "pixiv.feed.empty",
            "No data yet. Login first, then switch tabs.",
        )));
    }

    let ui = ctx.world.resource::<UiState>();
    let viewport = ctx.world.resource::<ViewportMetrics>();
    let (columns, _) = compute_feed_layout(viewport.width as f64, ui.sidebar_collapsed);
    let columns = columns.max(1);

    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut column_heights = vec![0.0_f64; columns];
    let mut column_views = vec![Vec::<UiView>::new(); columns];

    for (entity, child_view) in child_entities.into_iter().zip(ctx.children.into_iter()) {
        let estimated_height = estimate_illust_card_height(ctx.world, entity, CARD_BASE_WIDTH);
        let target_column = column_heights
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap_or(0);

        column_heights[target_column] += estimated_height + CARD_ROW_GAP;
        column_views[target_column].push(child_view);
    }

    let columns_ui = column_views
        .into_iter()
        .map(|items| {
            flex_col(
                items
                    .into_iter()
                    .map(|item| item.into_any_flex())
                    .collect::<Vec<_>>(),
            )
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(CARD_ROW_GAP))
            .into_any_flex()
        })
        .collect::<Vec<_>>();

    Arc::new(
        flex_row(columns_ui)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(Length::px(CARD_ROW_GAP))
            .width(Dim::Stretch),
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

fn illust_author_row(author: &str, avatar: UiView, style: &ResolvedStyle) -> UiView {
    Arc::new(flex_row((
        avatar.into_any_flex(),
        apply_label_style(label(author.to_string()), style).into_any_flex(),
    )))
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

    let ui = ctx.world.resource::<UiState>();
    let viewport = ctx.world.resource::<ViewportMetrics>();
    let (_, card_width) = compute_feed_layout(viewport.width as f64, ui.sidebar_collapsed);

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
    .fixed_width(Length::px(46.0));

    Arc::new(
        sized_box(apply_widget_style(
            flex_col(vec![
                button_with_child(
                    action_entities.open_thumbnail,
                    AppAction::OpenIllust(ctx.entity),
                    sized_box(illust_thumbnail_view(ctx.world, illust, &visual))
                        .dims((Dim::Stretch, Length::px(image_height))),
                )
                .padding(0.0)
                .border(Color::TRANSPARENT, 0.0)
                .background_color(Color::TRANSPARENT)
                .into_any_flex(),
                apply_label_style(label(illust.title.clone()), &style).into_any_flex(),
                illust_author_row(
                    &illust.user.name,
                    illust_avatar_view(&visual, &style),
                    &style,
                )
                .into_any_flex(),
                illust_stats_view(illust, &style).into_any_flex(),
                flex_row((heart_button.into_any_flex(),))
                    .main_axis_alignment(MainAxisAlignment::End)
                    .into_any_flex(),
            ]),
            &style,
        ))
        .fixed_width(Length::px((card_width * anim.card_scale as f64).max(180.0))),
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
                label(illust.title.clone()).into_any_flex(),
                label(format!(
                    "{} {}",
                    tr(ctx.world, "pixiv.overlay.author", "Author:"),
                    illust.user.name
                ))
                .into_any_flex(),
                label(format!(
                    "{} {}  {} {}  {} {}",
                    tr(ctx.world, "pixiv.overlay.views", "Views"),
                    illust.total_view,
                    tr(ctx.world, "pixiv.overlay.bookmarks", "Bookmarks"),
                    illust.total_bookmarks,
                    tr(ctx.world, "pixiv.overlay.comments", "Comments"),
                    illust.total_comments
                ))
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

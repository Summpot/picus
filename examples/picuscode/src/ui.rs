//! `UiComponentTemplate` implementations for picuscode view markers.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use picus::{
    ProjectionCtx, UiComponentTemplate, UiView, apply_direct_text_input_style, apply_label_style,
    apply_widget_style,
    bevy_ecs::hierarchy::Children,
    button_with_child, emit_ui_action,
    icon::icon,
    icons::{FluentIcon, IconGlyph},
    masonry_core::{
        layout::{Dim, Length},
        properties::Dimensions,
    },
    resolve_style, resolve_style_for_classes, text_input,
    xilem::{
        Color, InsertNewline,
        style::Style as _,
        view::{
            CrossAxisAlignment, FlexExt as _, MainAxisAlignment, flex_col, flex_item, flex_row,
            label, sized_box,
        },
    },
};

use crate::action::PicusCodeAction;
use crate::bridge::ThreadSummary;
use crate::state::*;

impl UiComponentTemplate for ChatRootView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let children = child_entities(&ctx)
            .into_iter()
            .zip(ctx.children)
            .map(|(entity, child)| {
                let grow = resolve_style(ctx.world, entity).layout.flex_grow;
                if grow > 0.0 {
                    flex_item(child, grow).into_any_flex()
                } else {
                    child.into_any_flex()
                }
            })
            .collect::<Vec<_>>();
        Arc::new(
            sized_box(apply_widget_style(
                flex_col(children)
                    .width(Dim::Stretch)
                    .height(Dim::Stretch)
                    .gap(Length::px(style.layout.gap)),
                &style,
            ))
            .dims(
                Dimensions::AUTO
                    .with_width(Dim::Stretch)
                    .with_height(Dim::Stretch),
            ),
        )
    }
}

impl UiComponentTemplate for ChatTitleBarView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let snapshot = HeaderSnapshot::from_state(&ctx);
        let title = text_view(&ctx, ["picuscode.title"], "picuscode");
        let subtitle = text_view(&ctx, ["picuscode.subtitle"], snapshot.subtitle);
        let brand = flex_row(vec![
            brand_mark(&ctx, 30.0).into_any_flex(),
            flex_col(vec![title.into_any_flex(), subtitle.into_any_flex()])
                .gap(Length::px(1.0))
                .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(9.0));

        let chips = flex_row(vec![
            chip_view(&ctx, snapshot.provider_chip, ChipTone::Neutral).into_any_flex(),
            chip_view(&ctx, snapshot.model_chip, ChipTone::Accent).into_any_flex(),
            chip_view(&ctx, snapshot.stream_chip, ChipTone::Success).into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0));

        let new_btn = primary_button(&ctx, PicusCodeAction::NewThread, "New", FluentIcon::Add);
        let settings_btn = toolbar_button(
            &ctx,
            PicusCodeAction::OpenSettings,
            "Settings",
            FluentIcon::Settings,
        );
        let about_btn =
            toolbar_button(&ctx, PicusCodeAction::OpenAbout, "About", FluentIcon::Info);
        Arc::new(apply_widget_style(
            flex_row(vec![
                sized_box(brand).flex(1.0).into_any_flex(),
                chips.into_any_flex(),
                new_btn.into_any_flex(),
                settings_btn.into_any_flex(),
                about_btn.into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(8.0)),
            &style,
        ))
    }
}

impl UiComponentTemplate for ChatBodyView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let children = child_entities(&ctx)
            .into_iter()
            .zip(ctx.children)
            .map(|(entity, child)| {
                let grow = resolve_style(ctx.world, entity).layout.flex_grow;
                if grow > 0.0 {
                    flex_item(child, grow).into_any_flex()
                } else {
                    child.into_any_flex()
                }
            })
            .collect::<Vec<_>>();
        Arc::new(apply_widget_style(
            flex_row(children)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(Length::px(style.layout.gap)),
            &style,
        ))
    }
}

impl UiComponentTemplate for SidebarColumnView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let state = ctx.world.get_resource::<PicusState>();
        let active_thread = state.and_then(|s| s.active_thread.clone());
        let threads = state.map(|s| s.threads.clone()).unwrap_or_default();

        let mut items: Vec<_> = Vec::with_capacity(threads.len() + 5);
        let thread_count = threads.len();
        let active_count = threads.iter().filter(|t| !t.archived).count();

        items.push(sidebar_brand_block(&ctx).into_any_flex());
        items.push(
            sized_box(primary_button(
                &ctx,
                PicusCodeAction::NewThread,
                "New session",
                FluentIcon::Edit,
            ))
            .width(Length::px(220.0))
            .into_any_flex(),
        );
        items.push(sidebar_section_header(&ctx, active_count, thread_count).into_any_flex());

        if threads.is_empty() {
            items.push(sidebar_empty_state(&ctx).into_any_flex());
        }
        for t in threads {
            let is_active = active_thread.as_deref() == Some(t.id.as_str());
            items.push(sidebar_thread_item(&ctx, &t, is_active).into_any_flex());
        }
        items.push(sidebar_footer(&ctx).into_any_flex());

        Arc::new(apply_widget_style(
            sized_box(flex_col(items).gap(Length::px(style.layout.gap))).width(Length::px(244.0)),
            &style,
        ))
    }
}

impl UiComponentTemplate for TranscriptColumnView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let state = ctx.world.get_resource::<PicusState>();
        let summary = TranscriptSummary::from_state(state);
        let mut rows = Vec::with_capacity(ctx.children.len() + 4);
        rows.push(transcript_header(&ctx, &summary).into_any_flex());

        if summary.active_thread.is_none()
            || (summary.message_count == 0 && ctx.children.is_empty())
        {
            rows.push(transcript_empty_state(&ctx, &summary).into_any_flex());
        }

        rows.extend(ctx.children.into_iter().map(|child| child.into_any_flex()));

        Arc::new(apply_widget_style(
            flex_col(rows)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(Length::px(style.layout.gap)),
            &style,
        ))
    }
}

impl UiComponentTemplate for MessageRowView {
    fn project(row: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let role = message_role(row.role.as_str());
        let row_style = resolve_style_for_classes(
            ctx.world,
            ["picuscode.message.row", message_role_row_class(role)],
        );
        let body_style = resolve_style_for_classes(ctx.world, ["picuscode.message.stack"]);
        let mut children = Vec::with_capacity(ctx.children.len() + 1);
        children.push(message_meta(&ctx, row, role).into_any_flex());
        children.extend(ctx.children.into_iter().map(|child| child.into_any_flex()));

        let alignment = if matches!(role, MessageRole::User) {
            CrossAxisAlignment::End
        } else {
            CrossAxisAlignment::Stretch
        };

        Arc::new(apply_widget_style(
            apply_widget_style(
                flex_col(children)
                    .cross_axis_alignment(alignment)
                    .gap(Length::px(body_style.layout.gap)),
                &body_style,
            ),
            &row_style,
        ))
    }
}

impl UiComponentTemplate for ComposerView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let state = ctx.world.get_resource::<PicusState>();
        let draft = ctx
            .world
            .get_resource::<PicusState>()
            .map(|s| s.draft.clone())
            .unwrap_or_default();
        let streaming = state.is_some_and(|s| s.streaming);
        let draft_count = draft_len(&draft);
        let input_entity = ctx.entity;
        let enter_entity = ctx.entity;
        let input_style = resolve_style_for_classes(ctx.world, ["picuscode.text-input"]);
        let input_row_style =
            resolve_style_for_classes(ctx.world, ["picuscode.composer.input-row"]);
        let input = apply_direct_text_input_style(
            text_input(input_entity, draft, PicusCodeAction::ComposerChanged)
                .placeholder("Message CodeWhale...")
                .insert_newline(InsertNewline::OnShiftEnter)
                .on_enter(move |_| {
                    emit_ui_action(enter_entity, PicusCodeAction::Send);
                }),
            &input_style,
        );
        let action_btn = if streaming {
            toolbar_button(
                &ctx,
                PicusCodeAction::CancelTurn,
                "Stop",
                FluentIcon::Stop,
            )
        } else {
            primary_button(&ctx, PicusCodeAction::Send, "Send", FluentIcon::Send)
        };
        let selected = state.and_then(|s| s.active_thread.as_deref()).is_some();
        let caret = if streaming { "…" } else { "›" };
        Arc::new(apply_widget_style(
            flex_col(vec![
                apply_widget_style(
                    flex_row(vec![
                        text_view(&ctx, ["picuscode.composer.caret"], caret).into_any_flex(),
                        input.flex(1.0).into_any_flex(),
                        action_btn.into_any_flex(),
                    ])
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(Length::px(8.0)),
                    &input_row_style,
                )
                .into_any_flex(),
                composer_context_bar(&ctx, state, draft_count, streaming, selected).into_any_flex(),
            ])
            .gap(Length::px(style.layout.gap)),
            &style,
        ))
    }
}

impl UiComponentTemplate for StatusLineView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let state = ctx.world.get_resource::<PicusState>();
        let metrics = state
            .map(|s| {
                vec![
                    status_metric(
                        &ctx,
                        FluentIcon::Accept,
                        "state",
                        truncate_preview(&s.status, 42),
                        if s.streaming {
                            ChipTone::Success
                        } else {
                            ChipTone::Neutral
                        },
                    ),
                    status_metric(
                        &ctx,
                        FluentIcon::Message,
                        "threads",
                        s.threads.len().to_string(),
                        ChipTone::Neutral,
                    ),
                    status_metric(
                        &ctx,
                        FluentIcon::List,
                        "messages",
                        s.messages.len().to_string(),
                        ChipTone::Neutral,
                    ),
                    status_metric(
                        &ctx,
                        FluentIcon::Globe,
                        "provider",
                        config_summary_value(s, "provider", "unset"),
                        ChipTone::Accent,
                    ),
                    status_metric(
                        &ctx,
                        FluentIcon::Contact,
                        "model",
                        config_summary_value(s, "model", "unset"),
                        ChipTone::Neutral,
                    ),
                ]
            })
            .unwrap_or_else(|| {
                vec![status_metric(
                    &ctx,
                    FluentIcon::Sync,
                    "state",
                    "Bridge starting",
                    ChipTone::Neutral,
                )]
            });
        Arc::new(apply_widget_style(
            flex_row(
                metrics
                    .into_iter()
                    .map(|metric| metric.into_any_flex())
                    .collect::<Vec<_>>(),
            )
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(6.0)),
            &style,
        ))
    }
}

impl UiComponentTemplate for AboutRootView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let close_btn =
            toolbar_button(&ctx, PicusCodeAction::CloseAbout, "Close", FluentIcon::Cancel);
        let children = ctx
            .children
            .into_iter()
            .map(|child| child.into_any_flex())
            .collect::<Vec<_>>();
        let mut all = children;
        all.push(close_btn.into_any_flex());
        Arc::new(apply_widget_style(
            flex_col(all)
                .width(Dim::Stretch)
                .height(Dim::Stretch)
                .gap(Length::px(12.0)),
            &style,
        ))
    }
}

impl UiComponentTemplate for SettingsRootView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let close_btn = toolbar_button(
            &ctx,
            PicusCodeAction::CloseSettings,
            "Close",
            FluentIcon::Cancel,
        );
        let save_btn = toolbar_button(
            &ctx,
            PicusCodeAction::ApplyConfigEdits,
            "Save",
            FluentIcon::Accept,
        );
        let reload_btn = toolbar_button(
            &ctx,
            PicusCodeAction::ReloadConfig,
            "Reload",
            FluentIcon::Refresh,
        );
        let children = ctx
            .children
            .into_iter()
            .map(|child| child.into_any_flex())
            .collect::<Vec<_>>();
        let mut all = children;
        all.push(
            flex_row(vec![
                sized_box(reload_btn).flex(1.0).into_any_flex(),
                save_btn.into_any_flex(),
                close_btn.into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(8.0))
            .into_any_flex(),
        );
        Arc::new(apply_widget_style(
            flex_col(all)
                .width(Dim::Stretch)
                .height(Dim::Stretch)
                .gap(Length::px(12.0)),
            &style,
        ))
    }
}

impl UiComponentTemplate for SettingsFormView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let state = ctx.world.get_resource::<PicusState>();

        let mut rows: Vec<_> = Vec::new();
        rows.push(settings_header(&ctx, state).into_any_flex());

        let values = state.map(|s| s.config_values.clone()).unwrap_or_default();
        let edits = state.map(|s| s.config_edits.clone()).unwrap_or_default();
        let active_provider = edits
            .get("provider")
            .or_else(|| values.get("provider"))
            .cloned()
            .unwrap_or_default();
        rows.push(
            settings_section(
                &ctx,
                "Connection",
                &values,
                &edits,
                &active_provider,
                &[
                    ("provider", "Provider", ConfigScope::Top),
                    ("model", "Model", ConfigScope::ProviderOrTop),
                    ("api_key", "API Key", ConfigScope::ProviderOrTop),
                    ("base_url", "Base URL", ConfigScope::ProviderOrTop),
                ],
            )
            .into_any_flex(),
        );
        rows.push(
            settings_section(
                &ctx,
                "Runtime",
                &values,
                &edits,
                &active_provider,
                &[
                    ("auth.mode", "Auth Mode", ConfigScope::Top),
                    ("telemetry", "Telemetry", ConfigScope::Top),
                ],
            )
            .into_any_flex(),
        );
        rows.push(
            settings_section(
                &ctx,
                "Safety",
                &values,
                &edits,
                &active_provider,
                &[
                    ("approval_policy", "Approval Policy", ConfigScope::Top),
                    ("sandbox_mode", "Sandbox Mode", ConfigScope::Top),
                ],
            )
            .into_any_flex(),
        );

        if let Some(s) = state
            && let Some(status) = &s.config_status
        {
            rows.push(
                text_view(&ctx, ["picuscode.settings.status"], status.as_str()).into_any_flex(),
            );
        }

        Arc::new(apply_widget_style(
            flex_col(rows)
                .cross_axis_alignment(CrossAxisAlignment::Stretch)
                .gap(Length::px(style.layout.gap)),
            &style,
        ))
    }
}

fn child_entities(ctx: &ProjectionCtx<'_>) -> Vec<picus::bevy_ecs::entity::Entity> {
    ctx.world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default()
}

fn text_view<const N: usize>(
    ctx: &ProjectionCtx<'_>,
    classes: [&'static str; N],
    text: impl Into<String>,
) -> UiView {
    let style = resolve_style_for_classes(ctx.world, classes);
    Arc::new(apply_label_style(label(text.into()), &style))
}

fn toolbar_button(
    ctx: &ProjectionCtx<'_>,
    action: PicusCodeAction,
    text: &'static str,
    glyph: impl Into<IconGlyph>,
) -> UiView {
    let glyph = glyph.into();
    let style = resolve_style_for_classes(ctx.world, ["picuscode.toolbar.button"]);
    let text_style = resolve_style_for_classes(ctx.world, ["picuscode.toolbar.button.text"]);
    let icon_color = text_style.colors.text.unwrap_or(Color::WHITE);
    let content = flex_row(vec![
        icon(glyph, 16.0, icon_color).into_any_flex(),
        Arc::new(apply_label_style(label(text), &text_style)).into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .main_axis_alignment(MainAxisAlignment::Center)
    .gap(Length::px(6.0));
    Arc::new(apply_widget_style(
        button_with_child(ctx.entity, action, content),
        &style,
    ))
}

fn primary_button(
    ctx: &ProjectionCtx<'_>,
    action: PicusCodeAction,
    text: &'static str,
    glyph: impl Into<IconGlyph>,
) -> UiView {
    let glyph = glyph.into();
    let style = resolve_style_for_classes(ctx.world, ["picuscode.primary.button"]);
    let text_style = resolve_style_for_classes(ctx.world, ["picuscode.primary.button.text"]);
    let icon_color = text_style.colors.text.unwrap_or(Color::WHITE);
    let content = flex_row(vec![
        icon(glyph, 15.0, icon_color).into_any_flex(),
        Arc::new(apply_label_style(label(text), &text_style)).into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .main_axis_alignment(MainAxisAlignment::Center)
    .gap(Length::px(8.0));
    Arc::new(apply_widget_style(
        button_with_child(ctx.entity, action, content),
        &style,
    ))
}

fn brand_mark(ctx: &ProjectionCtx<'_>, size: f64) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.brand.mark"]);
    let icon_style = resolve_style_for_classes(ctx.world, ["picuscode.brand.mark.icon"]);
    let icon_color = icon_style.colors.text.unwrap_or(Color::WHITE);
    Arc::new(apply_widget_style(
        sized_box(icon(FluentIcon::Contact, size * 0.46, icon_color))
            .width(Length::px(size))
            .height(Length::px(size)),
        &style,
    ))
}

fn shortcut_hint(ctx: &ProjectionCtx<'_>, key: &'static str, label_text: &'static str) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.shortcut"]);
    let key_style = resolve_style_for_classes(ctx.world, ["picuscode.shortcut.key"]);
    let label_style = resolve_style_for_classes(ctx.world, ["picuscode.shortcut.label"]);
    Arc::new(apply_widget_style(
        flex_row(vec![
            Arc::new(apply_label_style(label(key), &key_style)).into_any_flex(),
            Arc::new(apply_label_style(label(label_text), &label_style)).into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(5.0)),
        &style,
    ))
}

fn suggestion_button(ctx: &ProjectionCtx<'_>, prompt: &'static str, meta: &'static str) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.suggestion"]);
    let title_style = resolve_style_for_classes(ctx.world, ["picuscode.suggestion.title"]);
    let meta_style = resolve_style_for_classes(ctx.world, ["picuscode.suggestion.meta"]);
    let content = flex_col(vec![
        Arc::new(apply_label_style(label(prompt), &title_style)).into_any_flex(),
        Arc::new(apply_label_style(label(meta), &meta_style)).into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(Length::px(3.0));
    Arc::new(apply_widget_style(
        button_with_child(
            ctx.entity,
            PicusCodeAction::ComposerChanged(prompt.to_string()),
            content,
        ),
        &style,
    ))
}

fn sidebar_brand_block(ctx: &ProjectionCtx<'_>) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.brand"]);
    Arc::new(apply_widget_style(
        flex_row(vec![
            brand_mark(ctx, 34.0).into_any_flex(),
            flex_col(vec![
                text_view(ctx, ["picuscode.sidebar.brand.title"], "picuscode").into_any_flex(),
                text_view(ctx, ["picuscode.sidebar.brand.meta"], "CodeWhale desktop")
                    .into_any_flex(),
            ])
            .gap(Length::px(1.0))
            .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(10.0)),
        &style,
    ))
}

fn sidebar_section_header(
    ctx: &ProjectionCtx<'_>,
    active_count: usize,
    thread_count: usize,
) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.section"]);
    Arc::new(apply_widget_style(
        flex_row(vec![
            text_view(ctx, ["picuscode.sidebar.heading"], "Sessions").into_any_flex(),
            sized_box(label("")).flex(1.0).into_any_flex(),
            text_view(
                ctx,
                ["picuscode.sidebar.caption"],
                format!("{active_count}/{thread_count}"),
            )
            .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center),
        &style,
    ))
}

fn sidebar_thread_item(ctx: &ProjectionCtx<'_>, thread: &ThreadSummary, is_active: bool) -> UiView {
    let name = thread
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_thread_title(&thread.preview, &thread.id));
    let preview = if thread.preview.trim().is_empty() {
        "No preview yet".to_string()
    } else {
        truncate_preview(&thread.preview, 48)
    };
    let item_style = if is_active {
        resolve_style_for_classes(
            ctx.world,
            ["picuscode.thread.item", "picuscode.thread.item.active"],
        )
    } else {
        resolve_style_for_classes(ctx.world, ["picuscode.thread.item"])
    };
    let item_title_style = if is_active {
        resolve_style_for_classes(
            ctx.world,
            ["picuscode.thread.title", "picuscode.thread.title.active"],
        )
    } else {
        resolve_style_for_classes(ctx.world, ["picuscode.thread.title"])
    };
    let icon_style = if is_active {
        resolve_style_for_classes(
            ctx.world,
            ["picuscode.thread.icon", "picuscode.thread.icon.active"],
        )
    } else {
        resolve_style_for_classes(ctx.world, ["picuscode.thread.icon"])
    };
    let icon_color = icon_style.colors.text.unwrap_or(Color::WHITE);
    let meta = format!(
        "{} · {}",
        clean_provider(&thread.model_provider),
        format_short_timestamp(thread.updated_at)
    );
    let mut title_row = vec![
        icon(FluentIcon::Message, 13.0, icon_color).into_any_flex(),
        sized_box(Arc::new(apply_label_style(
            label(truncate_preview(&name, 30)),
            &item_title_style,
        )))
        .flex(1.0)
        .into_any_flex(),
    ];
    if thread.archived {
        title_row.push(chip_view(ctx, "archived", ChipTone::Danger).into_any_flex());
    } else if is_active {
        title_row.push(chip_view(ctx, "current", ChipTone::Accent).into_any_flex());
    }
    let content = flex_col(vec![
        flex_row(title_row)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(7.0))
            .into_any_flex(),
        text_view(ctx, ["picuscode.thread.preview"], preview).into_any_flex(),
        text_view(ctx, ["picuscode.thread.meta"], meta).into_any_flex(),
    ])
    .gap(Length::px(3.0));
    let btn = button_with_child(
        ctx.entity,
        PicusCodeAction::SelectThread(thread.id.clone()),
        apply_widget_style(content, &item_style),
    );
    Arc::new(btn)
}

fn sidebar_empty_state(ctx: &ProjectionCtx<'_>) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.empty"]);
    Arc::new(apply_widget_style(
        flex_col(vec![
            text_view(ctx, ["picuscode.empty.title"], "No sessions").into_any_flex(),
            text_view(
                ctx,
                ["picuscode.empty.body"],
                "Create one to sync CodeWhale state.",
            )
            .into_any_flex(),
        ])
        .gap(Length::px(4.0)),
        &style,
    ))
}

fn sidebar_footer(ctx: &ProjectionCtx<'_>) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.footer"]);
    Arc::new(apply_widget_style(
        flex_col(vec![
            sidebar_nav_button(
                ctx,
                PicusCodeAction::OpenSettings,
                "Settings",
                FluentIcon::Settings,
            )
            .into_any_flex(),
            sidebar_nav_button(ctx, PicusCodeAction::OpenAbout, "About", FluentIcon::Info)
                .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(Length::px(4.0)),
        &style,
    ))
}

fn sidebar_nav_button(
    ctx: &ProjectionCtx<'_>,
    action: PicusCodeAction,
    text: &'static str,
    glyph: impl Into<IconGlyph>,
) -> UiView {
    let glyph = glyph.into();
    let style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.nav"]);
    let text_style = resolve_style_for_classes(ctx.world, ["picuscode.sidebar.nav.text"]);
    let icon_color = text_style.colors.text.unwrap_or(Color::WHITE);
    let content = flex_row(vec![
        icon(glyph, 14.0, icon_color).into_any_flex(),
        Arc::new(apply_label_style(label(text), &text_style)).into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(Length::px(8.0));
    Arc::new(apply_widget_style(
        button_with_child(ctx.entity, action, content),
        &style,
    ))
}

#[derive(Clone, Copy)]
enum ChipTone {
    Neutral,
    Accent,
    Success,
    Danger,
}

fn chip_view(ctx: &ProjectionCtx<'_>, text: impl Into<String>, tone: ChipTone) -> UiView {
    let style = match tone {
        ChipTone::Neutral => resolve_style_for_classes(ctx.world, ["picuscode.chip"]),
        ChipTone::Accent => {
            resolve_style_for_classes(ctx.world, ["picuscode.chip", "picuscode.chip.accent"])
        }
        ChipTone::Success => {
            resolve_style_for_classes(ctx.world, ["picuscode.chip", "picuscode.chip.success"])
        }
        ChipTone::Danger => {
            resolve_style_for_classes(ctx.world, ["picuscode.chip", "picuscode.chip.danger"])
        }
    };
    Arc::new(apply_widget_style(
        apply_label_style(label(text.into()), &style),
        &style,
    ))
}

fn status_metric(
    ctx: &ProjectionCtx<'_>,
    glyph: impl Into<IconGlyph>,
    label_text: &'static str,
    value: impl Into<String>,
    tone: ChipTone,
) -> UiView {
    let glyph = glyph.into();
    let style = match tone {
        ChipTone::Neutral => resolve_style_for_classes(ctx.world, ["picuscode.status.metric"]),
        ChipTone::Accent => resolve_style_for_classes(
            ctx.world,
            ["picuscode.status.metric", "picuscode.status.metric.accent"],
        ),
        ChipTone::Success => resolve_style_for_classes(
            ctx.world,
            ["picuscode.status.metric", "picuscode.status.metric.success"],
        ),
        ChipTone::Danger => resolve_style_for_classes(
            ctx.world,
            ["picuscode.status.metric", "picuscode.status.metric.danger"],
        ),
    };
    let label_style = resolve_style_for_classes(ctx.world, ["picuscode.status.metric.label"]);
    let value_style = resolve_style_for_classes(ctx.world, ["picuscode.status.metric.value"]);
    let icon_color = value_style.colors.text.unwrap_or(Color::WHITE);
    Arc::new(apply_widget_style(
        flex_row(vec![
            icon(glyph, 12.0, icon_color).into_any_flex(),
            Arc::new(apply_label_style(label(label_text), &label_style)).into_any_flex(),
            Arc::new(apply_label_style(label(value.into()), &value_style)).into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(5.0)),
        &style,
    ))
}

#[derive(Clone, Copy)]
enum MessageRole {
    User,
    Assistant,
    System,
    Other,
}

fn message_role(role: &str) -> MessageRole {
    match role {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" | "history" => MessageRole::System,
        _ => MessageRole::Other,
    }
}

fn message_role_row_class(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "picuscode.message.row.user",
        MessageRole::Assistant => "picuscode.message.row.assistant",
        MessageRole::System => "picuscode.message.row.system",
        MessageRole::Other => "picuscode.message.row.other",
    }
}

fn message_role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "You",
        MessageRole::Assistant => "CodeWhale",
        MessageRole::System => "System",
        MessageRole::Other => "Message",
    }
}

fn message_role_icon(role: MessageRole) -> FluentIcon {
    match role {
        MessageRole::User => FluentIcon::Contact,
        MessageRole::Assistant => FluentIcon::Contact,
        MessageRole::System | MessageRole::Other => FluentIcon::Info,
    }
}

fn message_meta(ctx: &ProjectionCtx<'_>, row: &MessageRowView, role: MessageRole) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.message.meta"]);
    let author_style = resolve_style_for_classes(ctx.world, ["picuscode.message.author"]);
    let time_style = resolve_style_for_classes(ctx.world, ["picuscode.message.time"]);
    let icon_color = author_style.colors.text.unwrap_or(Color::WHITE);
    let time = if row.streaming {
        "streaming".to_string()
    } else {
        format_short_timestamp(row.created_at)
    };
    Arc::new(apply_widget_style(
        flex_row(vec![
            icon(message_role_icon(role), 13.0, icon_color).into_any_flex(),
            Arc::new(apply_label_style(
                label(message_role_label(role)),
                &author_style,
            ))
            .into_any_flex(),
            Arc::new(apply_label_style(label(time), &time_style)).into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0)),
        &style,
    ))
}

#[derive(Default)]
struct HeaderSnapshot {
    subtitle: String,
    provider_chip: String,
    model_chip: String,
    stream_chip: String,
}

impl HeaderSnapshot {
    fn from_state(ctx: &ProjectionCtx<'_>) -> Self {
        let Some(state) = ctx.world.get_resource::<PicusState>() else {
            return Self {
                subtitle: "Bridge starting".to_string(),
                provider_chip: "provider pending".to_string(),
                model_chip: "model pending".to_string(),
                stream_chip: "idle".to_string(),
            };
        };
        let active = state
            .active_thread
            .as_deref()
            .and_then(|id| state.threads.iter().find(|thread| thread.id == id));
        let subtitle = active
            .map(|thread| {
                fallback_thread_title(
                    thread.name.as_deref().unwrap_or(&thread.preview),
                    &thread.id,
                )
            })
            .unwrap_or_else(|| "No active thread".to_string());
        let provider_chip = config_summary_value(state, "provider", "provider unset");
        let model_chip = config_summary_value(state, "model", "model unset");
        let stream_chip = if state.streaming { "streaming" } else { "idle" }.to_string();
        Self {
            subtitle,
            provider_chip,
            model_chip,
            stream_chip,
        }
    }
}

#[derive(Default)]
struct TranscriptSummary {
    active_thread: Option<String>,
    title: String,
    subtitle: String,
    message_count: usize,
    user_count: usize,
    assistant_count: usize,
    streaming: bool,
}

impl TranscriptSummary {
    fn from_state(state: Option<&PicusState>) -> Self {
        let Some(state) = state else {
            return Self {
                title: "Starting bridge".to_string(),
                subtitle: "Waiting for CodeWhale state".to_string(),
                ..Default::default()
            };
        };
        let active_thread = state.active_thread.clone();
        let active = active_thread
            .as_deref()
            .and_then(|id| state.threads.iter().find(|thread| thread.id == id));
        let title = active
            .map(|thread| thread.name.as_deref().unwrap_or(&thread.preview))
            .filter(|title| !title.trim().is_empty())
            .map(|title| truncate_preview(title, 54))
            .unwrap_or_else(|| {
                active_thread
                    .as_deref()
                    .map(|id| fallback_thread_title("", id))
                    .unwrap_or_else(|| "No active thread".to_string())
            });
        let subtitle = active
            .map(|thread| {
                format!(
                    "{} · updated {}",
                    clean_provider(&thread.model_provider),
                    format_timestamp(thread.updated_at)
                )
            })
            .unwrap_or_else(|| "Thread list is shared with CodeWhale".to_string());
        let user_count = state
            .messages
            .iter()
            .filter(|message| message.role == "user")
            .count();
        let assistant_count = state
            .messages
            .iter()
            .filter(|message| message.role == "assistant")
            .count();
        Self {
            active_thread,
            title,
            subtitle,
            message_count: state.messages.len(),
            user_count,
            assistant_count,
            streaming: state.streaming,
        }
    }
}

fn transcript_header(ctx: &ProjectionCtx<'_>, summary: &TranscriptSummary) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.transcript.header"]);
    let state_chip = if summary.streaming {
        chip_view(ctx, "streaming", ChipTone::Success)
    } else {
        chip_view(
            ctx,
            format!("{} messages", summary.message_count),
            ChipTone::Neutral,
        )
    };
    Arc::new(apply_widget_style(
        flex_row(vec![
            flex_col(vec![
                text_view(ctx, ["picuscode.transcript.title"], summary.title.clone())
                    .into_any_flex(),
                text_view(
                    ctx,
                    ["picuscode.transcript.subtitle"],
                    summary.subtitle.clone(),
                )
                .into_any_flex(),
            ])
            .gap(Length::px(2.0))
            .into_any_flex(),
            sized_box(flex_row(vec![
                chip_view(
                    ctx,
                    format!("{} user", summary.user_count),
                    ChipTone::Accent,
                )
                .into_any_flex(),
                chip_view(
                    ctx,
                    format!("{} assistant", summary.assistant_count),
                    ChipTone::Neutral,
                )
                .into_any_flex(),
                state_chip.into_any_flex(),
            ]))
            .flex(1.0)
            .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(12.0)),
        &style,
    ))
}

fn transcript_empty_state(ctx: &ProjectionCtx<'_>, summary: &TranscriptSummary) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.empty.panel"]);
    let (title, body, primary_prompt) = if summary.active_thread.is_none() {
        (
            "picuscode",
            "A CodeWhale desktop shell for focused coding sessions.",
            "Explain this workspace's architecture",
        )
    } else {
        (
            "Fresh thread",
            "Describe a task or ask a question to start the turn.",
            "Summarize the recent changes in this thread",
        )
    };
    let new_btn = toolbar_button(
        ctx,
        PicusCodeAction::NewThread,
        "New thread",
        FluentIcon::Add,
    );
    Arc::new(apply_widget_style(
        flex_col(vec![
            brand_mark(ctx, 54.0).into_any_flex(),
            text_view(ctx, ["picuscode.empty.title"], title).into_any_flex(),
            text_view(ctx, ["picuscode.empty.body"], body).into_any_flex(),
            flex_row(vec![
                shortcut_hint(ctx, "/", "commands").into_any_flex(),
                shortcut_hint(ctx, "@", "files").into_any_flex(),
                shortcut_hint(ctx, "Enter", "send").into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(8.0))
            .into_any_flex(),
            flex_col(vec![
                suggestion_button(ctx, primary_prompt, "Start with a repository-level map")
                    .into_any_flex(),
                suggestion_button(
                    ctx,
                    "Find the riskiest TODOs",
                    "Scan for work that needs attention",
                )
                .into_any_flex(),
                suggestion_button(
                    ctx,
                    "Draft a focused implementation plan",
                    "Prepare a short next-step checklist",
                )
                .into_any_flex(),
            ])
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(Length::px(8.0))
            .into_any_flex(),
            new_btn.into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(12.0)),
        &style,
    ))
}

fn draft_meter(ctx: &ProjectionCtx<'_>, count: usize) -> UiView {
    let tone = if count > 0 {
        ChipTone::Accent
    } else {
        ChipTone::Neutral
    };
    chip_view(ctx, format!("{count} chars"), tone)
}

fn composer_context_bar(
    ctx: &ProjectionCtx<'_>,
    state: Option<&PicusState>,
    draft_count: usize,
    streaming: bool,
    selected: bool,
) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.composer.meta"]);
    let status = if streaming {
        chip_view(ctx, "streaming", ChipTone::Success)
    } else if selected {
        chip_view(ctx, "ready", ChipTone::Accent)
    } else {
        chip_view(ctx, "select a session", ChipTone::Neutral)
    };
    let (provider, model, approval, sandbox) = state
        .map(|s| {
            (
                config_summary_value(s, "provider", "provider unset"),
                config_summary_value(s, "model", "model unset"),
                config_summary_value(s, "approval_policy", "approval ask"),
                config_summary_value(s, "sandbox_mode", "sandbox default"),
            )
        })
        .unwrap_or_else(|| {
            (
                "provider pending".to_string(),
                "model pending".to_string(),
                "approval pending".to_string(),
                "sandbox pending".to_string(),
            )
        });

    Arc::new(apply_widget_style(
        flex_row(vec![
            status.into_any_flex(),
            chip_view(ctx, provider, ChipTone::Neutral).into_any_flex(),
            chip_view(ctx, model, ChipTone::Accent).into_any_flex(),
            chip_view(ctx, approval, ChipTone::Neutral).into_any_flex(),
            chip_view(ctx, sandbox, ChipTone::Neutral).into_any_flex(),
            sized_box(label("")).flex(1.0).into_any_flex(),
            draft_meter(ctx, draft_count).into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(6.0)),
        &style,
    ))
}

fn settings_header(ctx: &ProjectionCtx<'_>, state: Option<&PicusState>) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.settings.header"]);
    let subtitle = state
        .map(|s| {
            format!(
                "{} keys loaded · {}",
                s.config_values.len(),
                s.config_status.as_deref().unwrap_or("ready")
            )
        })
        .unwrap_or_else(|| "Config bridge starting".to_string());
    Arc::new(apply_widget_style(
        flex_col(vec![
            text_view(ctx, ["picuscode.settings.title"], "CodeWhale Settings").into_any_flex(),
            text_view(ctx, ["picuscode.settings.subtitle"], subtitle).into_any_flex(),
        ])
        .gap(Length::px(2.0)),
        &style,
    ))
}

fn settings_section(
    ctx: &ProjectionCtx<'_>,
    title: &'static str,
    values: &std::collections::BTreeMap<String, String>,
    edits: &std::collections::BTreeMap<String, String>,
    active_provider: &str,
    fields: &[(&'static str, &'static str, ConfigScope)],
) -> UiView {
    let style = resolve_style_for_classes(ctx.world, ["picuscode.settings.section"]);
    let input_style = resolve_style_for_classes(ctx.world, ["picuscode.text-input"]);
    let mut rows =
        vec![text_view(ctx, ["picuscode.settings.section.title"], title).into_any_flex()];
    for (key, display, scope) in fields {
        let current = resolve_config_field(values, active_provider, key, *scope);
        let target_key = config_field_target_key(active_provider, key, *scope);
        let display_value = display_config_field_value(key, &current, edits.get(&target_key));
        let placeholder = config_field_placeholder(key);
        let row = flex_row(vec![
            sized_box(text_view(ctx, ["picuscode.settings.label"], *display))
                .width(Length::px(132.0))
                .into_any_flex(),
            apply_direct_text_input_style(
                text_input(ctx.entity, display_value, move |v| {
                    PicusCodeAction::EditConfig(target_key.clone(), v)
                })
                .placeholder(placeholder),
                &input_style,
            )
            .flex(1.0)
            .into_any_flex(),
        ])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(8.0));
        rows.push(row.into_any_flex());
    }
    Arc::new(apply_widget_style(
        flex_col(rows)
            .cross_axis_alignment(CrossAxisAlignment::Stretch)
            .gap(Length::px(9.0)),
        &style,
    ))
}

#[derive(Clone, Copy)]
enum ConfigScope {
    Top,
    ProviderOrTop,
}

fn resolve_config_field(
    values: &std::collections::BTreeMap<String, String>,
    active_provider: &str,
    key: &str,
    scope: ConfigScope,
) -> String {
    match scope {
        ConfigScope::Top => values.get(key).cloned().unwrap_or_default(),
        ConfigScope::ProviderOrTop => {
            if !active_provider.is_empty() {
                let provider_key = format!("providers.{active_provider}.{key}");
                if let Some(value) = values.get(&provider_key) {
                    return value.clone();
                }
            }
            values.get(key).cloned().unwrap_or_default()
        }
    }
}

fn config_field_target_key(active_provider: &str, key: &str, scope: ConfigScope) -> String {
    match scope {
        ConfigScope::Top => key.to_string(),
        ConfigScope::ProviderOrTop if !active_provider.is_empty() => {
            format!("providers.{active_provider}.{key}")
        }
        ConfigScope::ProviderOrTop => key.to_string(),
    }
}

fn config_field_placeholder(key: &str) -> &'static str {
    if is_sensitive_config_key(key) {
        "Paste new key to update"
    } else {
        ""
    }
}

fn display_config_field_value(key: &str, value: &str, edit: Option<&String>) -> String {
    if let Some(edit) = edit {
        return edit.clone();
    }
    if is_sensitive_config_key(key) {
        String::new()
    } else {
        value.to_string()
    }
}

fn is_sensitive_config_key(key: &str) -> bool {
    key == "api_key" || key.ends_with(".api_key")
}

fn config_summary_value(state: &PicusState, key: &str, fallback: &str) -> String {
    state
        .config_values
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(truncate_config_value)
        .unwrap_or_else(|| fallback.to_string())
}

fn truncate_config_value(value: &str) -> String {
    if value.chars().count() <= 28 {
        value.to_string()
    } else {
        truncate_preview(value, 25)
    }
}

fn fallback_thread_title(seed: &str, id: &str) -> String {
    let seed = seed.trim();
    if !seed.is_empty() {
        return truncate_preview(seed, 34);
    }
    let short_id: String = id.chars().take(8).collect();
    format!("Thread {short_id}")
}

fn clean_provider(provider: &str) -> String {
    let provider = provider.trim();
    if provider.is_empty() {
        "unknown provider".to_string()
    } else {
        truncate_preview(provider, 22)
    }
}

fn format_timestamp(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "unknown time".to_string())
}

fn format_short_timestamp(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|dt| dt.format("%b %d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn draft_len(draft: &str) -> usize {
    draft.chars().count()
}

fn truncate_preview(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn preview_truncation_is_ascii_and_trimmed() {
        assert_eq!(truncate_preview("  hello world  ", 20), "hello world");
        assert_eq!(
            truncate_preview("abcdefghijklmnopqrstuvwxyz", 5),
            "abcde..."
        );
    }

    #[test]
    fn thread_title_falls_back_to_short_id() {
        assert_eq!(fallback_thread_title("", "abcdef123456"), "Thread abcdef12");
        assert_eq!(
            fallback_thread_title("A useful remembered prompt", "abcdef123456"),
            "A useful remembered prompt"
        );
    }

    #[test]
    fn timestamp_format_is_stable() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00 UTC");
    }

    #[test]
    fn settings_provider_scoped_field_reads_active_provider_value() {
        let mut values = BTreeMap::new();
        values.insert("provider".to_string(), "openrouter".to_string());
        values.insert("model".to_string(), "root-model".to_string());
        values.insert(
            "providers.openrouter.model".to_string(),
            "provider-model".to_string(),
        );

        assert_eq!(
            resolve_config_field(&values, "openrouter", "model", ConfigScope::ProviderOrTop),
            "provider-model"
        );
    }

    #[test]
    fn settings_provider_scoped_field_writes_active_provider_key() {
        assert_eq!(
            config_field_target_key("openrouter", "model", ConfigScope::ProviderOrTop),
            "providers.openrouter.model"
        );
        assert_eq!(
            config_field_target_key("", "model", ConfigScope::ProviderOrTop),
            "model"
        );
        assert_eq!(
            config_field_target_key("openrouter", "provider", ConfigScope::Top),
            "provider"
        );
    }

    #[test]
    fn settings_staged_provider_controls_provider_scoped_target_key() {
        let mut values = BTreeMap::new();
        values.insert("provider".to_string(), "deepseek".to_string());
        let mut edits = BTreeMap::new();
        edits.insert("provider".to_string(), "openrouter".to_string());

        let active_provider = edits
            .get("provider")
            .or_else(|| values.get("provider"))
            .cloned()
            .unwrap_or_default();

        assert_eq!(active_provider, "openrouter");
        assert_eq!(
            config_field_target_key(&active_provider, "model", ConfigScope::ProviderOrTop),
            "providers.openrouter.model"
        );
    }

    #[test]
    fn settings_api_key_field_does_not_display_redacted_secret_as_editable_value() {
        assert_eq!(
            display_config_field_value("api_key", "sk-d***cret", None),
            ""
        );
        assert_eq!(
            config_field_placeholder("api_key"),
            "Paste new key to update"
        );
        assert_eq!(
            display_config_field_value("api_key", "sk-d***cret", Some(&"sk-live".to_string())),
            "sk-live"
        );
        assert_eq!(
            display_config_field_value("model", "glm-5.2", None),
            "glm-5.2"
        );
    }
}

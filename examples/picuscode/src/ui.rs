//! Projection functions for picuscode view markers.

use std::sync::Arc;

use picus::{
    ProjectionCtx, UiView, apply_widget_style,
    button, emit_ui_action, text_input,
    masonry_core::layout::{Dim, Length},
    resolve_style,
    xilem::{
        InsertNewline,
        style::Style as _,
        view::{FlexExt as _, flex_col, flex_row, label, sized_box},
    },
};

use crate::action::PicusCodeAction;
use crate::state::*;

pub fn project_chat_root(_: &ChatRootView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();
    Arc::new(apply_widget_style(
        flex_col(children)
            .width(Dim::Stretch)
            .height(Dim::Stretch)
            .gap(Length::px(style.layout.gap)),
        &style,
    ))
}

pub fn project_title_bar(_: &ChatTitleBarView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let title = label("picuscode").text_size(16.0);
    let new_btn = button(ctx.entity, PicusCodeAction::NewThread, "+ New");
    let about_btn = button(ctx.entity, PicusCodeAction::OpenAbout, "About");
    let settings_btn = button(ctx.entity, PicusCodeAction::OpenSettings, "Settings");
    Arc::new(apply_widget_style(
        flex_row(vec![
            sized_box(title).flex(1.0).into_any_flex(),
            new_btn.into_any_flex(),
            settings_btn.into_any_flex(),
            about_btn.into_any_flex(),
        ])
        .gap(Length::px(8.0)),
        &style,
    ))
}

pub fn project_chat_body(_: &ChatBodyView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();
    Arc::new(apply_widget_style(
        flex_row(children).gap(Length::px(0.0)),
        &style,
    ))
}

pub fn project_sidebar_column(_: &SidebarColumnView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let state = ctx.world.get_resource::<PicusState>();
    let active_thread = state.and_then(|s| s.active_thread.clone());
    let threads = state.map(|s| s.threads.clone()).unwrap_or_default();

    let mut items: Vec<_> = Vec::with_capacity(threads.len() + 1);
    let header = label("Threads").text_size(13.0);
    items.push(sized_box(header).into_any_flex());

    if threads.is_empty() {
        items.push(
            label("(no threads yet — click + New)")
                .text_size(12.0)
                .into_any_flex(),
        );
    }
    for t in threads {
        let is_active = active_thread.as_deref() == Some(t.id.as_str());
        let prefix = if is_active { "▶ " } else { "  " };
        let name = t
            .name
            .clone()
            .unwrap_or_else(|| truncate_preview(&t.preview, 28));
        let label_text = format!("{prefix}{name}");
        let btn = button(
            ctx.entity,
            PicusCodeAction::SelectThread(t.id.clone()),
            label_text.as_str(),
        );
        items.push(btn.into_any_flex());
    }

    Arc::new(apply_widget_style(
        sized_box(flex_col(items).gap(Length::px(4.0)))
            .width(Length::px(220.0))
            .height(Dim::Stretch),
        &style,
    ))
}

pub fn project_transcript_column(_: &TranscriptColumnView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();
    Arc::new(apply_widget_style(
        flex_col(children).gap(Length::px(12.0)),
        &style,
    ))
}

pub fn project_composer(_: &ComposerView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let draft = ctx
        .world
        .get_resource::<PicusState>()
        .map(|s| s.draft.clone())
        .unwrap_or_default();
    let streaming = ctx
        .world
        .get_resource::<PicusState>()
        .is_some_and(|s| s.streaming);
    let input_entity = ctx.entity;
    let enter_entity = ctx.entity;
    let input = text_input(
        input_entity,
        draft,
        PicusCodeAction::ComposerChanged,
    )
    .placeholder("Message CodeWhale...")
    .insert_newline(InsertNewline::OnShiftEnter)
    .on_enter(move |_| {
        emit_ui_action(enter_entity, PicusCodeAction::Send);
    });
    let action_btn = if streaming {
        button(ctx.entity, PicusCodeAction::CancelTurn, "Cancel")
    } else {
        button(ctx.entity, PicusCodeAction::Send, "Send")
    };
    Arc::new(apply_widget_style(
        flex_row(vec![
            input.flex(1.0).into_any_flex(),
            action_btn.into_any_flex(),
        ])
        .gap(Length::px(8.0)),
        &style,
    ))
}

pub fn project_status_line(_: &StatusLineView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let text = ctx
        .world
        .get_resource::<PicusState>()
        .map(|s| s.status.clone())
        .unwrap_or_else(|| "Ready".to_string());
    Arc::new(apply_widget_style(label(text).text_size(12.0), &style))
}

pub fn project_about_root(_: &AboutRootView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let close_btn = button(ctx.entity, PicusCodeAction::CloseAbout, "Close");
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

pub fn project_settings_root(_: &SettingsRootView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let close_btn = button(ctx.entity, PicusCodeAction::CloseSettings, "Close");
    let reload_btn = button(ctx.entity, PicusCodeAction::ReloadConfig, "Reload from disk");
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();
    let mut all = children;
    all.push(reload_btn.into_any_flex());
    all.push(close_btn.into_any_flex());
    Arc::new(apply_widget_style(
        flex_col(all)
            .width(Dim::Stretch)
            .height(Dim::Stretch)
            .gap(Length::px(12.0)),
        &style,
    ))
}

pub fn project_settings_form(_: &SettingsFormView, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let state = ctx.world.get_resource::<PicusState>();

    // A small curated set of config keys to surface in the settings panel.
    // These map directly to codewhale config.toml fields and are persisted
    // through the bridge to the same file an installed codewhale reads.
    let curated = [
        ("provider", "Provider"),
        ("model", "Model"),
        ("api_key", "API Key"),
        ("base_url", "Base URL"),
        ("approval_policy", "Approval Policy"),
        ("sandbox_mode", "Sandbox Mode"),
        ("auth_mode", "Auth Mode"),
        ("telemetry", "Telemetry"),
    ];

    let mut rows: Vec<_> = Vec::new();
    let header = label("CodeWhale Settings").text_size(18.0);
    rows.push(header.into_any_flex());

    let values = state.map(|s| s.config_values.clone()).unwrap_or_default();
    for (key, display) in curated {
        let current = values.get(key).cloned().unwrap_or_default();
        let display_label: &str = display;
        let row = flex_row(vec![
            sized_box(label(display_label).text_size(13.0))
                .width(Length::px(140.0))
                .into_any_flex(),
            text_input(
                ctx.entity,
                current,
                move |v| PicusCodeAction::SetConfig(key.to_string(), v),
            )
            .flex(1.0)
            .into_any_flex(),
        ])
        .gap(Length::px(8.0));
        rows.push(row.into_any_flex());
    }

    if let Some(s) = state
        && let Some(status) = &s.config_status
    {
        rows.push(label(status.as_str()).text_size(12.0).into_any_flex());
    }

    let path_note = label("Settings persist to ~/.codewhale/config.toml — shared with your installed codewhale.")
        .text_size(11.0);
    rows.push(path_note.into_any_flex());

    Arc::new(apply_widget_style(
        flex_col(rows).gap(Length::px(10.0)),
        &style,
    ))
}

fn truncate_preview(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
//! UiComponentTemplate implementations for the gallery layout structure.
//!
//! In Fluent UI terms, these are the "app shell" components that define
//! the overall page layout — analogous to the Fluent UI `App` wrapper
//! and `ThemeProvider` with their style injection pattern.

use std::sync::Arc;

use picus_core::{
    ProjectionCtx, UiView, apply_label_style, apply_widget_style,
    bevy_ecs::prelude::*,
    masonry_core::layout::{Dim, Length},
    resolve_style, resolve_style_for_classes,
    xilem::{
        style::Style as _,
        view::{FlexExt as _, flex_col, label},
    },
};

use crate::state::GalleryState;

/// Root gallery component: renders a full-viewport flex column layout.
///
/// This corresponds to Fluent UI's `FluentProvider` or `ThemeProvider`
/// wrapping the entire application with consistent styling.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryRoot;

/// Status bar component: displays the most recent user interaction event.
///
/// Similar to Fluent UI's playground/example status indicators that
/// show the latest action performed on a control.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryStatus;

pub fn project_gallery_root(_: &GalleryRoot, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let children = ctx
        .children
        .into_iter()
        .map(|child| child.into_any_flex())
        .collect::<Vec<_>>();

    Arc::new(apply_widget_style(
        flex_col(children)
            .gap(Length::px(style.layout.gap))
            .width(Dim::Stretch)
            .height(Dim::Stretch),
        &style,
    ))
}

pub fn project_gallery_status(_: &GalleryStatus, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let text_style = resolve_style_for_classes(ctx.world, ["gallery.note"]);
    let state = ctx.world.resource::<GalleryState>();

    Arc::new(apply_widget_style(
        apply_label_style(label(state.last_event.clone()), &text_style),
        &style,
    ))
}

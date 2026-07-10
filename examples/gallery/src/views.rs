//! UiComponentTemplate implementations for the gallery layout structure.
//!
//! In Fluent UI terms, these are the "app shell" components that define
//! the overall page layout — analogous to the Fluent UI `FluentProvider`
//! wrapping the entire application with consistent styling.

use std::sync::Arc;

use picus::{
    ProjectionCtx, StyleClass, UiComponentTemplate, UiSearch, UiThemePicker, UiView,
    apply_widget_style,
    bevy_ecs::prelude::*,
    masonry_core::{
        layout::{Dim, Length},
        properties::Dimensions,
    },
    resolve_style,
    xilem::{
        style::Style as _,
        view::{FlexExt as _, FlexSpacer, flex_col, flex_item, flex_row, sized_box},
    },
};

/// Root gallery component: renders a full-viewport flex column layout.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryRoot;

/// Fixed top bar shell: brand at start, search near the end, tools at the edge.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryTopBar;

fn child_entity_views(ctx: &ProjectionCtx<'_>) -> Vec<(Entity, UiView)> {
    let child_entities = ctx
        .world
        .get::<Children>(ctx.entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();

    child_entities.into_iter().zip(ctx.children.clone()).collect()
}

fn has_style_class(world: &World, entity: Entity, class: &str) -> bool {
    world
        .get::<StyleClass>(entity)
        .is_some_and(|classes| classes.0.iter().any(|name| name == class))
}

impl UiComponentTemplate for GalleryRoot {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let child_entities = ctx
            .world
            .get::<Children>(ctx.entity)
            .map(|children| children.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let children = child_entities
            .into_iter()
            .zip(ctx.children)
            .map(|(entity, child)| {
                let flex_grow = resolve_style(ctx.world, entity).layout.flex_grow;
                if flex_grow > 0.0 {
                    flex_item(child, flex_grow).into()
                } else {
                    child.into_any_flex()
                }
            })
            .collect::<Vec<_>>();

        Arc::new(
            sized_box(apply_widget_style(
                flex_col(children).gap(Length::px(style.layout.gap)),
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

impl UiComponentTemplate for GalleryTopBar {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let style = resolve_style(ctx.world, ctx.entity);
        let pairs = child_entity_views(&ctx);

        let brand = pairs
            .iter()
            .find(|(entity, _)| has_style_class(ctx.world, *entity, "gallery.brand"))
            .map(|(_, view)| view.clone());
        let search = pairs
            .iter()
            .find(|(entity, _)| ctx.world.get::<UiSearch>(*entity).is_some())
            .map(|(_, view)| view.clone());
        let theme = pairs
            .iter()
            .find(|(entity, _)| ctx.world.get::<UiThemePicker>(*entity).is_some())
            .map(|(_, view)| view.clone());

        let mut children = Vec::new();
        if let Some(brand) = brand {
            children.push(
                sized_box(brand)
                    .width(Dim::Fixed(Length::px(240.0)))
                    .into_any_flex(),
            );
        }
        children.push(FlexSpacer::Flex(1.0).into_any_flex());
        if let Some(search) = search {
            children.push(
                sized_box(search)
                    .width(Dim::Fixed(Length::px(360.0)))
                    .into_any_flex(),
            );
        }
        if let Some(theme) = theme {
            children.push(theme.into_any_flex());
        }

        Arc::new(
            sized_box(apply_widget_style(
                flex_row(children).gap(Length::px(style.layout.gap)),
                &style,
            ))
            .dims(
                Dimensions::AUTO.with_width(Dim::Stretch),
            ),
        )
    }
}


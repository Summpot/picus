use std::sync::Arc;

use bevy_ecs::{entity::Entity, world::World};
use masonry::layout::{Dim, Length};
use xilem_masonry::style::Style as _;
use xilem_masonry::view::{
    CrossAxisAlignment, FlexExt as _, flex_col, label, sized_box, transformed,
};

use crate::{
    ecs::{OverlayComputedPosition, UiPopover},
    styling::{ResolvedStyle, apply_flex_alignment, apply_widget_style, resolve_style},
    views::opaque_hitbox_for_entity,
};

use super::{
    core::{ProjectionCtx, UiView},
    utils::hide_style_without_collapsing_layout,
};

pub(crate) fn popover_geometry(
    world: &World,
    entity: Entity,
    fallback_size: (f64, f64),
    styles: &mut [&mut ResolvedStyle],
) -> OverlayComputedPosition {
    let mut computed = world
        .get::<OverlayComputedPosition>(entity)
        .copied()
        .unwrap_or_default();

    if !computed.is_positioned {
        for style in styles.iter_mut() {
            hide_style_without_collapsing_layout(style);
        }
    }

    computed.width = if computed.width > 1.0 {
        computed.width
    } else {
        fallback_size.0.max(1.0)
    };
    computed.height = if computed.height > 1.0 {
        computed.height
    } else {
        fallback_size.1.max(1.0)
    };

    computed
}

pub(crate) fn project_popover(popover: &UiPopover, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let computed = popover_geometry(
        ctx.world,
        ctx.entity,
        popover.size_hint(),
        &mut [&mut style],
    );

    let children = if ctx.children.is_empty() {
        vec![label("").into_any_flex()]
    } else {
        ctx.children
            .into_iter()
            .map(|child| child.into_any_flex())
            .collect::<Vec<_>>()
    };

    let panel_content = apply_flex_alignment(
        flex_col(children).cross_axis_alignment(CrossAxisAlignment::Stretch),
        &style,
    )
    .gap(Length::px(style.layout.gap.max(0.0)));

    let panel = sized_box(panel_content).dims((
        Dim::Fixed(Length::px(computed.width)),
        Dim::Fixed(Length::px(computed.height)),
    ));

    Arc::new(
        transformed(opaque_hitbox_for_entity(
            ctx.entity,
            apply_widget_style(panel, &style),
        ))
        .translate((computed.x, computed.y)),
    )
}

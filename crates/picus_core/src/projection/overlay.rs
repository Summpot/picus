use super::core::{ProjectionCtx, UiView};
use crate::{
    ecs::{OverlayStack, OverlayState, UiOverlayRoot},
    resize::WindowSize,
    styling::{apply_widget_style, resolve_style_for_classes},
};
use crate::masonry_core::{
    layout::{Dim, UnitPoint},
    properties::Dimensions,
};
use picus_view::style::Style;
use picus_view::view::{label, sized_box, zstack};
use std::sync::Arc;

pub(crate) fn project_overlay_root(_: &UiOverlayRoot, ctx: ProjectionCtx<'_>) -> UiView {
    let has_modal_overlay = ctx
        .world
        .get_resource::<OverlayStack>()
        .is_some_and(|stack| {
            stack.active_overlays.iter().any(|overlay| {
                ctx.world
                    .get::<OverlayState>(*overlay)
                    .is_some_and(|state| state.is_modal)
            })
        });

    let mut layers = Vec::with_capacity(ctx.children.len() + usize::from(has_modal_overlay));

    if has_modal_overlay {
        let dimmer_style = resolve_style_for_classes(ctx.world, ["overlay.modal.dimmer"]);
        let dimmer_dims = if let Some(window_size) = ctx.world.get_resource::<WindowSize>()
            && window_size.width > 0.0
            && window_size.height > 0.0
        {
            Dimensions::AUTO
                .with_width(Dim::Fixed(crate::masonry_core::layout::Length::px(window_size.width)))
                .with_height(Dim::Fixed(crate::masonry_core::layout::Length::px(window_size.height)))
        } else {
            Dimensions::AUTO
                .with_width(Dim::Stretch)
                .with_height(Dim::Stretch)
        };

        let dimmer: UiView = Arc::new(apply_widget_style(
            sized_box(label("")).dims(dimmer_dims),
            &dimmer_style,
        ));
        layers.push(dimmer);
    }

    layers.extend(ctx.children);

    Arc::new(
        zstack(layers).alignment(UnitPoint::TOP_LEFT).dims(
            Dimensions::AUTO
                .with_width(Dim::Stretch)
                .with_height(Dim::Stretch),
        ),
    )
}

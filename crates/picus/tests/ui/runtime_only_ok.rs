use std::sync::Arc;

use picus::{
    ProjectionCtx, UiComponent, UiComponentTemplate, UiView, bevy_ecs::prelude::Component,
    xilem::view::label,
};

// runtime_only skips Default + Clone authoring assertions.
#[derive(Component, UiComponent)]
#[ui_component(runtime_only)]
struct RuntimeOnly;

impl UiComponentTemplate for RuntimeOnly {
    fn project(_: &Self, _: ProjectionCtx<'_>) -> UiView {
        Arc::new(label("ok"))
    }
}

fn main() {}

use std::sync::Arc;

use bevy_ecs::prelude::*;
use picus_view::view::{AnyFlexChild, FlexExt as _, FlexSpacer, flex_row, label};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Title bar icon type.
#[derive(Debug, Clone)]
pub enum TitleBarIcon {
    /// Use the system-default application icon.
    System,
    /// A custom icon rendered by the specified entity.
    Custom(Entity),
}

/// Title bar action event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarAction {
    Minimize,
    Maximize,
    Restore,
    Close,
    FullScreen,
}

/// Runtime state tracked on the title bar entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TitleBarState {
    pub is_maximized: bool,
    pub is_full_screen: bool,
}

/// Title bar UI component.
///
/// Renders a horizontal bar with an optional icon, a title label, and
/// window control buttons (minimize, maximize, close).
#[derive(Component, Debug, Clone)]
pub struct UiTitleBar {
    pub title: String,
    pub icon: Option<TitleBarIcon>,
    pub show_minimize: bool,
    pub show_maximize: bool,
    pub show_close: bool,
    pub is_drag_region: bool,
    pub height: f32,
}

impl Default for UiTitleBar {
    fn default() -> Self {
        Self {
            title: String::new(),
            icon: None,
            show_minimize: true,
            show_maximize: true,
            show_close: true,
            is_drag_region: true,
            height: 32.0,
        }
    }
}

impl UiComponentTemplate for UiTitleBar {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let mut children: Vec<AnyFlexChild<(), ()>> = Vec::new();

        // Optional icon placeholder
        if component.icon.is_some() {
            // Icon placeholder — currently renders an empty label.
            // In a full implementation this would render an image or glyph.
            children.push(label("").into_any_flex());
        }

        // Title text
        children.push(label(component.title.as_str()).into_any_flex());

        // Spacer to push control buttons to the right
        children.push(FlexSpacer::Flex(1.0).into_any_flex());

        // Window control buttons — TitleBarAction is consumed by the internal
        // titlebar system (not application UiAction messages), so use the
        // retained bridge helper rather than ProjectionCtx::button.
        if component.show_minimize {
            children.push(
                crate::retained_bridge::button(ctx.entity, TitleBarAction::Minimize, "─")
                    .into_any_flex(),
            );
        }
        if component.show_maximize {
            children.push(
                crate::retained_bridge::button(ctx.entity, TitleBarAction::Maximize, "□")
                    .into_any_flex(),
            );
        }
        if component.show_close {
            children.push(
                crate::retained_bridge::button(ctx.entity, TitleBarAction::Close, "✕")
                    .into_any_flex(),
            );
        }

        Arc::new(flex_row(children))
    }
}

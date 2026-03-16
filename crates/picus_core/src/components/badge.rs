use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// A small non-interactive badge / pill label.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiBadge {
    pub text: String,
    pub text_key: Option<String>,
}

impl UiBadge {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            text_key: None,
        }
    }

    #[must_use]
    pub fn with_text_key(mut self, key: impl Into<String>) -> Self {
        self.text_key = Some(key.into());
        self
    }
}

impl UiComponentTemplate for UiBadge {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_badge(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::UiBadge;

    #[test]
    fn badge_builder_keeps_text_and_key() {
        let badge = UiBadge::new("Beta").with_text_key("demo-badge");
        assert_eq!(badge.text, "Beta");
        assert_eq!(badge.text_key.as_deref(), Some("demo-badge"));
    }
}

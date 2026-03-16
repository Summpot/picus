use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// A determinate or indeterminate progress bar.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct UiProgressBar {
    pub progress: Option<f64>,
}

impl UiProgressBar {
    #[must_use]
    pub const fn new(progress: Option<f64>) -> Self {
        Self { progress }
    }

    #[must_use]
    pub const fn determinate(progress: f64) -> Self {
        Self {
            progress: Some(progress),
        }
    }

    #[must_use]
    pub const fn indeterminate() -> Self {
        Self { progress: None }
    }
}

impl UiComponentTemplate for UiProgressBar {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_progress_bar(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::UiProgressBar;

    #[test]
    fn progress_bar_supports_determinate_and_indeterminate_modes() {
        assert_eq!(UiProgressBar::determinate(0.5).progress, Some(0.5));
        assert_eq!(UiProgressBar::indeterminate().progress, None);
    }
}

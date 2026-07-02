use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Size variant for the rating control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RatingSize {
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
}

impl RatingSize {
    #[must_use]
    pub const fn star_font_size(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 18.0,
            Self::Large => 24.0,
            Self::ExtraLarge => 32.0,
        }
    }
}

/// Colour variant for rating stars.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RatingColor {
    #[default]
    Neutral,
    Brand,
    Marigold,
}

/// Emitted when a rating value is changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRatingChanged {
    pub rating: Entity,
    pub value: f64,
}

/// Rating control.
///
/// Renders interactive stars grouped in a horizontal row.
/// Each star emits a typed [`UiRatingChanged`] event when clicked.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiRating {
    /// Current value (0.0 … max).
    pub value: f64,
    /// Maximum rating (number of stars). Default: 5.
    pub max: u16,
    /// Step precision (1.0 for whole stars, 0.5 for half).
    pub step: f64,
    /// Star size.
    pub size: RatingSize,
    /// Colour scheme.
    pub color: RatingColor,
}

impl UiRating {
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value: value.clamp(0.0, 5.0),
            max: 5,
            step: 1.0,
            size: RatingSize::Medium,
            color: RatingColor::Neutral,
        }
    }

    #[must_use]
    pub fn with_max(mut self, max: u16) -> Self {
        self.max = max.max(1);
        self.value = self.value.clamp(0.0, f64::from(self.max));
        self
    }

    #[must_use]
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step.max(f64::EPSILON);
        self
    }

    #[must_use]
    pub fn with_size(mut self, size: RatingSize) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn with_color(mut self, color: RatingColor) -> Self {
        self.color = color;
        self
    }
}

impl UiComponentTemplate for UiRating {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_rating(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_defaults_to_5_stars() {
        let r = UiRating::new(3.0);
        assert_eq!(r.max, 5);
    }

    #[test]
    fn rating_clamps_value_to_max() {
        let r = UiRating::new(10.0).with_max(3);
        assert_eq!(r.value, 3.0);
    }

    #[test]
    fn rating_size_font_sizes_are_positive() {
        for size in &[RatingSize::Small, RatingSize::Medium, RatingSize::Large, RatingSize::ExtraLarge] {
            assert!(size.star_font_size() > 0.0);
        }
    }
}

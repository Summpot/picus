use bevy_ecs::{entity::Entity, prelude::*};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Built-in numeric up-down spinner control with ECS-native value.
///
/// Renders a horizontal row with decrement, value, and increment buttons that
/// step the value by `step` within `[min, max]`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiNumericUpDown {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    /// Increment applied by the +/- buttons.
    pub step: f64,
    /// Number of decimal places to render. `0` renders an integer.
    pub precision: u8,
    /// Optional text shown before the value (e.g. a currency symbol or unit).
    pub prefix: Option<&'static str>,
    /// Optional text shown after the value (e.g. "%", "px").
    pub suffix: Option<&'static str>,
    /// When true the +/- buttons do not change the value.
    pub disabled: bool,
}

impl UiNumericUpDown {
    #[must_use]
    pub fn new(min: f64, max: f64, value: f64) -> Self {
        let min = min.min(max);
        let max = max.max(min);
        let value = value.clamp(min, max);
        let span = (max - min).abs();
        let step = (span / 20.0).max(0.01);
        Self {
            value,
            min,
            max,
            step,
            precision: 0,
            prefix: None,
            suffix: None,
            disabled: false,
        }
    }

    #[must_use]
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step.abs().max(f64::EPSILON);
        self
    }

    #[must_use]
    pub fn with_precision(mut self, precision: u8) -> Self {
        self.precision = precision;
        self
    }

    #[must_use]
    pub fn with_prefix(mut self, prefix: &'static str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    #[must_use]
    pub fn with_suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Format the current value for display using the configured precision and prefix/suffix.
    #[must_use]
    pub fn formatted_value(&self) -> String {
        let number = if self.precision == 0 {
            format!("{:.0}", self.value.round())
        } else {
            format!("{:.*}", usize::from(self.precision), self.value)
        };
        match (self.prefix, self.suffix) {
            (Some(p), Some(s)) => format!("{p}{number}{s}"),
            (Some(p), None) => format!("{p}{number}"),
            (None, Some(s)) => format!("{number}{s}"),
            (None, None) => number,
        }
    }
}

impl Default for UiNumericUpDown {
    fn default() -> Self {
        Self::new(0.0, 100.0, 0.0)
    }
}

/// Emitted when [`UiNumericUpDown`] value changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiNumericUpDownChanged {
    pub numeric: Entity,
    pub value: f64,
}

impl UiComponentTemplate for UiNumericUpDown {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_numeric_up_down(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UiEventQueue;

    #[test]
    fn numeric_up_down_step_action_updates_value() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let numeric = world
            .spawn((crate::UiNumericUpDown::new(0.0, 100.0, 20.0).with_step(5.0),))
            .id();

        world.resource::<UiEventQueue>().push_typed(
            numeric,
            crate::WidgetUiAction::StepNumericUpDown {
                numeric,
                delta: 1.0,
            },
        );
        crate::handle_widget_actions(&mut world);

        let state = world
            .get::<crate::UiNumericUpDown>(numeric)
            .expect("numeric should exist");
        assert_eq!(state.value, 25.0);

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiNumericUpDownChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].action.value, 25.0);
    }

    #[test]
    fn numeric_up_down_clamps_to_range() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let numeric = world
            .spawn((crate::UiNumericUpDown::new(0.0, 10.0, 8.0).with_step(5.0),))
            .id();

        // Step beyond max should clamp.
        world.resource::<UiEventQueue>().push_typed(
            numeric,
            crate::WidgetUiAction::StepNumericUpDown {
                numeric,
                delta: 1.0,
            },
        );
        crate::handle_widget_actions(&mut world);

        let state = world
            .get::<crate::UiNumericUpDown>(numeric)
            .expect("numeric should exist");
        assert_eq!(state.value, 10.0, "value should clamp to max");
    }

    #[test]
    fn numeric_up_down_formats_value_with_precision_and_suffix() {
        let n = crate::UiNumericUpDown::new(0.0, 1.0, 0.30)
            .with_step(0.05)
            .with_precision(2)
            .with_suffix(" s");
        assert_eq!(n.formatted_value(), "0.30 s");

        let integer = crate::UiNumericUpDown::new(0.0, 100.0, 25.0).with_suffix(" px");
        assert_eq!(integer.formatted_value(), "25 px");

        let prefixed = crate::UiNumericUpDown::new(0.0, 1000.0, 42.0).with_prefix("$");
        assert_eq!(prefixed.formatted_value(), "$42");
    }
}

use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate, icons::IconGlyph};

/// Button appearance matching Fluent UI v9 Button component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonAppearance {
    /// Default button with subtle background and border.
    #[default]
    Default,
    /// Filled with brand/accent color.
    Primary,
    /// Transparent background with visible border.
    Outline,
    /// Nearly transparent, minimal style.
    Subtle,
    /// Fully transparent background, no border.
    Transparent,
}

/// Button size matching Fluent UI v9 size scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Medium,
    Small,
    Large,
}

/// Button shape variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonShape {
    /// Default rounded (borderRadiusMedium).
    #[default]
    Rounded,
    /// Fully circular/pill shape.
    Circular,
    /// Sharp square corners.
    Square,
}

/// Icon position relative to button label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonIconPosition {
    /// Icon before the label text.
    #[default]
    Before,
    /// Icon after the label text.
    After,
    /// Only icon, no label text.
    IconOnly,
}

/// Built-in button component.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiButton {
    pub label: String,
    pub appearance: ButtonAppearance,
    pub size: ButtonSize,
    pub shape: ButtonShape,
    pub icon: Option<IconGlyph>,
    pub icon_position: ButtonIconPosition,
    /// When true the button is non-interactive: it does not emit click actions
    /// and is rendered with the `button.disabled` style class.
    pub disabled: bool,
}

impl UiButton {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            appearance: ButtonAppearance::default(),
            size: ButtonSize::default(),
            shape: ButtonShape::default(),
            icon: None,
            icon_position: ButtonIconPosition::Before,
            disabled: false,
        }
    }

    #[must_use]
    pub fn with_appearance(mut self, appearance: ButtonAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    #[must_use]
    pub fn with_size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn with_shape(mut self, shape: ButtonShape) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<IconGlyph>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn with_icon_position(mut self, icon_position: ButtonIconPosition) -> Self {
        self.icon_position = icon_position;
        self
    }

    /// Mark this button as disabled (non-interactive).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl UiComponentTemplate for UiButton {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_button(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::*;
    use crate::{PicusPlugin, UiRoot};
    use bevy_app::App;
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_window::{PrimaryWindow, Window};

    #[test]
    fn ui_button_projects_to_action_button_with_child_widget() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let button = app
            .world_mut()
            .spawn((crate::UiButton::new("Action"), ChildOf(root)))
            .id();

        app.update();

        let debug = format!("entity={}", button.to_bits());
        let widget_id = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            find_widget_id_by_debug_text(root, &debug)
                .expect("UiButton should project an entity-tagged action button widget")
        };

        let short_type = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_widget(widget_id)
                .map(|widget| widget.short_type_name().to_string())
                .unwrap_or_default()
        };

        assert_eq!(short_type, "ActionButtonWithChildWidget");
    }

    #[test]
    fn ui_button_disabled_does_not_project_action_button_widget() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let button = app
            .world_mut()
            .spawn((
                crate::UiButton::new("Disabled").disabled(true),
                ChildOf(root),
            ))
            .id();

        app.update();

        // A disabled button should NOT project an ActionButtonWithChildWidget;
        // it renders as a plain styled container so it cannot emit click actions.
        let debug = format!("entity={}", button.to_bits());
        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        let root = runtime
            .primary()
            .expect("primary window runtime should exist")
            .render_root
            .get_layer_root(0);
        let widget_id = find_widget_id_by_debug_text(root, &debug);
        assert!(
            widget_id.is_none(),
            "disabled UiButton should not project an entity-tagged action button widget"
        );
    }

    #[test]
    fn ui_button_disabled_builder_sets_disabled_field() {
        let button = crate::UiButton::new("Label").disabled(true);
        assert!(
            button.disabled,
            "disabled(true) should set the disabled field"
        );
        let enabled = crate::UiButton::new("Label").disabled(false);
        assert!(!enabled.disabled);
        let default = crate::UiButton::new("Label");
        assert!(!default.disabled, "default UiButton should not be disabled");
    }
}

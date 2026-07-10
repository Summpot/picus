use crate::backdrop::{
    ThemeManagedWindowBackdrop, WindowBackdropColorScheme, WindowBackdropMaterial,
};
use crate::styling::{
    StyleSheet, resolve_theme_backdrop_color_scheme, resolve_theme_backdrop_material,
};
use crate::xilem::winit::{dpi::Size, error::EventLoopError};
use bevy_a11y::AccessibilityPlugin;
use bevy_app::App;
use bevy_input::InputPlugin;
use bevy_window::{PrimaryWindow, Window, WindowPlugin};
use bevy_winit::{UpdateMode, WinitPlugin, WinitSettings};
use std::time::Duration;

/// Compatibility window options applied to Bevy's primary window before `App::run()`.
#[derive(Clone, Debug, Default)]
pub struct BevyWindowOptions {
    resizable: Option<bool>,
    initial_inner_size: Option<Size>,
    min_inner_size: Option<Size>,
    backdrop_material: Option<WindowBackdropMaterial>,
    backdrop_color_scheme: Option<WindowBackdropColorScheme>,
    theme_managed_backdrop: bool,
}

impl BevyWindowOptions {
    /// Sets whether the window is resizable.
    #[must_use]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = Some(resizable);
        self
    }

    /// Sets the initial inner size.
    #[must_use]
    pub fn with_initial_inner_size<S: Into<Size>>(mut self, size: S) -> Self {
        self.initial_inner_size = Some(size.into());
        self
    }

    /// Sets the minimum inner size.
    #[must_use]
    pub fn with_min_inner_size<S: Into<Size>>(mut self, size: S) -> Self {
        self.min_inner_size = Some(size.into());
        self
    }

    /// Requests a native top-level window backdrop material.
    ///
    /// Backdrop materials are applied to the primary window by
    /// [`run_app_with_window_options`]. On Windows they map to DWM system
    /// backdrops such as Mica and Desktop Acrylic; unsupported platforms ignore
    /// the native request while keeping normal Picus rendering.
    #[must_use]
    pub fn with_backdrop_material(mut self, material: WindowBackdropMaterial) -> Self {
        self.backdrop_material = Some(material);
        self.theme_managed_backdrop = false;
        self
    }

    /// Requests an explicit light/dark appearance for the native backdrop.
    #[must_use]
    pub fn with_backdrop_color_scheme(
        mut self,
        color_scheme: WindowBackdropColorScheme,
    ) -> Self {
        self.backdrop_color_scheme = Some(color_scheme);
        self
    }

    /// Requests the Windows Mica system backdrop for the primary window.
    #[must_use]
    pub fn with_mica_backdrop(self) -> Self {
        self.with_backdrop_material(WindowBackdropMaterial::Mica)
    }

    /// Requests the Windows Desktop Acrylic system backdrop for the primary window.
    #[must_use]
    pub fn with_acrylic_backdrop(self) -> Self {
        self.with_backdrop_material(WindowBackdropMaterial::Acrylic)
    }
}

fn apply_theme_backdrop_option(app: &App, options: &mut BevyWindowOptions) {
    if options.backdrop_material.is_some() {
        return;
    }
    let Some(material) = app
        .world()
        .get_resource::<StyleSheet>()
        .and_then(resolve_theme_backdrop_material)
    else {
        return;
    };

    options.backdrop_material = Some(material);
    if options.backdrop_color_scheme.is_none() {
        options.backdrop_color_scheme = app
            .world()
            .get_resource::<StyleSheet>()
            .and_then(resolve_theme_backdrop_color_scheme);
    }
    options.theme_managed_backdrop = true;
}

fn size_to_logical(size: Size) -> (f32, f32) {
    match size {
        Size::Physical(physical) => (physical.width as f32, physical.height as f32),
        Size::Logical(logical) => (logical.width as f32, logical.height as f32),
    }
}

fn apply_window_options(window: &mut Window, title: &str, options: &BevyWindowOptions) {
    window.title = title.to_string();

    if let Some(resizable) = options.resizable {
        window.resizable = resizable;
    }

    if let Some(initial_inner_size) = options.initial_inner_size {
        let (width, height) = size_to_logical(initial_inner_size);
        window.resolution.set(width, height);
    }

    if let Some(min_inner_size) = options.min_inner_size {
        let (min_width, min_height) = size_to_logical(min_inner_size);
        window.resize_constraints.min_width = min_width.max(1.0);
        window.resize_constraints.min_height = min_height.max(1.0);
    }

    if let Some(material) = options.backdrop_material {
        material.configure_window(window);
    }
}

fn build_primary_window(title: &str, options: &BevyWindowOptions) -> Window {
    let mut window = Window::default();
    apply_window_options(&mut window, title, options);
    window
}

fn primary_window_exists(app: &mut App) -> bool {
    let mut query = app
        .world_mut()
        .query_filtered::<&Window, bevy_ecs::query::With<PrimaryWindow>>();
    query.iter(app.world_mut()).next().is_some()
}

fn latency_bounded_winit_settings() -> WinitSettings {
    WinitSettings {
        focused_mode: UpdateMode::reactive(Duration::from_secs_f64(1.0 / 120.0)),
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 30.0)),
    }
}

fn ensure_latency_bounded_winit_settings(app: &mut App) {
    if !app.world().contains_resource::<WinitSettings>() {
        app.insert_resource(latency_bounded_winit_settings());
    }
}

fn ensure_native_windowing_plugins(app: &mut App, primary_window: &Window) {
    let had_primary_window = primary_window_exists(app);

    // Bevy's native window lifecycle depends on the same core plugin stack used
    // by `bevy::DefaultPlugins` for windowed apps.
    if !app.is_plugin_added::<AccessibilityPlugin>() {
        app.add_plugins(AccessibilityPlugin);
    }

    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }

    if !app.is_plugin_added::<WindowPlugin>() {
        app.add_plugins(WindowPlugin {
            primary_window: if had_primary_window {
                None
            } else {
                Some(primary_window.clone())
            },
            ..Default::default()
        });
    }

    ensure_latency_bounded_winit_settings(app);

    if !app.is_plugin_added::<WinitPlugin>() {
        app.add_plugins(WinitPlugin::default());
    }
}

fn configure_primary_window(app: &mut App, title: &str, options: &BevyWindowOptions) {
    let primary_entity = {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(bevy_ecs::entity::Entity, &mut Window), bevy_ecs::query::With<PrimaryWindow>>();
        if let Some((entity, mut window)) = query.iter_mut(world).next() {
            apply_window_options(&mut window, title, options);
            Some(entity)
        } else {
            None
        }
    };

    if let Some(entity) = primary_entity {
        if let Some(material) = options.backdrop_material {
            let mut entity = app.world_mut().entity_mut(entity);
            entity.insert(material);
            if let Some(color_scheme) = options.backdrop_color_scheme {
                entity.insert(color_scheme);
            }
            if options.theme_managed_backdrop {
                entity.insert(ThemeManagedWindowBackdrop);
            }
        }
        return;
    }

    let window = build_primary_window(title, options);
    let mut entity_commands = app.world_mut().spawn((window, PrimaryWindow));
    if let Some(material) = options.backdrop_material {
        entity_commands.insert(material);
        if let Some(color_scheme) = options.backdrop_color_scheme {
            entity_commands.insert(color_scheme);
        }
        if options.theme_managed_backdrop {
            entity_commands.insert(ThemeManagedWindowBackdrop);
        }
    }
}

/// Run a Bevy app using Bevy's native runner and default `bevy_winit` event loop.
///
/// This no longer creates a separate Xilem runner/event loop.
pub fn run_app(bevy_app: App, window_title: impl Into<String>) -> Result<(), EventLoopError> {
    run_app_with_window_options(bevy_app, window_title, |options| options)
}

/// Same as [`run_app`] with primary-window option overrides.
///
/// The closure receives and returns [`BevyWindowOptions`], preserving ergonomic
/// call sites while delegating execution to Bevy's own runner.
pub fn run_app_with_window_options(
    mut bevy_app: App,
    window_title: impl Into<String>,
    configure_window: impl Fn(BevyWindowOptions) -> BevyWindowOptions + Send + Sync + 'static,
) -> Result<(), EventLoopError> {
    let title = window_title.into();
    let mut options = configure_window(BevyWindowOptions::default());
    apply_theme_backdrop_option(&bevy_app, &mut options);
    let primary_window = build_primary_window(&title, &options);
    ensure_native_windowing_plugins(&mut bevy_app, &primary_window);
    configure_primary_window(&mut bevy_app, &title, &options);

    let _ = bevy_app.run();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StyleValue, ThemeBackdrop};
    use crate::xilem::winit::dpi::{LogicalSize, PhysicalSize};
    use std::collections::HashMap;

    #[test]
    fn options_apply_initial_and_min_sizes() {
        let mut window = Window::default();
        let options = BevyWindowOptions::default()
            .with_initial_inner_size(LogicalSize::new(640.0, 480.0))
            .with_min_inner_size(PhysicalSize::new(320, 200))
            .with_resizable(false);

        apply_window_options(&mut window, "Test", &options);

        assert_eq!(window.title, "Test");
        assert_eq!(window.width(), 640.0);
        assert_eq!(window.height(), 480.0);
        assert_eq!(window.resize_constraints.min_width, 320.0);
        assert_eq!(window.resize_constraints.min_height, 200.0);
        assert!(!window.resizable);
    }

    #[test]
    fn options_apply_backdrop_window_flags() {
        let mut window = Window::default();
        let options = BevyWindowOptions::default().with_backdrop_material(WindowBackdropMaterial::Mica);

        apply_window_options(&mut window, "Test", &options);

        assert_eq!(window.title, "Test");
        assert_eq!(window.transparent, cfg!(windows));
        assert_eq!(
            window.composite_alpha_mode,
            if cfg!(windows) {
                bevy_window::CompositeAlphaMode::PostMultiplied
            } else {
                bevy_window::CompositeAlphaMode::Auto
            }
        );
    }

    #[test]
    fn configure_primary_window_inserts_backdrop_component() {
        let mut app = App::new();
        let options =
            BevyWindowOptions::default().with_backdrop_material(WindowBackdropMaterial::Acrylic);

        configure_primary_window(&mut app, "Test", &options);

        let world = app.world_mut();
        let mut query = world.query_filtered::<(
            &Window,
            &WindowBackdropMaterial,
        ), bevy_ecs::query::With<PrimaryWindow>>();
        let (window, material) = query
            .iter(world)
            .next()
            .expect("primary window should be configured");

        assert_eq!(*material, WindowBackdropMaterial::Acrylic);
        assert_eq!(window.transparent, cfg!(windows));
        assert_eq!(
            window.composite_alpha_mode,
            if cfg!(windows) {
                bevy_window::CompositeAlphaMode::PostMultiplied
            } else {
                bevy_window::CompositeAlphaMode::Auto
            }
        );
    }

    #[test]
    fn theme_backdrop_is_applied_before_primary_window_creation() {
        let mut app = App::new();
        app.insert_resource(StyleSheet {
            backdrop: Some(ThemeBackdrop {
                material: StyleValue::value(WindowBackdropMaterial::Mica),
                color_scheme: WindowBackdropColorScheme::Dark,
                styles: HashMap::new(),
            }),
            ..StyleSheet::default()
        });
        let mut options = BevyWindowOptions::default();

        apply_theme_backdrop_option(&app, &mut options);
        configure_primary_window(&mut app, "Theme backdrop", &options);

        let world = app.world_mut();
        let mut query = world.query_filtered::<(
            &Window,
            &WindowBackdropMaterial,
            &WindowBackdropColorScheme,
            &ThemeManagedWindowBackdrop,
        ), bevy_ecs::query::With<PrimaryWindow>>();
        let (window, material, color_scheme, _) = query
            .iter(world)
            .next()
            .expect("theme-managed primary window should exist");
        assert_eq!(*material, WindowBackdropMaterial::Mica);
        assert_eq!(*color_scheme, WindowBackdropColorScheme::Dark);
        assert_eq!(window.transparent, cfg!(windows));
    }

    #[test]
    fn native_windowing_defaults_to_bounded_reactive_updates() {
        let mut app = App::new();

        ensure_latency_bounded_winit_settings(&mut app);

        let settings = app.world().resource::<WinitSettings>();
        assert_eq!(
            settings.focused_mode,
            UpdateMode::reactive(Duration::from_secs_f64(1.0 / 120.0))
        );
        assert_eq!(
            settings.unfocused_mode,
            UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 30.0))
        );
    }

    #[test]
    fn native_windowing_respects_existing_winit_settings() {
        let mut app = App::new();
        app.insert_resource(WinitSettings::desktop_app());

        ensure_latency_bounded_winit_settings(&mut app);

        let settings = app.world().resource::<WinitSettings>();
        assert_eq!(
            settings.focused_mode,
            WinitSettings::desktop_app().focused_mode
        );
        assert_eq!(
            settings.unfocused_mode,
            WinitSettings::desktop_app().unfocused_mode
        );
    }
}

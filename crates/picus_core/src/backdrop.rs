//! Native top-level window backdrop materials.

use bevy_ecs::prelude::*;
use bevy_window::{CompositeAlphaMode, RawHandleWrapper, Window, WindowWrapper};
use picus_surface::{
    NativeWindowBackdropColorScheme, NativeWindowBackdropError, NativeWindowBackdropMaterial,
    set_native_window_backdrop_material_with_color_scheme,
};
use serde::Deserialize;

use crate::styling::{
    StyleSheet, resolve_theme_backdrop_color_scheme, resolve_theme_backdrop_material,
};

/// Native desktop backdrop material for a top-level [`Window`].
///
/// Attach this component to a Bevy window entity to request the corresponding
/// platform material. On Windows, Picus maps these values to DWM system
/// backdrops. Other platforms keep running and treat the request as unsupported.
///
/// On Windows, backdrop windows must be created as transparent so Picus' wgpu
/// surface can reveal the compositor material behind the app content. Use
/// [`Self::configure_window`] before the native window is created, or use
/// [`crate::BevyWindowOptions::with_backdrop_material`] for the primary window.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WindowBackdropMaterial {
    /// No native backdrop material.
    #[default]
    None,
    /// Let the operating system choose the backdrop.
    Auto,
    /// Windows Mica system backdrop for long-lived app windows.
    Mica,
    /// Windows Desktop Acrylic system backdrop for transient surfaces.
    Acrylic,
    /// Windows tabbed/Mica Alt system backdrop.
    MicaAlt,
}

/// Light/dark appearance requested for a native top-level window backdrop.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
pub enum WindowBackdropColorScheme {
    /// Preserve the operating system's current window appearance policy.
    #[default]
    System,
    /// Request light window chrome and backdrop composition.
    Light,
    /// Request dark window chrome and backdrop composition.
    Dark,
}

impl WindowBackdropColorScheme {
    const fn native(self) -> NativeWindowBackdropColorScheme {
        match self {
            Self::System => NativeWindowBackdropColorScheme::System,
            Self::Light => NativeWindowBackdropColorScheme::Light,
            Self::Dark => NativeWindowBackdropColorScheme::Dark,
        }
    }
}

impl WindowBackdropMaterial {
    /// Stable lowercase name used by theme files and public tooling.
    #[must_use]
    pub const fn theme_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Auto => "auto",
            Self::Mica => "mica",
            Self::Acrylic => "acrylic",
            Self::MicaAlt => "mica-alt",
        }
    }

    /// Parse a backdrop name used by a theme file.
    pub fn from_theme_name(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "auto" => Ok(Self::Auto),
            "mica" => Ok(Self::Mica),
            "acrylic" => Ok(Self::Acrylic),
            "mica-alt" | "mica_alt" | "micaalt" => Ok(Self::MicaAlt),
            _ => Err(format!(
                "unknown window backdrop `{name}`; expected none, auto, mica, acrylic, or mica-alt"
            )),
        }
    }

    /// Returns `true` when this material needs a transparent window surface.
    #[must_use]
    pub const fn requires_transparent_surface(self) -> bool {
        cfg!(windows) && !matches!(self, Self::None)
    }

    /// Apply the window-creation flags needed for this backdrop.
    ///
    /// Call this before Bevy/winit creates the native window. Changing these
    /// flags after creation is platform-limited, especially `transparent`.
    pub fn configure_window(self, window: &mut Window) {
        if self.requires_transparent_surface() {
            if !window.transparent {
                window.transparent = true;
            }
            if window.composite_alpha_mode != CompositeAlphaMode::PreMultiplied {
                window.composite_alpha_mode = CompositeAlphaMode::PreMultiplied;
            }
        }
    }

    fn needs_window_configuration(self, window: &Window) -> bool {
        self.requires_transparent_surface()
            && (!window.transparent
                || window.composite_alpha_mode != CompositeAlphaMode::PreMultiplied)
    }

    const fn native(self) -> NativeWindowBackdropMaterial {
        match self {
            Self::None => NativeWindowBackdropMaterial::None,
            Self::Auto => NativeWindowBackdropMaterial::Auto,
            Self::Mica => NativeWindowBackdropMaterial::Mica,
            Self::Acrylic => NativeWindowBackdropMaterial::Acrylic,
            Self::MicaAlt => NativeWindowBackdropMaterial::MicaAlt,
        }
    }
}

/// Applies native backdrop flags to a Bevy [`Window`] before creation.
pub fn configure_window_for_backdrop(window: &mut Window, material: WindowBackdropMaterial) {
    material.configure_window(window);
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedWindowBackdropMaterial {
    material: WindowBackdropMaterial,
    color_scheme: WindowBackdropColorScheme,
}

/// Marks windows whose backdrop component is owned by the active theme.
#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct ThemeManagedWindowBackdrop;

type ThemeBackdropWindows<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Window,
        Option<&'static WindowBackdropMaterial>,
        Option<&'static WindowBackdropColorScheme>,
        Option<&'static ThemeManagedWindowBackdrop>,
    ),
>;

type NativeBackdropWindows<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static WindowBackdropMaterial,
        Option<&'static WindowBackdropColorScheme>,
        &'static mut Window,
        Option<&'static AppliedWindowBackdropMaterial>,
    ),
>;

/// Synchronize the active theme backdrop to windows without an explicit
/// application-owned [`WindowBackdropMaterial`].
pub(crate) fn sync_theme_window_backdrops(
    mut commands: Commands,
    stylesheet: Res<StyleSheet>,
    mut windows: ThemeBackdropWindows<'_, '_>,
) {
    let Some(material) = resolve_theme_backdrop_material(&stylesheet) else {
        return;
    };
    let color_scheme = resolve_theme_backdrop_color_scheme(&stylesheet).unwrap_or_default();

    for (entity, mut window, current, current_color_scheme, managed) in &mut windows {
        if current.is_some() && managed.is_none() {
            continue;
        }
        if current.is_some_and(|current| *current == material)
            && current_color_scheme.is_some_and(|current| *current == color_scheme)
            && managed.is_some()
        {
            continue;
        }

        material.configure_window(&mut window);
        commands
            .entity(entity)
            .insert((material, color_scheme, ThemeManagedWindowBackdrop));
    }
}

/// Synchronize requested native backdrop materials to attached winit windows.
pub(crate) fn apply_window_backdrop_materials(
    mut commands: Commands,
    mut window_query: NativeBackdropWindows<'_, '_>,
) {
    for (entity, material, color_scheme, mut window, applied) in &mut window_query {
        let color_scheme = color_scheme.copied().unwrap_or_default();
        if material.needs_window_configuration(&window) {
            material.configure_window(&mut window);
        }

        if applied.is_some_and(|applied| {
            applied.material == *material && applied.color_scheme == color_scheme
        }) {
            continue;
        }

        let Some(result) = with_window_raw_handle(entity, |raw_handle| {
            set_native_window_backdrop_material_with_color_scheme(
                &raw_handle,
                material.native(),
                color_scheme.native(),
            )
        }) else {
            continue;
        };

        match result {
            Ok(()) => {
                commands
                    .entity(entity)
                    .insert(AppliedWindowBackdropMaterial {
                        material: *material,
                        color_scheme,
                    });
            }
            Err(error) => {
                log_backdrop_error(entity, *material, error);
                commands
                    .entity(entity)
                    .insert(AppliedWindowBackdropMaterial {
                        material: *material,
                        color_scheme,
                    });
            }
        }
    }
}

fn with_window_raw_handle<T>(
    entity: Entity,
    apply: impl FnOnce(RawHandleWrapper) -> T,
) -> Option<T> {
    bevy_winit::WINIT_WINDOWS.with(|winit_windows| {
        let winit_windows = winit_windows.borrow();
        let window: &WindowWrapper<crate::xilem::winit::window::Window> =
            winit_windows.get_window(entity)?;
        match RawHandleWrapper::new(window) {
            Ok(raw_handle) => Some(apply(raw_handle)),
            Err(error) => {
                tracing::error!(
                    "failed to create raw window handle for backdrop material on window {:?}: {error}",
                    entity
                );
                None
            }
        }
    })
}

fn log_backdrop_error(
    entity: Entity,
    material: WindowBackdropMaterial,
    error: NativeWindowBackdropError,
) {
    match error {
        NativeWindowBackdropError::UnsupportedPlatform => {
            tracing::debug!(
                "window backdrop material {:?} is unsupported on this platform for window {:?}",
                material,
                entity
            );
        }
        NativeWindowBackdropError::UnsupportedWindowHandle => {
            tracing::warn!(
                "window backdrop material {:?} requires a supported native window handle for window {:?}",
                material,
                entity
            );
        }
        NativeWindowBackdropError::WindowsHresult(hr) => {
            tracing::warn!(
                "failed to apply window backdrop material {:?} to window {:?}: HRESULT {hr:#010x}",
                material,
                entity
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StyleSheet, StyleValue, ThemeBackdrop};
    use bevy_app::{App, Update};
    use std::collections::HashMap;

    #[test]
    fn backdrop_material_configures_transparent_window() {
        let mut window = Window::default();

        WindowBackdropMaterial::Mica.configure_window(&mut window);

        assert_eq!(window.transparent, cfg!(windows));
        assert_eq!(
            window.composite_alpha_mode,
            if cfg!(windows) {
                CompositeAlphaMode::PreMultiplied
            } else {
                CompositeAlphaMode::Auto
            }
        );
    }

    #[test]
    fn no_backdrop_keeps_default_window_opacity() {
        let mut window = Window::default();

        WindowBackdropMaterial::None.configure_window(&mut window);

        assert!(!window.transparent);
        assert_eq!(window.composite_alpha_mode, CompositeAlphaMode::Auto);
    }

    #[test]
    fn theme_backdrop_manages_unconfigured_windows() {
        let mut app = App::new();
        app.insert_resource(StyleSheet {
            backdrop: Some(ThemeBackdrop {
                material: StyleValue::value(WindowBackdropMaterial::Mica),
                color_scheme: WindowBackdropColorScheme::Dark,
                styles: HashMap::new(),
            }),
            ..StyleSheet::default()
        })
        .add_systems(Update, sync_theme_window_backdrops);
        let window = app.world_mut().spawn(Window::default()).id();

        app.update();

        assert_eq!(
            app.world().get::<WindowBackdropMaterial>(window),
            Some(&WindowBackdropMaterial::Mica)
        );
        assert_eq!(
            app.world().get::<WindowBackdropColorScheme>(window),
            Some(&WindowBackdropColorScheme::Dark)
        );
        assert!(app.world().get::<ThemeManagedWindowBackdrop>(window).is_some());
    }

    #[test]
    fn explicit_window_backdrop_takes_precedence_over_theme() {
        let mut app = App::new();
        app.insert_resource(StyleSheet {
            backdrop: Some(ThemeBackdrop {
                material: StyleValue::value(WindowBackdropMaterial::Mica),
                color_scheme: WindowBackdropColorScheme::Dark,
                styles: HashMap::new(),
            }),
            ..StyleSheet::default()
        })
        .add_systems(Update, sync_theme_window_backdrops);
        let window = app
            .world_mut()
            .spawn((Window::default(), WindowBackdropMaterial::Acrylic))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<WindowBackdropMaterial>(window),
            Some(&WindowBackdropMaterial::Acrylic)
        );
        assert!(app.world().get::<ThemeManagedWindowBackdrop>(window).is_none());
    }
}

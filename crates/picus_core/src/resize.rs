//! Responsive breakpoint tracking for the Bevy window.
//!
//! Provides a `WindowSize` resource updated each frame from the primary window,
//! and an `AppBreakpoints` resource with configurable named breakpoints matching
//! Fluent UI v9 conventions:
//!
//! | Name | Default range  |
//! |------|----------------|
//! | `xs` | 0 – 479px     |
//! | `sm` | 480 – 639px   |
//! | `md` | 640 – 1023px  |
//! | `lg` | 1024 – 1439px |
//! | `xl` | 1440 – 1919px |
//! | `xxl`| 1920px+        |

use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window};

/// Current window inner size (logical pixels).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct WindowSize {
    /// Window inner width in logical pixels.
    pub width: f64,
    /// Window inner height in logical pixels.
    pub height: f64,
    /// DPI scale factor.
    pub scale_factor: f64,
}

/// Configurable breakpoint thresholds.
///
/// Each field is the **minimum** width (logical px) at which that breakpoint
/// activates. Defaults follow Fluent UI v9 conventions.
///
/// The active breakpoint is the largest named breakpoint whose threshold
/// is ≤ current window width.
#[derive(Resource, Debug, Clone)]
pub struct AppBreakpoints {
    /// Extra-small: 0px (always active baseline)
    pub xs: f64,
    /// Small: 480px
    pub sm: f64,
    /// Medium: 640px
    pub md: f64,
    /// Large: 1024px
    pub lg: f64,
    /// Extra-large: 1440px
    pub xl: f64,
    /// Extra-extra-large: 1920px
    pub xxl: f64,
}

impl Default for AppBreakpoints {
    fn default() -> Self {
        Self {
            xs: 0.0,
            sm: 480.0,
            md: 640.0,
            lg: 1024.0,
            xl: 1440.0,
            xxl: 1920.0,
        }
    }
}

impl AppBreakpoints {
    /// Return the name of the active breakpoint ("xs".."xxl") for a given width.
    pub fn name_for_width(&self, width: f64) -> &'static str {
        if width >= self.xxl {
            "xxl"
        } else if width >= self.xl {
            "xl"
        } else if width >= self.lg {
            "lg"
        } else if width >= self.md {
            "md"
        } else if width >= self.sm {
            "sm"
        } else {
            "xs"
        }
    }

    /// Return the numeric index of the active breakpoint (0 = xs .. 5 = xxl).
    pub fn index_for_width(&self, width: f64) -> usize {
        if width >= self.xxl {
            5
        } else if width >= self.xl {
            4
        } else if width >= self.lg {
            3
        } else if width >= self.md {
            2
        } else if width >= self.sm {
            1
        } else {
            0
        }
    }

    /// Returns `true` when the current window width is at or above the named breakpoint.
    ///
    /// Valid names: `"xs"`, `"sm"`, `"md"`, `"lg"`, `"xl"`, `"xxl"`.
    /// Returns `false` for invalid names.
    pub fn is_at_least(&self, width: f64, breakpoint: &str) -> bool {
        let threshold = match breakpoint {
            "xs" => self.xs,
            "sm" => self.sm,
            "md" => self.md,
            "lg" => self.lg,
            "xl" => self.xl,
            "xxl" => self.xxl,
            _ => return false,
        };
        width >= threshold
    }

    /// Returns `true` when the current window width is below the named breakpoint.
    pub fn is_below(&self, width: f64, breakpoint: &str) -> bool {
        let threshold = match breakpoint {
            "xs" => self.xs,
            "sm" => self.sm,
            "md" => self.md,
            "lg" => self.lg,
            "xl" => self.xl,
            "xxl" => self.xxl,
            _ => return false,
        };
        width < threshold
    }

    /// Return the threshold value (in logical px) for a named breakpoint.
    ///
    /// Returns `None` for unknown names.
    pub fn threshold(&self, breakpoint: &str) -> Option<f64> {
        match breakpoint {
            "xs" => Some(self.xs),
            "sm" => Some(self.sm),
            "md" => Some(self.md),
            "lg" => Some(self.lg),
            "xl" => Some(self.xl),
            "xxl" => Some(self.xxl),
            _ => None,
        }
    }
}

/// System: track window size each frame from Bevy's primary window.
pub fn track_window_size(
    mut window_size: ResMut<WindowSize>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
) {
    if let Ok(window) = primary_window.single() {
        window_size.width = window.width() as f64;
        window_size.height = window.height() as f64;
        window_size.scale_factor = window.scale_factor() as f64;
    }
}

/// System: log breakpoint transitions (tracing).
pub fn log_breakpoint_transitions(breakpoints: Res<AppBreakpoints>, window_size: Res<WindowSize>) {
    // Only log when breakpoints resource changes are detected by the system
    // (not a hot-loop — this runs every frame)
    if window_size.is_changed() {
        let name = breakpoints.name_for_width(window_size.width);
        tracing::trace!(width = %window_size.width, breakpoint = %name, "current window breakpoint");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_breakpoints() {
        let bp = AppBreakpoints::default();
        assert_eq!(bp.name_for_width(0.0), "xs");
        assert_eq!(bp.name_for_width(479.0), "xs");
        assert_eq!(bp.name_for_width(480.0), "sm");
        assert_eq!(bp.name_for_width(639.0), "sm");
        assert_eq!(bp.name_for_width(640.0), "md");
        assert_eq!(bp.name_for_width(1023.0), "md");
        assert_eq!(bp.name_for_width(1024.0), "lg");
        assert_eq!(bp.name_for_width(1439.0), "lg");
        assert_eq!(bp.name_for_width(1440.0), "xl");
        assert_eq!(bp.name_for_width(1919.0), "xl");
        assert_eq!(bp.name_for_width(1920.0), "xxl");
        assert_eq!(bp.name_for_width(9999.0), "xxl");
    }

    #[test]
    fn breakpoint_indices() {
        let bp = AppBreakpoints::default();
        assert_eq!(bp.index_for_width(0.0), 0);
        assert_eq!(bp.index_for_width(480.0), 1);
        assert_eq!(bp.index_for_width(640.0), 2);
        assert_eq!(bp.index_for_width(1024.0), 3);
        assert_eq!(bp.index_for_width(1440.0), 4);
        assert_eq!(bp.index_for_width(1920.0), 5);
    }

    #[test]
    fn is_at_least_and_below() {
        let bp = AppBreakpoints::default();
        // At 800px wide
        assert!(bp.is_at_least(800.0, "xs"));
        assert!(bp.is_at_least(800.0, "sm"));
        assert!(bp.is_at_least(800.0, "md"));
        assert!(!bp.is_at_least(800.0, "lg"));
        assert!(!bp.is_at_least(800.0, "xl"));
        assert!(!bp.is_at_least(800.0, "xxl"));

        assert!(!bp.is_below(800.0, "xs"));
        assert!(!bp.is_below(800.0, "sm"));
        assert!(!bp.is_below(800.0, "md"));
        assert!(bp.is_below(800.0, "lg"));
        assert!(bp.is_below(800.0, "xl"));
        assert!(bp.is_below(800.0, "xxl"));
    }

    #[test]
    fn custom_breakpoints() {
        let bp = AppBreakpoints {
            sm: 600.0,
            md: 900.0,
            ..Default::default()
        };
        assert_eq!(bp.name_for_width(500.0), "xs");
        assert_eq!(bp.name_for_width(600.0), "sm");
        assert_eq!(bp.name_for_width(800.0), "sm");
        assert_eq!(bp.name_for_width(900.0), "md");
    }

    #[test]
    fn threshold_lookup() {
        let bp = AppBreakpoints::default();
        assert_eq!(bp.threshold("xs"), Some(0.0));
        assert_eq!(bp.threshold("xxl"), Some(1920.0));
        assert_eq!(bp.threshold("unknown"), None);
    }
}

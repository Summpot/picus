use bevy_ecs::{entity::Entity, prelude::Component, prelude::Resource};
use bevy_time::{Timer, TimerMode};

/// Marker component for UI tree roots.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UiRoot;

/// Marker component for the global overlay/portal root.
///
/// Overlay entities (dialogs, dropdowns, tooltips, etc.) should be attached as
/// descendants of this node so they are not clipped by regular layout parents.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UiOverlayRoot;

/// Built-in vertical container marker.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiFlexColumn;

/// Built-in horizontal container marker.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiFlexRow;

/// Built-in text label component.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiLabel {
    pub text: String,
}

impl UiLabel {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Typography preset matching Fluent v9 type ramp.
///
/// Attach this component (or the corresponding `StyleClass`) to an entity
/// to apply a complete set of font-size, font-weight, and line-height values.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TypographyPreset {
    #[default]
    Body1,
    Body2,
    Caption1,
    Caption1Strong,
    Caption2,
    Subtitle1,
    Subtitle2,
    Title1,
    Title2,
    Title3,
    LargeTitle,
    Display,
}

impl TypographyPreset {
    /// Return the `StyleClass` class name for this preset.
    #[must_use]
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Body1 => "type.body1",
            Self::Body2 => "type.body2",
            Self::Caption1 => "type.caption1",
            Self::Caption1Strong => "type.caption1-strong",
            Self::Caption2 => "type.caption2",
            Self::Subtitle1 => "type.subtitle1",
            Self::Subtitle2 => "type.subtitle2",
            Self::Title1 => "type.title1",
            Self::Title2 => "type.title2",
            Self::Title3 => "type.title3",
            Self::LargeTitle => "type.large-title",
            Self::Display => "type.display",
        }
    }
}

/// Translation key marker for localized text projection.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct LocalizeText {
    pub key: String,
}

impl LocalizeText {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

/// Universal placement hints for floating overlays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverlayPlacement {
    /// Centered inside the viewport.
    #[default]
    Center,
    /// Anchored above the anchor/window edge.
    Top,
    /// Anchored below the anchor/window edge.
    Bottom,
    /// Anchored to the left of the anchor/window edge.
    Left,
    /// Anchored to the right of the anchor/window edge.
    Right,
    /// Anchored to top edge, aligned to logical start.
    TopStart,
    /// Anchored to top edge, aligned to logical end.
    TopEnd,
    /// Anchored to bottom edge, aligned to logical start.
    BottomStart,
    /// Anchored to bottom edge, aligned to logical end.
    BottomEnd,
    /// Anchored to left edge, aligned to logical start.
    LeftStart,
    /// Anchored to right edge, aligned to logical start.
    RightStart,
}

/// Placement and collision behavior for an overlay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayConfig {
    /// Preferred placement for this overlay.
    pub placement: OverlayPlacement,
    /// Anchor entity for placement. `None` anchors to the window.
    pub anchor: Option<Entity>,
    /// Enables automatic placement flipping when the preferred side overflows.
    pub auto_flip: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            placement: OverlayPlacement::Center,
            anchor: None,
            auto_flip: false,
        }
    }
}

/// Runtime-computed window-space placement for an overlay surface.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlayComputedPosition {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub placement: OverlayPlacement,
    /// Becomes `true` once layout/placement sync has written a valid final position.
    pub is_positioned: bool,
}

/// Centralized z-ordered overlay stack.
///
/// The last entry is the top-most overlay (highest z-index).
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct OverlayStack {
    pub active_overlays: Vec<Entity>,
}

/// Behavioral state for an overlay instance.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayState {
    /// `true` for modal layers (dialogs/sheets) that block interactions under them.
    pub is_modal: bool,
    /// Optional trigger/anchor entity that opened this overlay.
    pub anchor: Option<Entity>,
}

/// Generic timer-driven lifecycle component.
///
/// Entities carrying this component are despawned when [`Self::timer`] finishes.
#[derive(Component, Debug, Clone)]
pub struct AutoDismiss {
    pub timer: Timer,
}

impl AutoDismiss {
    #[must_use]
    pub fn from_seconds(seconds: f32) -> Self {
        Self {
            timer: Timer::from_seconds(seconds.max(0.0), TimerMode::Once),
        }
    }
}

impl Default for AutoDismiss {
    fn default() -> Self {
        Self::from_seconds(0.0)
    }
}

/// Marker telling an overlay widget which anchor entity it follows.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchoredTo(pub Entity);

/// Cached window-space rectangle for anchored overlays.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlayAnchorRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// UI component that switches from flex-row to flex-column when the viewport
/// width drops below the named breakpoint (e.g. "md").
///
/// - At or above the breakpoint → renders as a horizontal flex row
/// - Below the breakpoint → renders as a vertical flex column
#[derive(Component, Debug, Clone)]
pub struct UiResponsiveRow {
    /// Breakpoint name: "xs", "sm", "md", "lg", "xl", or "xxl".
    /// Below this breakpoint the layout collapses into a column.
    pub collapse_at: String,
}

impl UiResponsiveRow {
    /// Create a responsive row that collapses to column below `collapse_at`.
    #[must_use]
    pub fn new(collapse_at: impl Into<String>) -> Self {
        Self {
            collapse_at: collapse_at.into(),
        }
    }
}

impl Default for UiResponsiveRow {
    fn default() -> Self {
        Self {
            collapse_at: "md".to_string(),
        }
    }
}

/// Conditionally renders children only when the viewport is within the
/// specified breakpoint range.
///
/// - `show_from`: show when width ≥ this breakpoint (None = always)
/// - `show_until`: show when width < this breakpoint (None = always)
///
/// When the viewport is outside the range, the entity projects to an empty widget
/// (zero-size, transparent, non-interactive).
#[derive(Component, Debug, Clone, Default)]
pub struct UiVisibleResponsive {
    /// Show when viewport ≥ this breakpoint. `None` means no lower bound.
    pub show_from: Option<String>,
    /// Show when viewport width < this breakpoint. `None` means no upper bound.
    pub show_until: Option<String>,
}

impl UiVisibleResponsive {
    /// Show only at or above the given breakpoint.
    #[must_use]
    pub fn show_from(breakpoint: impl Into<String>) -> Self {
        Self {
            show_from: Some(breakpoint.into()),
            show_until: None,
        }
    }

    /// Show only below the given breakpoint.
    #[must_use]
    pub fn show_until(breakpoint: impl Into<String>) -> Self {
        Self {
            show_from: None,
            show_until: Some(breakpoint.into()),
        }
    }

    /// Show only within the inclusive range [from, until).
    #[must_use]
    pub fn range(from: impl Into<String>, until: impl Into<String>) -> Self {
        Self {
            show_from: Some(from.into()),
            show_until: Some(until.into()),
        }
    }
}

/// Responsive grid that selects a column count based on the current viewport
/// width and a list of (breakpoint, column_count) rules.
///
/// Rules are evaluated in order; the first rule whose breakpoint is satisfied
/// (viewport width ≥ threshold) wins.
#[derive(Component, Debug, Clone)]
pub struct UiResponsiveGrid {
    /// Ordered column-break rules: `[(breakpoint_name, column_count), …]`.
    /// The first rule whose threshold is ≤ current width wins.
    pub column_rules: Vec<(String, u32)>,
    /// Default columns when no rules match (should be ≥ 1).
    pub default_columns: u32,
    /// Default rows (used when the responsive column count is active).
    pub rows: u32,
    /// Show grid lines for debugging.
    pub show_grid_lines: bool,
}

impl UiResponsiveGrid {
    /// Create a responsive grid with the given column rules.
    ///
    /// Rules should be ordered from smallest to largest breakpoint.
    /// The last rule should typically be the largest breakpoint.
    #[must_use]
    pub fn new(rules: Vec<(impl Into<String>, u32)>, default_columns: u32) -> Self {
        Self {
            column_rules: rules.into_iter().map(|(b, c)| (b.into(), c)).collect(),
            default_columns: default_columns.max(1),
            rows: default_columns.max(1),
            show_grid_lines: false,
        }
    }

    /// Set the number of rows.
    #[must_use]
    pub fn with_rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Show grid lines for debugging.
    #[must_use]
    pub fn with_grid_lines(mut self, show: bool) -> Self {
        self.show_grid_lines = show;
        self
    }
}

impl Default for UiResponsiveGrid {
    fn default() -> Self {
        Self {
            column_rules: vec![
                ("sm".to_string(), 1),
                ("md".to_string(), 2),
                ("lg".to_string(), 4),
            ],
            default_columns: 1,
            rows: 1,
            show_grid_lines: false,
        }
    }
}

pub use crate::components::*;

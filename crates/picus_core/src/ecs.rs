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

pub use crate::components::*;

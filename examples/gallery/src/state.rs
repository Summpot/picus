//! Gallery state resources and page enumeration.
//!
//! This module defines the `GalleryPage` enum (mapping to Fluent UI's component categories),
//! the `GalleryState` resource for tracking the last event, and the `GalleryRuntime` resource
//! that stores entity references for interactive controls across pages.
//!
//! In Fluent UI terms, this serves as the "app state" that coordinates between
//! the sidebar navigation, tab bar, and interactive component examples.

use bevy_ecs::prelude::*;

/// Enum listing all gallery pages, corresponding to Fluent UI component categories.
///
/// Each variant maps to a page that showcases a group of related Picus components.
/// Inspired by the Fluent UI v9 documentation navigation pattern where components
/// are grouped by functionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryPage {
    Buttons,
    Inputs,
    Selection,
    WindowMenu,
    MessageBox,
    Lists,
    GridView,
    Panels,
    Layout,
    Typography,
    Media,
    Shapes,
    Icons,
    Transitions,
    Overlay,
}

impl GalleryPage {
    /// All gallery pages in display order, matching the Fluent UI component documentation flow.
    pub const ALL: [Self; 15] = [
        Self::Buttons,
        Self::Inputs,
        Self::Selection,
        Self::WindowMenu,
        Self::MessageBox,
        Self::Lists,
        Self::GridView,
        Self::Panels,
        Self::Layout,
        Self::Typography,
        Self::Media,
        Self::Shapes,
        Self::Icons,
        Self::Transitions,
        Self::Overlay,
    ];

    /// Human-readable label for this page, used in navigation buttons and section titles.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Buttons => "Buttons",
            Self::Inputs => "Inputs",
            Self::Selection => "Selection",
            Self::WindowMenu => "Window/Menu",
            Self::MessageBox => "MessageBox",
            Self::Lists => "Lists",
            Self::GridView => "GridView",
            Self::Panels => "Panels",
            Self::Layout => "Layout",
            Self::Typography => "Typography",
            Self::Media => "Media",
            Self::Shapes => "Shapes",
            Self::Icons => "Icons",
            Self::Transitions => "Transitions",
            Self::Overlay => "Overlay",
        }
    }
}

/// Runtime state: tracks the last user interaction event for the status bar display.
#[derive(Resource, Debug, Clone)]
pub struct GalleryState {
    pub last_event: String,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            last_event: "Gallery ready. Interact with a control to see events here.".to_string(),
        }
    }
}

/// Runtime entity references for interactive controls across all pages.
///
/// These are stored so the event handler can dispatch actions to the correct widget,
/// following the Fluent UI pattern of separating component registration from event routing.
#[derive(Resource, Debug, Clone)]
pub struct GalleryRuntime {
    pub pages_tab_bar: Entity,
    pub nav_buttons: Vec<Entity>,
    pub open_dialog_btn: Entity,
    pub persistent_toast_btn: Entity,
    pub success_toast_btn: Entity,
    pub warning_toast_btn: Entity,
    pub error_toast_btn: Entity,
    pub prompt_dialog_btn: Entity,
    pub native_message_btn: Entity,
    pub popover_dialog_btn: Entity,
    pub burst_placeholder_btn: Entity,
}

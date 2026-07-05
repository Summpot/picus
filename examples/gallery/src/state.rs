//! Gallery state resources, page enumeration, and category groupings.
//!
//! This module defines the `GalleryPage` enum (mapping to Fluent UI's component categories),
//! the `GalleryState` resource for tracking the last event, and the `GalleryRuntime` resource
//! that stores entity references for interactive controls across pages.
//!
//! Inspired by the Fluent UI v9 documentation sidebar navigation pattern where components
//! are organized under category headings.

use bevy_ecs::prelude::*;
use picus::PicusIcon;

/// A sidebar category heading that groups related pages.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavCategory {
    pub label: &'static str,
    pub first_page_index: usize,
    pub page_count: usize,
}

/// Enum listing all gallery pages, corresponding to Fluent UI component categories.
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
    I18n,
}

impl GalleryPage {
    /// All gallery pages in display order.
    pub const ALL: [Self; 16] = [
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
        Self::I18n,
        Self::Media,
        Self::Shapes,
        Self::Icons,
        Self::Transitions,
        Self::Overlay,
    ];

    /// Sidebar category groups — matches the Fluent UI pattern of
    /// grouping related docs under a section heading.
    #[allow(dead_code)]
    pub const CATEGORIES: &'static [NavCategory] = &[
        NavCategory {
            label: "Input",
            first_page_index: 0,
            page_count: 3,
        },
        NavCategory {
            label: "Navigation & Lists",
            first_page_index: 3,
            page_count: 4,
        },
        NavCategory {
            label: "Layout & Panels",
            first_page_index: 7,
            page_count: 4,
        },
        NavCategory {
            label: "Display",
            first_page_index: 11,
            page_count: 4,
        },
        NavCategory {
            label: "Overlay & Motion",
            first_page_index: 15,
            page_count: 1,
        },
    ];

    /// Short description shown as page intro.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Buttons => {
                "Buttons, toggles, switches, checkboxes, sliders, and progress indicators for user actions and settings."
            }
            Self::Inputs => {
                "Text input, password, multiline text, combo box, slider, and tooltip controls for data entry."
            }
            Self::Selection => {
                "Checkbox groups, radio buttons, color pickers, date pickers, combo boxes, and list views for making selections."
            }
            Self::WindowMenu => "Menu bars with dropdown panels for command-driven navigation.",
            Self::MessageBox => {
                "Modal dialogs and message boxes for alerts, confirmations, and prompts."
            }
            Self::Lists => "List views, tree views, and data tables for structured content.",
            Self::GridView => "Data tables with sortable columns, selection, and template columns.",
            Self::Panels => {
                "Group boxes, split panes, tab bars, and popover containers for organizing content."
            }
            Self::Layout => {
                "Flex layouts, grid layouts, and canvas/absolute positioning for page structure."
            }
            Self::Typography => {
                "Text scale, CJK/Unicode support, and text wrapping in various sizes and weights."
            }
            Self::Media => "Images, canvas drawings, and media placeholders for visual content.",
            Self::Shapes => "Canvas-drawn primitives, color swatches, and shape samples.",
            Self::Icons => {
                "Icon glyphs from the bundled Lucide icon font, displayed in a gallery grid."
            }
            Self::Transitions => {
                "Theme transitions, spinners, progress bars, and motion indicators."
            }
            Self::Overlay => {
                "Dialogs, toasts, tooltips, combo overlays, and anchored popup surfaces."
            }
            Self::I18n => {
                "Internationalization, locale switching, and CJK font fallback with Fluent bundles."
            }
        }
    }

    /// Human-readable label for this page, used in navigation and titles.
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
            Self::I18n => "I18n",
        }
    }

    /// Icon glyph for this page (used in the sidebar nav).
    pub const fn icon(self) -> PicusIcon {
        match self {
            Self::Buttons => PicusIcon::Pointer,
            Self::Inputs => PicusIcon::TextCursorInput,
            Self::Selection => PicusIcon::CheckSquare,
            Self::WindowMenu => PicusIcon::Menu,
            Self::MessageBox => PicusIcon::MessageSquare,
            Self::Lists => PicusIcon::List,
            Self::GridView => PicusIcon::Table,
            Self::Panels => PicusIcon::LayoutPanelLeft,
            Self::Layout => PicusIcon::LayoutGrid,
            Self::Typography => PicusIcon::Type,
            Self::Media => PicusIcon::Image,
            Self::Shapes => PicusIcon::Square,
            Self::Icons => PicusIcon::Images,
            Self::Transitions => PicusIcon::Sparkles,
            Self::Overlay => PicusIcon::Layers,
            Self::I18n => PicusIcon::Globe,
        }
    }
}

/// Runtime state: tracks the last user interaction event for the status bar display.
#[derive(Resource, Debug, Clone)]
pub struct GalleryState {
    pub last_event: String,
    pub active_page: usize,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            last_event: "Gallery ready. Interact with a control to see events here.".to_string(),
            active_page: 0,
        }
    }
}

/// Runtime entity references for interactive controls across all pages.
#[derive(Resource, Debug, Clone)]
pub struct GalleryRuntime {
    pub nav_view: Entity,
    #[allow(dead_code)]
    pub search_input: Entity,
    pub open_dialog_btn: Entity,
    pub persistent_toast_btn: Entity,
    pub success_toast_btn: Entity,
    pub warning_toast_btn: Entity,
    pub error_toast_btn: Entity,
    pub prompt_dialog_btn: Entity,
    pub native_message_btn: Entity,
    pub popover_dialog_btn: Entity,
    pub burst_placeholder_btn: Entity,
    pub locale_combo: Entity,
}

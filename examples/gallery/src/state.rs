//! Gallery state resources, page enumeration, and category groupings.
//!
//! Mirrors the WinUI Gallery model: the sidebar lists individual controls
//! (one navigation item per component), grouped under category headings for
//! documentation/navigation metadata. Each [`GalleryPage`] maps to a single
//! control showcase page.
//!
//! Pages that depend on XAML authoring, media capture/playback, maps, or
//! OS-shell APIs are intentionally omitted; see gallery docs in comments on
//! [`GalleryPage::ALL`].

use bevy_ecs::prelude::*;
use picus::prelude::{FluentIcon, ToastKind};

/// Marks the Window Backdrop page's theme-backed native material picker.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryBackdropPicker;

/// Marks the I18n page locale combo so events can switch the active bundle.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct GalleryLocaleCombo;

/// A sidebar category heading that groups related control pages.
///
/// Mapped to an expandable WinUI-style `NavigationViewItem` parent with nested
/// leaf MenuItems for each page in the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavCategory {
    pub label: &'static str,
    pub first_page_index: usize,
    pub page_count: usize,
}

/// One gallery page = one Picus component (WinUI Gallery style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryPage {
    // Basic Input
    Button,
    HyperlinkButton,
    ToggleSwitch,
    CheckBox,
    RadioButton,
    Slider,
    ComboBox,
    ColorPicker,
    RatingControl,
    DatePicker,
    TimePicker,
    NumberBox,
    // Text
    TextBox,
    PasswordBox,
    MultiLineTextBox,
    SearchBox,
    TextBlock,
    // Collections
    ListView,
    TreeView,
    Table,
    DataTable,
    // Menus & window
    MenuBar,
    MenuFlyout,
    Toolbar,
    TitleBar,
    WindowBackdrop,
    // Status & info
    ProgressBar,
    Spinner,
    ToolTip,
    InfoBadge,
    InfoBar,
    Avatar,
    // Dialogs & flyouts
    Dialog,
    Toast,
    ContextMenu,
    Popover,
    // Layout
    StackPanel,
    Grid,
    Responsive,
    GroupBox,
    SplitPane,
    TabBar,
    Canvas,
    Expander,
    Divider,
    ScrollView,
    FormRow,
    Card,
    // Navigation
    BreadcrumbBar,
    NavigationView,
    // Media & design
    Image,
    Icons,
    Shapes,
    Brushes,
    Typography,
    Markdown,
    Theme,
    I18n,
}

impl GalleryPage {
    /// All gallery pages in display order (matches WinUI-style control list).
    ///
    /// Intentionally omitted vs WinUI Gallery (not a good Picus fit today):
    /// XAML Fundamentals (Resources/Style/Binding/Templates/ScratchPad),
    /// Media (WebView2/MediaPlayer/Map/Camera/Sound/AnimatedVisual*),
    /// Motion interop, Shell JumpList/Badge notifications, AppWindow multi-window
    /// demos, Accessibility docs pages, platform-only CommandBarFlyout/Swipe, etc.
    pub const ALL: [Self; 58] = [
        // Basic Input
        Self::Button,
        Self::HyperlinkButton,
        Self::ToggleSwitch,
        Self::CheckBox,
        Self::RadioButton,
        Self::Slider,
        Self::ComboBox,
        Self::ColorPicker,
        Self::RatingControl,
        Self::DatePicker,
        Self::TimePicker,
        Self::NumberBox,
        // Text
        Self::TextBox,
        Self::PasswordBox,
        Self::MultiLineTextBox,
        Self::SearchBox,
        Self::TextBlock,
        // Collections
        Self::ListView,
        Self::TreeView,
        Self::Table,
        Self::DataTable,
        // Menus & window
        Self::MenuBar,
        Self::MenuFlyout,
        Self::Toolbar,
        Self::TitleBar,
        Self::WindowBackdrop,
        // Status & info
        Self::ProgressBar,
        Self::Spinner,
        Self::ToolTip,
        Self::InfoBadge,
        Self::InfoBar,
        Self::Avatar,
        // Dialogs & flyouts
        Self::Dialog,
        Self::Toast,
        Self::ContextMenu,
        Self::Popover,
        // Layout
        Self::StackPanel,
        Self::Grid,
        Self::Responsive,
        Self::GroupBox,
        Self::SplitPane,
        Self::TabBar,
        Self::Canvas,
        Self::Expander,
        Self::Divider,
        Self::ScrollView,
        Self::FormRow,
        Self::Card,
        // Navigation
        Self::BreadcrumbBar,
        Self::NavigationView,
        // Media & design
        Self::Image,
        Self::Icons,
        Self::Shapes,
        Self::Brushes,
        Self::Typography,
        Self::Markdown,
        Self::Theme,
        Self::I18n,
    ];

    /// Sidebar category groups — WinUI Gallery-style expandable MenuItem parents.
    pub const CATEGORIES: &'static [NavCategory] = &[
        NavCategory {
            label: "Basic Input",
            first_page_index: 0,
            page_count: 12,
        },
        NavCategory {
            label: "Text",
            first_page_index: 12,
            page_count: 5,
        },
        NavCategory {
            label: "Collections",
            first_page_index: 17,
            page_count: 4,
        },
        NavCategory {
            label: "Menus & Window",
            first_page_index: 21,
            page_count: 5,
        },
        NavCategory {
            label: "Status & Info",
            first_page_index: 26,
            page_count: 6,
        },
        NavCategory {
            label: "Dialogs & Flyouts",
            first_page_index: 32,
            page_count: 4,
        },
        NavCategory {
            label: "Layout",
            first_page_index: 36,
            page_count: 12,
        },
        NavCategory {
            label: "Navigation",
            first_page_index: 48,
            page_count: 2,
        },
        NavCategory {
            label: "Media & Design",
            first_page_index: 50,
            page_count: 8,
        },
    ];

    /// Short description shown as page intro.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Button => {
                "A button initiates an action. Show default, accent, flat, danger, and disabled variants."
            }
            Self::HyperlinkButton => {
                "A hyperlink-style control navigates or invokes an action with link appearance."
            }
            Self::ToggleSwitch => {
                "A toggle switch represents a physical switch that allows users to turn things on or off."
            }
            Self::CheckBox => {
                "A checkbox allows the user to select a true (checked) or false (unchecked) option, including an indeterminate state."
            }
            Self::RadioButton => {
                "A radio button allows users to select one option from a group of mutually exclusive choices."
            }
            Self::Slider => {
                "A slider is a control that lets the user select from a range of values by moving a thumb control along a track."
            }
            Self::ComboBox => {
                "A combo box presents a list of items in a drop-down; the selected item is shown as the control value."
            }
            Self::ColorPicker => {
                "A color picker lets the user select a color value, including optional alpha."
            }
            Self::RatingControl => {
                "A rating control collects or displays a star rating with size and color variants."
            }
            Self::DatePicker => {
                "A date picker lets the user pick a calendar date through an anchored month grid."
            }
            Self::TimePicker => {
                "A time picker lets the user select hour, minute, and second in 12h or 24h mode."
            }
            Self::NumberBox => {
                "A number box (numeric up/down) lets the user enter or adjust a numeric value with step and precision."
            }
            Self::TextBox => "A text box is a single-line text input for free-form string entry.",
            Self::PasswordBox => {
                "A password box is a single-line text input that obscures the typed value."
            }
            Self::MultiLineTextBox => {
                "A multi-line text box accepts longer notes and wraps text across lines."
            }
            Self::SearchBox => {
                "A search box is a text field with search affordance for filtering or query entry."
            }
            Self::TextBlock => {
                "A text block displays static text with typography presets from the Fluent type ramp."
            }
            Self::ListView => {
                "A list view presents a scrollable collection of items with single or multi selection."
            }
            Self::TreeView => "A tree view displays hierarchical data with expandable nodes.",
            Self::Table => "A table presents structured rows and columns for compact tabular data.",
            Self::DataTable => {
                "A data table supports typed columns, selection, and image cell templates."
            }
            Self::MenuBar => "A menu bar hosts top-level menus that open dropdown command panels.",
            Self::MenuFlyout => {
                "A menu flyout is a left-click command list (WinUI MenuFlyout). Contrast with ContextMenu, which opens on right-click."
            }
            Self::Toolbar => {
                "A toolbar groups compact action buttons, toggles, and separators horizontally."
            }
            Self::TitleBar => {
                "A custom title bar draws window chrome with minimize, maximize, and close actions."
            }
            Self::WindowBackdrop => {
                "Window backdrop selects the native material (None, Mica, Acrylic) and theme-aware fills."
            }
            Self::ProgressBar => {
                "A progress bar shows determinate completion or indeterminate activity."
            }
            Self::Spinner => {
                "A spinner indicates ongoing work without a known completion percentage."
            }
            Self::ToolTip => {
                "A tooltip shows a short description when the pointer hovers a control."
            }
            Self::InfoBadge => {
                "An info badge is a small pill or count used to highlight status on related UI."
            }
            Self::InfoBar => {
                "An info bar (message bar) surfaces persistent informational, success, warning, or error banners."
            }
            Self::Avatar => {
                "An avatar shows a person or entity as initials (or optional image) with size and shape variants."
            }
            Self::Dialog => {
                "A content dialog (WinUI ContentDialog → UiDialog) is a modal overlay with title, body, and dismiss action."
            }
            Self::Toast => {
                "A toast is a transient or persistent notification on an overlay surface."
            }
            Self::ContextMenu => {
                "A context menu opens a command list when the user right-clicks a control (WinUI MenuFlyout vs ContextMenu: see MenuFlyout page for left-click)."
            }
            Self::Popover => {
                "A popover (WinUI Flyout ≈ UiPopover; Popup ≈ spawn_manual_overlay_at) places a floating panel at an anchor or an explicit pixel origin."
            }
            Self::StackPanel => {
                "A stack panel (flex row/column) lays out children along a single axis with gap."
            }
            Self::Grid => {
                "A grid places children into rows and columns with star, auto, and pixel tracks."
            }
            Self::Responsive => {
                "Responsive row, grid, and visibility helpers adapt layout at width breakpoints."
            }
            Self::GroupBox => {
                "A group box nests related controls under a labeled grouping surface."
            }
            Self::SplitPane => {
                "A split pane divides content into resizable primary and secondary regions."
            }
            Self::TabBar => "A tab bar switches between sibling content panels by selected tab.",
            Self::Canvas => {
                "A canvas draws vector commands and supports absolute child positioning."
            }
            Self::Expander => {
                "An expander collapses and expands a content region under a clickable header."
            }
            Self::Divider => {
                "A divider draws a horizontal or vertical rule to separate related content."
            }
            Self::ScrollView => {
                "A scroll view hosts content larger than its viewport with optional scrollbars."
            }
            Self::FormRow => {
                "A form row pairs a field caption with an input control in a horizontal layout."
            }
            Self::Card => {
                "A card is a elevated content surface for grouping related controls and text."
            }
            Self::BreadcrumbBar => {
                "A breadcrumb bar shows the navigation path to the current location."
            }
            Self::NavigationView => {
                "A navigation view provides a pane of destinations with hierarchical menu items, pane display modes, back chrome, info badges, and an optional Settings footer."
            }
            Self::Image => {
                "An image displays bitmap content, including empty fallback placeholders."
            }
            Self::Icons => {
                "Fluent Design icon glyphs using the Segoe Fluent Icons font fallback stack."
            }
            Self::Shapes => {
                "Canvas-drawn primitives such as rectangles, circles, lines, and paths."
            }
            Self::Brushes => {
                "Solid color swatches and linear/radial gradient brush samples on canvas."
            }
            Self::Typography => {
                "Text scale samples from hero through caption, plus wrapping behavior."
            }
            Self::Markdown => {
                "Markdown rendering for headings, lists, tables, emphasis, and fenced code."
            }
            Self::Theme => "Theme variant switching and interactive color transition samples.",
            Self::I18n => "Locale switching, Fluent bundle keys, and CJK font fallback samples.",
        }
    }

    /// Human-readable label for this page (sidebar + title).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Button => "Button",
            Self::HyperlinkButton => "HyperlinkButton",
            Self::ToggleSwitch => "ToggleSwitch",
            Self::CheckBox => "CheckBox",
            Self::RadioButton => "RadioButton",
            Self::Slider => "Slider",
            Self::ComboBox => "ComboBox",
            Self::ColorPicker => "ColorPicker",
            Self::RatingControl => "RatingControl",
            Self::DatePicker => "DatePicker",
            Self::TimePicker => "TimePicker",
            Self::NumberBox => "NumberBox",
            Self::TextBox => "TextBox",
            Self::PasswordBox => "PasswordBox",
            Self::MultiLineTextBox => "MultiLineTextBox",
            Self::SearchBox => "SearchBox",
            Self::TextBlock => "TextBlock",
            Self::ListView => "ListView",
            Self::TreeView => "TreeView",
            Self::Table => "Table",
            Self::DataTable => "DataTable",
            Self::MenuBar => "MenuBar",
            Self::MenuFlyout => "MenuFlyout",
            Self::Toolbar => "Toolbar",
            Self::TitleBar => "TitleBar",
            Self::WindowBackdrop => "WindowBackdrop",
            Self::ProgressBar => "ProgressBar",
            Self::Spinner => "Spinner",
            Self::ToolTip => "ToolTip",
            Self::InfoBadge => "InfoBadge",
            Self::InfoBar => "InfoBar",
            Self::Avatar => "Avatar",
            Self::Dialog => "Dialog",
            Self::Toast => "Toast",
            Self::ContextMenu => "ContextMenu",
            Self::Popover => "Popover",
            Self::StackPanel => "StackPanel",
            Self::Grid => "Grid",
            Self::Responsive => "Responsive",
            Self::GroupBox => "GroupBox",
            Self::SplitPane => "SplitPane",
            Self::TabBar => "TabBar",
            Self::Canvas => "Canvas",
            Self::Expander => "Expander",
            Self::Divider => "Divider",
            Self::ScrollView => "ScrollView",
            Self::FormRow => "FormRow",
            Self::Card => "Card",
            Self::BreadcrumbBar => "BreadcrumbBar",
            Self::NavigationView => "NavigationView",
            Self::Image => "Image",
            Self::Icons => "Icons",
            Self::Shapes => "Shapes",
            Self::Brushes => "Brushes",
            Self::Typography => "Typography",
            Self::Markdown => "Markdown",
            Self::Theme => "Theme",
            Self::I18n => "I18n",
        }
    }

    /// Icon glyph for this page (sidebar nav).
    pub const fn icon(self) -> FluentIcon {
        match self {
            Self::Button => FluentIcon::TouchPointer,
            Self::HyperlinkButton => FluentIcon::Globe,
            Self::ToggleSwitch => FluentIcon::Checkbox,
            Self::CheckBox => FluentIcon::Checkbox,
            Self::RadioButton => FluentIcon::Checkmark,
            Self::Slider => FluentIcon::Settings,
            Self::ComboBox => FluentIcon::ChevronDown,
            Self::ColorPicker => FluentIcon::Edit,
            Self::RatingControl => FluentIcon::Accept,
            Self::DatePicker => FluentIcon::Clock,
            Self::TimePicker => FluentIcon::Clock,
            Self::NumberBox => FluentIcon::Add,
            Self::TextBox => FluentIcon::Character,
            Self::PasswordBox => FluentIcon::Contact,
            Self::MultiLineTextBox => FluentIcon::Character,
            Self::SearchBox => FluentIcon::Search,
            Self::TextBlock => FluentIcon::Font,
            Self::ListView => FluentIcon::List,
            Self::TreeView => FluentIcon::Folder,
            Self::Table => FluentIcon::ViewAll,
            Self::DataTable => FluentIcon::ViewAll,
            Self::MenuBar => FluentIcon::GlobalNavigationButton,
            // List distinguishes MenuFlyout from ContextMenu (More) in the sidebar.
            Self::MenuFlyout => FluentIcon::List,
            Self::Toolbar => FluentIcon::AllApps,
            Self::TitleBar => FluentIcon::AllApps,
            Self::WindowBackdrop => FluentIcon::Brightness,
            Self::ProgressBar => FluentIcon::Sync,
            Self::Spinner => FluentIcon::Sync,
            Self::ToolTip => FluentIcon::Info,
            Self::InfoBadge => FluentIcon::Info,
            Self::InfoBar => FluentIcon::Message,
            Self::Avatar => FluentIcon::Contact,
            Self::Dialog => FluentIcon::Message,
            Self::Toast => FluentIcon::Info,
            Self::ContextMenu => FluentIcon::More,
            Self::Popover => FluentIcon::Map,
            Self::StackPanel => FluentIcon::DockLeft,
            Self::Grid => FluentIcon::ViewAll,
            Self::Responsive => FluentIcon::AllApps,
            Self::GroupBox => FluentIcon::Folder,
            Self::SplitPane => FluentIcon::DockLeft,
            Self::TabBar => FluentIcon::More,
            Self::Canvas => FluentIcon::Edit,
            Self::Expander => FluentIcon::ChevronDown,
            Self::Divider => FluentIcon::Remove,
            Self::ScrollView => FluentIcon::ViewAll,
            Self::FormRow => FluentIcon::Edit,
            Self::Card => FluentIcon::ViewAll,
            Self::BreadcrumbBar => FluentIcon::Forward,
            Self::NavigationView => FluentIcon::GlobalNavigationButton,
            Self::Image => FluentIcon::Pictures,
            Self::Icons => FluentIcon::AllApps,
            Self::Shapes => FluentIcon::Placeholder,
            Self::Brushes => FluentIcon::Edit,
            Self::Typography => FluentIcon::Font,
            Self::Markdown => FluentIcon::Character,
            Self::Theme => FluentIcon::Brightness,
            Self::I18n => FluentIcon::Globe,
        }
    }
}

/// Marker describing what a gallery demo button should do on click.
///
/// Attached to showcase buttons that only need to echo a toast, dialog, or
/// transient feedback. The gallery event dispatcher reads this component when
/// a `BuiltinUiAction::Clicked` is received.
#[derive(Component, Debug, Clone)]
pub enum GalleryButtonAction {
    /// Spawn a toast notification with the given message, kind, and duration
    /// in seconds. A duration of `0.0` produces a persistent toast.
    Toast {
        message: String,
        kind: ToastKind,
        duration: f32,
    },
    /// Spawn a modal dialog overlay with the given title, body, and dismiss label.
    Dialog {
        title: String,
        body: String,
        dismiss_label: String,
    },
    /// Show a transient informational toast with the given message.
    Info { message: String },
}

/// Runtime entity references and shell navigation state for the gallery.
#[derive(Resource, Debug, Clone)]
pub struct GalleryRuntime {
    /// Shell [`picus::UiNavigationView`] entity.
    pub nav_view: Entity,
    /// Top-bar [`picus::UiSearch`] that filters sidebar leaves.
    pub search_input: Entity,
    /// Content page root entities in [`GalleryPage::ALL`] order (stable).
    pub content_pages: Vec<Entity>,
    /// Maps the current nav leaf index → [`GalleryPage::ALL`] index.
    ///
    /// Identity when the search box is empty; a filtered subset while searching.
    pub leaf_to_page: Vec<usize>,
    /// Currently displayed page index into [`GalleryPage::ALL`].
    pub current_page: usize,
}

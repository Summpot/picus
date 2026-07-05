#![forbid(unsafe_code)]

/// Preferred family name exposed by the bundled Lucide font.
///
/// `lucide-icons` itself uses this family identifier in its own integration code.
pub const LUCIDE_FONT_FAMILY: &str = "lucide";

/// Raw TrueType bytes for Lucide glyph rendering.
pub const LUCIDE_FONT_BYTES: &[u8] = lucide_icons::LUCIDE_FONT_BYTES;

/// Narrow icon set currently used by `picus_core` built-in widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PicusIcon {
    Check,
    ChevronDown,
    ChevronUp,
    ChevronRight,
    Circle,
    CircleDot,
    X,
    SunMoon,
    Plus,
    Search,
    Edit,
    Delete,
    Settings,
    ArrowLeft,
    ArrowRight,
    Clock,
    /// Chat / conversation bubble (for thread list items).
    MessageSquare,
    /// Paper-plane send action (for composer send button).
    Send,
    /// Circular refresh (for reload buttons).
    RefreshCw,
    /// Pointer/click target glyph (for button examples).
    Pointer,
    /// Text cursor in an input field (for input examples).
    TextCursorInput,
    /// Checked square (for selection examples).
    CheckSquare,
    /// Menu bars and command menus.
    Menu,
    /// Information glyph (for About).
    Info,
    /// Filled stop circle (for cancel-turn button).
    StopCircle,
    /// Archive box (for archive-thread action).
    Archive,
    /// Three-dot overflow menu (Ellipsis).
    Ellipsis,
    /// Bot face (for assistant attribution).
    Bot,
    /// Person (for user attribution).
    User,
    /// Sparkles (for "new" / AI accents).
    Sparkles,
    /// Simple list glyph.
    List,
    /// Data table / grid glyph.
    Table,
    /// Panel layout glyph.
    LayoutPanelLeft,
    /// Grid layout glyph.
    LayoutGrid,
    /// Typography glyph.
    Type,
    /// Image/media glyph.
    Image,
    /// Stacked images glyph.
    Images,
    /// Basic square/shape glyph.
    Square,
    /// Layer stack glyph.
    Layers,
    /// Globe glyph.
    Globe,
    /// Spinning loader (idle/thinking indicator).
    Loader,
    /// Folder (for workspace / cwd display).
    Folder,
}

impl PicusIcon {
    #[must_use]
    pub const fn as_lucide(self) -> lucide_icons::Icon {
        match self {
            Self::Check => lucide_icons::Icon::Check,
            Self::ChevronDown => lucide_icons::Icon::ChevronDown,
            Self::ChevronUp => lucide_icons::Icon::ChevronUp,
            Self::ChevronRight => lucide_icons::Icon::ChevronRight,
            Self::Circle => lucide_icons::Icon::Circle,
            Self::CircleDot => lucide_icons::Icon::CircleDot,
            Self::X => lucide_icons::Icon::X,
            Self::SunMoon => lucide_icons::Icon::SunMoon,
            Self::Plus => lucide_icons::Icon::Plus,
            Self::Search => lucide_icons::Icon::Search,
            Self::Edit => lucide_icons::Icon::Edit,
            Self::Delete => lucide_icons::Icon::Trash2,
            Self::Settings => lucide_icons::Icon::Settings,
            Self::ArrowLeft => lucide_icons::Icon::ArrowLeft,
            Self::ArrowRight => lucide_icons::Icon::ArrowRight,
            Self::Clock => lucide_icons::Icon::Clock,
            Self::MessageSquare => lucide_icons::Icon::MessageSquare,
            Self::Send => lucide_icons::Icon::Send,
            Self::RefreshCw => lucide_icons::Icon::RefreshCw,
            Self::Pointer => lucide_icons::Icon::Pointer,
            Self::TextCursorInput => lucide_icons::Icon::TextCursorInput,
            Self::CheckSquare => lucide_icons::Icon::CheckSquare,
            Self::Menu => lucide_icons::Icon::Menu,
            Self::Info => lucide_icons::Icon::Info,
            Self::StopCircle => lucide_icons::Icon::CircleStop,
            Self::Archive => lucide_icons::Icon::Archive,
            Self::Ellipsis => lucide_icons::Icon::Ellipsis,
            Self::Bot => lucide_icons::Icon::Bot,
            Self::User => lucide_icons::Icon::User,
            Self::Sparkles => lucide_icons::Icon::Sparkles,
            Self::List => lucide_icons::Icon::List,
            Self::Table => lucide_icons::Icon::Table,
            Self::LayoutPanelLeft => lucide_icons::Icon::LayoutPanelLeft,
            Self::LayoutGrid => lucide_icons::Icon::LayoutGrid,
            Self::Type => lucide_icons::Icon::Type,
            Self::Image => lucide_icons::Icon::Image,
            Self::Images => lucide_icons::Icon::Images,
            Self::Square => lucide_icons::Icon::Square,
            Self::Layers => lucide_icons::Icon::Layers,
            Self::Globe => lucide_icons::Icon::Globe,
            Self::Loader => lucide_icons::Icon::Loader,
            Self::Folder => lucide_icons::Icon::Folder,
        }
    }

    #[must_use]
    pub fn glyph(self) -> char {
        char::from(self.as_lucide())
    }
}

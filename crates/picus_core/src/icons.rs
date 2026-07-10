#![forbid(unsafe_code)]

/// Font family used by Windows Fluent Design symbol glyphs.
///
/// This is the WinUI `SymbolIcon` family on current Windows releases.
pub const FLUENT_SYMBOL_FONT_FAMILY: &str = "Segoe Fluent Icons";

/// Fluent icon fallback stack.
///
/// `Segoe MDL2 Assets` keeps older Windows installations and many WinUI symbol
/// codepoints working. `FabricMDL2Icons` matches Fluent UI web's historical
/// font icon package when an application chooses to register that font.
pub const FLUENT_SYMBOL_FONT_FALLBACKS: &[&str] = &[
    FLUENT_SYMBOL_FONT_FAMILY,
    "Segoe MDL2 Assets",
    "FabricMDL2Icons",
    "Segoe UI Symbol",
];

/// A resolved icon glyph plus the font stack required to draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconGlyph {
    glyph: char,
    font_families: &'static [&'static str],
}

impl IconGlyph {
    #[must_use]
    pub const fn new(glyph: char, font_families: &'static [&'static str]) -> Self {
        Self {
            glyph,
            font_families,
        }
    }

    #[must_use]
    pub const fn glyph(self) -> char {
        self.glyph
    }

    #[must_use]
    pub const fn font_families(self) -> &'static [&'static str] {
        self.font_families
    }

    #[must_use]
    pub fn font_family_vec(self) -> Vec<String> {
        self.font_families
            .iter()
            .map(|family| (*family).to_string())
            .collect()
    }
}

impl Default for IconGlyph {
    fn default() -> Self {
        Self::from(PicusIcon::Check)
    }
}

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
    /// Horizontal minus / dash (for indeterminate checkbox).
    Minus,
}

impl PicusIcon {
    #[must_use]
    pub fn glyph(self) -> char {
        self.as_fluent().glyph()
    }

    #[must_use]
    pub fn glyph_source(self) -> IconGlyph {
        self.into()
    }

    #[must_use]
    pub const fn as_fluent(self) -> FluentIcon {
        match self {
            Self::Check => FluentIcon::Checkmark,
            Self::ChevronDown => FluentIcon::ChevronDown,
            Self::ChevronUp => FluentIcon::ChevronUp,
            Self::ChevronRight => FluentIcon::ChevronRight,
            Self::Circle => FluentIcon::Placeholder,
            Self::CircleDot => FluentIcon::Accept,
            Self::X => FluentIcon::Cancel,
            Self::SunMoon => FluentIcon::Brightness,
            Self::Plus => FluentIcon::Add,
            Self::Search => FluentIcon::Search,
            Self::Edit => FluentIcon::Edit,
            Self::Delete => FluentIcon::Delete,
            Self::Settings => FluentIcon::Settings,
            Self::ArrowLeft => FluentIcon::Back,
            Self::ArrowRight => FluentIcon::Forward,
            Self::Clock => FluentIcon::Clock,
            Self::MessageSquare => FluentIcon::Message,
            Self::Send => FluentIcon::Send,
            Self::RefreshCw => FluentIcon::Refresh,
            Self::Pointer => FluentIcon::TouchPointer,
            Self::TextCursorInput => FluentIcon::Character,
            Self::CheckSquare => FluentIcon::Checkbox,
            Self::Menu => FluentIcon::GlobalNavigationButton,
            Self::Info => FluentIcon::Info,
            Self::StopCircle => FluentIcon::Stop,
            Self::Archive => FluentIcon::Folder,
            Self::Ellipsis => FluentIcon::More,
            Self::Bot => FluentIcon::Contact,
            Self::User => FluentIcon::Contact,
            Self::Sparkles => FluentIcon::Sync,
            Self::List => FluentIcon::List,
            Self::Table => FluentIcon::ViewAll,
            Self::LayoutPanelLeft => FluentIcon::DockLeft,
            Self::LayoutGrid => FluentIcon::AllApps,
            Self::Type => FluentIcon::Font,
            Self::Image => FluentIcon::Pictures,
            Self::Images => FluentIcon::AllApps,
            Self::Square => FluentIcon::Placeholder,
            Self::Layers => FluentIcon::Map,
            Self::Globe => FluentIcon::Globe,
            Self::Loader => FluentIcon::Sync,
            Self::Folder => FluentIcon::Folder,
            Self::Minus => FluentIcon::Remove,
        }
    }

    #[must_use]
    pub fn fluent_glyph_source(self) -> IconGlyph {
        self.as_fluent().into()
    }
}

impl From<PicusIcon> for IconGlyph {
    fn from(icon: PicusIcon) -> Self {
        Self::new(icon.glyph(), FLUENT_SYMBOL_FONT_FALLBACKS)
    }
}

/// Common Fluent Design icon glyphs backed by the Windows symbol font stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FluentIcon {
    Accept,
    Add,
    AllApps,
    Back,
    Brightness,
    Cancel,
    Character,
    Checkmark,
    Checkbox,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    Clock,
    Contact,
    Delete,
    DockLeft,
    Edit,
    Folder,
    Font,
    Forward,
    GlobalNavigationButton,
    Globe,
    Help,
    Info,
    List,
    Map,
    Message,
    More,
    Pictures,
    Placeholder,
    Refresh,
    Remove,
    Search,
    Send,
    Settings,
    Stop,
    Sync,
    TouchPointer,
    ViewAll,
}

impl FluentIcon {
    /// Return the Unicode codepoint for this icon.
    ///
    /// The core set follows WinUI's `Symbol` enum mapping to Segoe Fluent
    /// Icons. `Info` uses Fluent UI's MDL2 web mapping, with the same fallback
    /// stack used by the other glyphs.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Accept => '\u{E8FB}',
            Self::Add => '\u{E710}',
            Self::AllApps => '\u{E71D}',
            Self::Back => '\u{E72B}',
            Self::Brightness => '\u{E706}',
            Self::Cancel => '\u{E711}',
            Self::Character => '\u{E8C1}',
            Self::Checkmark => '\u{E73E}',
            Self::Checkbox => '\u{E73A}',
            Self::ChevronDown => '\u{E70D}',
            Self::ChevronLeft => '\u{E76B}',
            Self::ChevronRight => '\u{E76C}',
            Self::ChevronUp => '\u{E70E}',
            Self::Clock => '\u{E823}',
            Self::Contact => '\u{E77B}',
            Self::Delete => '\u{E74D}',
            Self::DockLeft => '\u{E90C}',
            Self::Edit => '\u{E70F}',
            Self::Folder => '\u{E8B7}',
            Self::Font => '\u{E8D2}',
            Self::Forward => '\u{E72A}',
            Self::GlobalNavigationButton => '\u{E700}',
            Self::Globe => '\u{E774}',
            Self::Help => '\u{E897}',
            Self::Info => '\u{E946}',
            Self::List => '\u{EA37}',
            Self::Map => '\u{E707}',
            Self::Message => '\u{E8BD}',
            Self::More => '\u{E712}',
            Self::Pictures => '\u{E8B9}',
            Self::Placeholder => '\u{E18A}',
            Self::Refresh => '\u{E72C}',
            Self::Remove => '\u{E738}',
            Self::Search => '\u{E721}',
            Self::Send => '\u{E724}',
            Self::Settings => '\u{E713}',
            Self::Stop => '\u{E71A}',
            Self::Sync => '\u{E895}',
            Self::TouchPointer => '\u{E7C9}',
            Self::ViewAll => '\u{E8A9}',
        }
    }

    #[must_use]
    pub const fn glyph_source(self) -> IconGlyph {
        IconGlyph::new(self.glyph(), FLUENT_SYMBOL_FONT_FALLBACKS)
    }
}

impl From<FluentIcon> for IconGlyph {
    fn from(icon: FluentIcon) -> Self {
        icon.glyph_source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picus_icon_converts_to_fluent_icon_glyph() {
        let glyph = IconGlyph::from(PicusIcon::Send);

        assert_eq!(glyph.glyph(), PicusIcon::Send.glyph());
        assert_eq!(glyph.font_families(), FLUENT_SYMBOL_FONT_FALLBACKS);
    }

    #[test]
    fn fluent_icon_uses_fluent_symbol_fallback_stack() {
        let glyph = IconGlyph::from(FluentIcon::Send);

        assert_eq!(glyph.glyph(), '\u{E724}');
        assert_eq!(glyph.font_families(), FLUENT_SYMBOL_FONT_FALLBACKS);
    }

    #[test]
    fn picus_icon_can_map_to_fluent_compatibility_glyph() {
        let glyph = PicusIcon::Send.fluent_glyph_source();

        assert_eq!(glyph.glyph(), FluentIcon::Send.glyph());
        assert_eq!(glyph.font_families(), FLUENT_SYMBOL_FONT_FALLBACKS);
    }

    #[test]
    fn picus_check_icon_uses_checkbox_checkmark_glyph() {
        let glyph = IconGlyph::from(PicusIcon::Check);

        assert_eq!(glyph.glyph(), FluentIcon::Checkmark.glyph());
        assert_ne!(glyph.glyph(), FluentIcon::Accept.glyph());
    }

    #[test]
    fn theme_icon_uses_brightness_instead_of_refresh() {
        let glyph = IconGlyph::from(PicusIcon::SunMoon);

        assert_eq!(glyph.glyph(), FluentIcon::Brightness.glyph());
        assert_ne!(glyph.glyph(), FluentIcon::Refresh.glyph());
        assert_ne!(glyph.glyph(), FluentIcon::Sync.glyph());
    }
}

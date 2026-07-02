use bevy_ecs::prelude::*;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Avatar size matching Fluent UI v9 AvatarSize.
pub type AvatarSize = u16;

/// Standard avatar sizes in pixels.
pub mod avatar_sizes {
    use super::AvatarSize;
    /// 16px — smallest avatar (e.g. for notification lists)
    pub const XS: AvatarSize = 16;
    /// 20px
    pub const SM: AvatarSize = 20;
    /// 24px
    pub const MD: AvatarSize = 24;
    /// 32px — default size
    pub const LG: AvatarSize = 32;
    /// 40px
    pub const XL: AvatarSize = 40;
    /// 48px
    pub const XXL: AvatarSize = 48;
    /// 56px
    pub const XXLG: AvatarSize = 56;
    /// 72px
    pub const XXXL: AvatarSize = 72;
    /// 96px
    pub const HUGE: AvatarSize = 96;
    /// 120px
    pub const JUMBO: AvatarSize = 120;
}

/// Shape of the avatar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AvatarShape {
    #[default]
    Circular,
    Square,
}

impl AvatarShape {
    #[must_use]
    pub const fn corner_radius_for_size(self, size: AvatarSize) -> f64 {
        match self {
            Self::Circular => size as f64 / 2.0,
            Self::Square => size as f64 * 0.15,
        }
    }
}

/// Avatar using initials extracted from the name.
///
/// When no name and no image URL is given, a fallback icon character is shown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiAvatar {
    /// Display name used to derive initials.
    pub name: String,
    /// Optional image URL for photo avatars.
    pub image_url: Option<String>,
    /// Pixel size (typically 16–128).
    pub size: AvatarSize,
    /// Shape — circular (default) or square.
    pub shape: AvatarShape,
    /// Optional named color from the avatar palette.
    /// If `None`, the avatar picks a deterministic color based on the name hash.
    pub color: Option<String>,
}

impl UiAvatar {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            image_url: None,
            size: avatar_sizes::LG,
            shape: AvatarShape::Circular,
            color: None,
        }
    }

    /// Set the pixel size.
    #[must_use]
    pub fn with_size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Set the shape.
    #[must_use]
    pub fn with_shape(mut self, shape: AvatarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Set an image URL for a photo avatar.
    #[must_use]
    pub fn with_image(mut self, url: impl Into<String>) -> Self {
        self.image_url = Some(url.into());
        self
    }

    /// Set a named color from the avatar palette.
    #[must_use]
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// Generate initials from a name string (max 2 characters).
///
/// Returns the first letter of the first word and, if available, the first letter
/// of the last word.  Falls back to "?" for empty names.
#[must_use]
pub fn get_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "?".to_string();
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let first = parts
        .first()
        .and_then(|w| w.chars().next())
        .map(|c| c.to_uppercase().next().unwrap_or('?'))
        .unwrap_or('?');

    let second = parts
        .get(1)
        .and_then(|w| w.chars().next())
        .map(|c| c.to_uppercase().next().unwrap_or('?'));

    match second {
        Some(snd) => format!("{first}{snd}"),
        None => format!("{first}"),
    }
}

/// Pick an avatar colour set index from a name hash.
#[must_use]
pub fn pick_avatar_color_index(name: &str) -> usize {
    if name.is_empty() {
        return 0;
    }
    let hash: u64 = name
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    hash as usize % AVATAR_COLOR_CLASSES.len()
}

/// Class names for the 25+ Fluent named avatar colours.
pub const AVATAR_COLOR_CLASSES: &[&str] = &[
    "avatar.color.dark-red",
    "avatar.color.cranberry",
    "avatar.color.red",
    "avatar.color.pumpkin",
    "avatar.color.peach",
    "avatar.color.marigold",
    "avatar.color.gold",
    "avatar.color.brass",
    "avatar.color.brown",
    "avatar.color.forest",
    "avatar.color.seafoam",
    "avatar.color.dark-green",
    "avatar.color.light-teal",
    "avatar.color.teal",
    "avatar.color.steel",
    "avatar.color.blue",
    "avatar.color.royal-blue",
    "avatar.color.cornflower",
    "avatar.color.navy",
    "avatar.color.lavender",
    "avatar.color.purple",
    "avatar.color.grape",
    "avatar.color.lilac",
    "avatar.color.pink",
    "avatar.color.magenta",
];

impl UiComponentTemplate for UiAvatar {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_avatar(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_uses_first_letter_of_first_and_last_word() {
        assert_eq!(get_initials("John Doe"), "JD");
    }

    #[test]
    fn initials_single_word_uses_first_letter() {
        assert_eq!(get_initials("Alice"), "A");
    }

    #[test]
    fn initials_empty_name_falls_back_to_question_mark() {
        assert_eq!(get_initials(""), "?");
    }

    #[test]
    fn initials_handles_extra_whitespace() {
        assert_eq!(get_initials("  hello   world  "), "HW");
    }

    #[test]
    fn color_index_is_deterministic() {
        let a = pick_avatar_color_index("Alice");
        let b = pick_avatar_color_index("Alice");
        assert_eq!(a, b);
    }

    #[test]
    fn avatar_default_size_is_32() {
        let av = UiAvatar::new("Test");
        assert_eq!(av.size, 32);
    }
}

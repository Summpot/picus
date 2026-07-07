//! Public icon view helper rendering a font glyph in a fixed-size box.
//!
//! This is the application-facing counterpart of the internal
//! `projection::elements::create_icon_view`. It lets Picus applications render
//! icons directly inside their own projection functions (e.g. for custom
//! buttons, status indicators, sidebar rows) without re-implementing the
//! glyph-label and font-stack setup.
//!
//! The helper composes the existing public `label` view with an icon font stack
//! and a sized-box bounding constraint, so it tracks the same rendering path as
//! every other label and needs no private widget APIs.

use std::borrow::Cow;

use picus_view::WidgetView;
use picus_view::masonry_core::layout::Length;
use picus_view::picus_widget::parley::{FontFamily, FontFamilyName};
use picus_view::picus_widget::peniko::Color;
use picus_view::style::Style as _;
use picus_view::view::{label, sized_box};

use crate::icons::{FluentIcon, IconGlyph, FLUENT_SYMBOL_FONT_FALLBACKS, PicusIcon};

/// Render an icon glyph as a fixed-size icon view.
///
/// `size_px` is the bounding box; the glyph itself is drawn at ~90% to leave
/// optical padding. `color` is the glyph color (use
/// [`picus_view::picus_widget::peniko::Color`] or a theme-resolved color).
///
/// # Example
/// ```
/// # use picus_core::xilem as xilem;
/// use picus_core::icon::icon;
/// use picus_core::icons::FluentIcon;
/// use xilem::palette;
/// use xilem::view::{FlexExt as _, flex_row, label};
/// # fn view() -> impl xilem::WidgetView<()> {
/// flex_row(vec![
///     icon(FluentIcon::Send, 18.0, palette::css::WHITE).into_any_flex(),
///     label("Send").into_any_flex(),
/// ])
/// # }
/// ```
#[must_use = "View values do nothing unless provided to Xilem."]
pub fn icon(icon: impl Into<IconGlyph>, size_px: f64, color: Color) -> impl WidgetView<()> {
    icon_source(icon, size_px, color)
}

/// Explicit helper for rendering a Fluent Design icon.
#[must_use = "View values do nothing unless provided to Xilem."]
pub fn fluent_icon(fluent_icon: FluentIcon, size_px: f64, color: Color) -> impl WidgetView<()> {
    icon_source(fluent_icon, size_px, color)
}

/// Explicit helper for rendering a bundled Picus/Lucide icon.
#[must_use = "View values do nothing unless provided to Xilem."]
pub fn picus_icon(picus_icon: PicusIcon, size_px: f64, color: Color) -> impl WidgetView<()> {
    icon_source(picus_icon, size_px, color)
}

/// Render a resolved icon glyph and its font stack.
#[must_use = "View values do nothing unless provided to Xilem."]
pub fn icon_source(icon: impl Into<IconGlyph>, size_px: f64, color: Color) -> impl WidgetView<()> {
    let icon = icon.into();
    icon_glyph_with_font_stack(icon.glyph(), icon.font_families(), size_px, color)
}

/// Like [`icon`] but takes a raw Lucide `char` glyph, for legacy callers that
/// already hold a [`PicusIcon::glyph`] value.
pub fn icon_glyph(glyph: char, size_px: f64, color: Color) -> impl WidgetView<()> {
    icon_glyph_with_font_stack(glyph, FLUENT_SYMBOL_FONT_FALLBACKS, size_px, color)
}

/// Like [`icon_glyph`] but with an explicit icon font fallback stack.
pub fn icon_glyph_with_font_stack(
    glyph: char,
    font_families: &'static [&'static str],
    size_px: f64,
    color: Color,
) -> impl WidgetView<()> {
    let font_family = match font_families {
        [] => FontFamily::List(Cow::Borrowed(&[])),
        [family] => FontFamily::Single(FontFamilyName::Named((*family).into())),
        families => FontFamily::List(Cow::Owned(
            families
                .iter()
                .map(|family| FontFamilyName::Named((*family).into()))
                .collect(),
        )),
    };
    let icon_label = label(glyph.to_string())
        .text_size((size_px * 0.90) as f32)
        .font(font_family)
        .color(color);
    sized_box(icon_label)
        .fixed_width(Length::px(size_px))
        .fixed_height(Length::px(size_px))
}

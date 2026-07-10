use picus_widget::core::{ArcStr, StyleProperty};
use picus_widget::parley::style::FontWeight;
use picus_widget::parley::{FontFamily, FontFamilyName, GenericFamily, LineHeight};
use picus_widget::peniko::Color;
use picus_widget::properties::ContentColor;
use picus_widget::widgets;

use crate::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use crate::{Pod, TextAlign, ViewCtx};

/// A non-interactive text element.
/// # Example
///
/// ```
/// # use picus_view as xilem;
/// use xilem::picus_widget::palette;
/// use xilem::view::label;
/// use xilem::style::Style as _;
/// use xilem::picus_widget::parley::Alignment as TextAlign;
/// use xilem::picus_widget::parley::style::FontWeight;
/// use xilem::picus_widget::parley::fontique;
/// # use xilem::WidgetView;
///
/// # fn view() -> impl WidgetView<()> {
/// label("Text example.")
///     .text_alignment(TextAlign::Center)
///     .text_size(24.0)
///     .letter_spacing(-0.3)
///     .weight(FontWeight::BOLD)
///     .font(fontique::GenericFamily::Serif)
///     .color(palette::css::RED)
/// # }
/// ```
pub fn label(label: impl Into<ArcStr>) -> Label {
    Label {
        label: label.into(),
        text_alignment: TextAlign::default(),
        text_size: picus_widget::theme::TEXT_SIZE_NORMAL,
        weight: FontWeight::NORMAL,
        enable_hinting: true,
        line_height: LineHeight::default(),
        font: FontFamily::Single(FontFamilyName::Generic(GenericFamily::SystemUi)),
        text_color: None,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        underline: false,
        strikethrough: false,
    }
}

/// The [`View`] created by [`label`] from a text which `impl Into<`[`ArcStr`]`>`.
///
/// See `label` documentation for more context.
#[must_use = "View values do nothing unless provided to Xilem."]
pub struct Label {
    label: ArcStr,
    text_alignment: TextAlign,
    text_size: f32,
    weight: FontWeight,
    enable_hinting: bool,
    line_height: LineHeight,
    font: FontFamily<'static>,
    text_color: Option<Color>,
    letter_spacing: f32,
    word_spacing: f32,
    underline: bool,
    strikethrough: bool,
    // TODO: add more attributes of `picus_widget::widgets::Label`
}

impl Label {
    /// Sets text alignment: `Start`, `Middle`, `End` or `Justified`.
    pub fn text_alignment(mut self, text_alignment: TextAlign) -> Self {
        self.text_alignment = text_alignment;
        self
    }

    /// Sets text size.
    #[doc(alias = "font_size")]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    /// Sets font weight.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Allows low-DPI [hinting](https://en.wikipedia.org/wiki/Font_hinting) for this label.
    ///
    /// Picus automatically disables hinting above 125% scale, where Vello's
    /// grayscale area anti-aliasing produces smoother compositor-safe text.
    pub fn enable_hinting(mut self, enable_hinting: bool) -> Self {
        self.enable_hinting = enable_hinting;
        self
    }

    /// Sets line height.
    pub fn line_height(mut self, line_height: LineHeight) -> Self {
        self.line_height = line_height;
        self
    }

    /// Sets font tracking width.
    #[doc(alias = "tracking")]
    pub fn letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }

    /// Sets word spacing width.
    pub fn word_spacing(mut self, word_spacing: f32) -> Self {
        self.word_spacing = word_spacing;
        self
    }

    /// Draw an underline below this label.
    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Draw a strikethrough line through this label.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    /// Set the [font family](FontFamily) this label will use.
    ///
    /// A font family allows for providing fallbacks. If there is no matching font
    /// for a character, a system font will be used (if the system fonts are enabled).
    pub fn font(mut self, font: impl Into<FontFamily<'static>>) -> Self {
        self.font = font.into();
        self
    }

    /// Set the text color.
    ///
    /// When unset, the view writes transparent text so a Picus surface without
    /// an active theme does not fall back to the retained widget's static black.
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }
}

impl<T> From<T> for Label
where
    T: Into<ArcStr>,
{
    fn from(text: T) -> Self {
        label(text)
    }
}

impl ViewMarker for Label {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_default_text() {
        let l = label("Hello");
        assert_eq!(l.label.as_ref(), "Hello");
    }

    #[test]
    fn label_builder_text_size() {
        let l = label("Test").text_size(32.0);
        assert!((l.text_size - 32.0).abs() < f32::EPSILON);
    }

    #[test]
    fn label_builder_text_alignment() {
        let l = label("Test").text_alignment(TextAlign::Center);
        assert_eq!(l.text_alignment, TextAlign::Center);
    }

    #[test]
    fn label_builder_weight() {
        let l = label("Bold").weight(FontWeight::BOLD);
        assert_eq!(l.weight, FontWeight::BOLD);
    }

    #[test]
    fn label_builder_color() {
        let color = picus_widget::peniko::Color::from_rgba8(255, 0, 0, 255);
        let l = label("Red").text_color(color);
        assert_eq!(l.text_color, Some(color));
    }

    #[test]
    fn label_builder_letter_spacing() {
        let l = label("Spread").letter_spacing(2.0);
        assert!((l.letter_spacing - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn label_builder_underline() {
        let l = label("Under").underline(true);
        assert!(l.underline);
    }

    #[test]
    fn label_builder_enable_hinting() {
        let l = label("Hinted").enable_hinting(false);
        assert!(!l.enable_hinting);
    }

    #[test]
    fn label_text_color_default_none() {
        let l = label("Default");
        assert_eq!(l.text_color, None);
    }

    #[test]
    fn label_from_string_via_trait() {
        let l: Label = "Converted".into();
        assert_eq!(l.label.as_ref(), "Converted");
    }

    #[test]
    fn label_default_values() {
        let l = label("Defaults");
        assert_eq!(l.text_alignment, TextAlign::default());
        assert!((l.text_size - picus_widget::theme::TEXT_SIZE_NORMAL).abs() < f32::EPSILON);
        assert_eq!(l.weight, FontWeight::NORMAL);
        assert!(l.enable_hinting);
        assert!(!l.underline);
        assert!(!l.strikethrough);
    }
}

impl<State: 'static, Action> View<State, Action, ViewCtx> for Label {
    type Element = Pod<widgets::Label>;
    type ViewState = ();

    fn build(&self, _ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, Self::ViewState) {
        let label = widgets::Label::new(self.label.clone())
            .with_text_alignment(self.text_alignment)
            .with_style(StyleProperty::FontSize(self.text_size))
            .with_style(StyleProperty::FontWeight(self.weight))
            .with_style(StyleProperty::LineHeight(self.line_height))
            .with_style(StyleProperty::FontFamily(self.font.clone()))
            .with_style(StyleProperty::WordSpacing(self.word_spacing))
            .with_style(StyleProperty::LetterSpacing(self.letter_spacing))
            .with_style(StyleProperty::Underline(self.underline))
            .with_style(StyleProperty::Strikethrough(self.strikethrough))
            .with_hint(self.enable_hinting);

        let pod = Pod::new_with_props(
            label,
            ContentColor {
                color: self.text_color.unwrap_or(Color::TRANSPARENT),
            },
        );
        (pod, ())
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut State,
    ) {
        if prev.label != self.label {
            widgets::Label::set_text(&mut element, self.label.clone());
        }
        if prev.text_alignment != self.text_alignment {
            widgets::Label::set_text_alignment(&mut element, self.text_alignment);
        }
        if prev.text_size != self.text_size {
            widgets::Label::insert_style(&mut element, StyleProperty::FontSize(self.text_size));
        }
        if prev.weight != self.weight {
            widgets::Label::insert_style(&mut element, StyleProperty::FontWeight(self.weight));
        }
        if prev.line_height != self.line_height {
            widgets::Label::insert_style(&mut element, StyleProperty::LineHeight(self.line_height));
        }
        if prev.letter_spacing != self.letter_spacing {
            widgets::Label::insert_style(
                &mut element,
                StyleProperty::LetterSpacing(self.letter_spacing),
            );
        }
        if prev.word_spacing != self.word_spacing {
            widgets::Label::insert_style(
                &mut element,
                StyleProperty::WordSpacing(self.word_spacing),
            );
        }
        if prev.font != self.font {
            widgets::Label::insert_style(
                &mut element,
                StyleProperty::FontFamily(self.font.clone()),
            );
        }
        if prev.underline != self.underline {
            widgets::Label::insert_style(&mut element, StyleProperty::Underline(self.underline));
        }
        if prev.strikethrough != self.strikethrough {
            widgets::Label::insert_style(
                &mut element,
                StyleProperty::Strikethrough(self.strikethrough),
            );
        }
        if prev.text_color != self.text_color {
            element.insert_prop(ContentColor {
                color: self.text_color.unwrap_or(Color::TRANSPARENT),
            });
        }
        if prev.enable_hinting != self.enable_hinting {
            widgets::Label::set_hint(&mut element, self.enable_hinting);
        }
    }

    fn teardown(&self, (): &mut Self::ViewState, _: &mut ViewCtx, _: Mut<'_, Self::Element>) {}

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _element: Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) -> MessageResult<Action> {
        tracing::error!(
            ?message,
            "Message arrived in Label::message, but Label doesn't consume any messages, this is a bug"
        );
        MessageResult::Stale
    }
}

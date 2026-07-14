use std::{
    any::TypeId,
    borrow::Cow,
    collections::{HashMap, HashSet},
    io,
    time::Duration,
};

use crate::bevy_tween::{
    bevy_time_runner::{TimeContext, TimeRunner, TimeSpan},
    interpolate::Interpolator,
    interpolation::EaseKind,
    tween::{ComponentTween, TweenInterpolationValue, TweenPreviousValue},
};
use crate::xilem::{Color, style::Style as _};
use bevy_asset::{
    Asset, AssetEvent, AssetLoader, AssetServer, Assets, Handle, LoadContext, io::Reader,
};
use bevy_ecs::{
    change_detection::Mut,
    component::ComponentId,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    message::{MessageCursor, Messages},
    prelude::*,
};
use bevy_reflect::TypePath;
use bevy_time::Time;
use masonry_core::core::UsesProperty;
use masonry_core::{
    layout::Length,
    parley::{
        Alignment as ParleyTextAlign, FontFamily, FontFamilyName, GenericFamily, LineHeight,
        style::FontWeight,
    },
    properties::{Background, BorderColor, BorderWidth, BoxShadow, CornerRadius, Padding},
};
use picus_view::picus_widget::properties::{
    AbsoluteLinearGradient, BorderBrush, LineBreaking,
};
use picus_view::picus_widget::properties::types::Gradient;
use picus_view::{
    WidgetView,
    view::{CrossAxisAlignment, Flex, Label, MainAxisAlignment, TextInput, sized_box, transformed},
};
use serde::{
    Deserialize,
    de::{
        self, EnumAccess, IntoDeserializer, VariantAccess, Visitor,
        value::{MapAccessDeserializer, SeqAccessDeserializer},
    },
};

use crate::{UiEventQueue, WindowBackdropColorScheme, WindowBackdropMaterial};

pub(crate) const DEFAULT_TEXT_SIZE: f32 = 15.0;

/// Conventional Fluent theme token that selects the top-level window backdrop.
pub const WINDOW_BACKDROP_TOKEN: &str = "window-backdrop";

/// Marker component for CSS-like class names attached to an entity.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleClass(pub Vec<String>);

/// Marker component for entities whose style cache needs recomputation.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct StyleDirty;

/// Transient interaction state used for pseudo-class styling.
///
/// This replaces frequently inserted/removed marker components like `Hovered`/`Pressed`.
/// Keeping state in a stable component avoids archetype churn when the pointer moves.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InteractionState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
}

/// Delays entry into the hovered pseudo-class to reduce hover flicker.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct HoverDebounce {
    pub enter_delay_secs: f32,
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PendingHoverState {
    entered_at_secs: f64,
}

/// Consolidated inline style overrides.
///
/// This is a "mega-component" that reduces archetype fragmentation vs inserting a
/// handful of smaller optional style components.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct InlineStyle {
    pub layout: LayoutStyle,
    pub colors: ColorStyle,
    pub text: TextStyle,
    pub transition: Option<StyleTransition>,
}

impl InlineStyle {
    /// Start from an empty inline override set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn padding(mut self, padding: f64) -> Self {
        self.layout.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn gap(mut self, gap: f64) -> Self {
        self.layout.gap = Some(gap);
        self
    }

    #[must_use]
    pub fn corner_radius(mut self, corner_radius: f64) -> Self {
        self.layout.corner_radius = Some(corner_radius);
        self
    }

    #[must_use]
    pub fn border_width(mut self, border_width: f64) -> Self {
        self.layout.border_width = Some(border_width);
        self
    }

    #[must_use]
    pub fn flex_grow(mut self, flex_grow: f64) -> Self {
        self.layout.flex_grow = Some(flex_grow);
        self
    }

    #[must_use]
    pub fn bg(mut self, color: Color) -> Self {
        self.colors.bg = Some(color);
        self
    }

    #[must_use]
    pub fn text_color(mut self, color: Color) -> Self {
        self.colors.text = Some(color);
        self
    }

    #[must_use]
    pub fn border_color(mut self, color: Color) -> Self {
        self.colors.border = Some(color);
        self
    }

    #[must_use]
    pub fn text_size(mut self, size: f32) -> Self {
        self.text.size = Some(size);
        self
    }

    #[must_use]
    pub fn text_weight(mut self, weight: f32) -> Self {
        self.text.weight = Some(weight);
        self
    }
}

/// Inline layout style that can be attached to entities.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct LayoutStyle {
    pub padding: Option<f64>,
    pub gap: Option<f64>,
    pub corner_radius: Option<f64>,
    pub border_width: Option<f64>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub scale: Option<f64>,
    /// Flex grow factor. When this entity is a child of a flex container,
    /// it will take remaining space proportional to this factor.
    /// `None` = `0.0` (no grow).
    pub flex_grow: Option<f64>,
}

/// Inline color style that can be attached to entities.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct ColorStyle {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    pub border: Option<Color>,
    pub hover_bg: Option<Color>,
    pub hover_text: Option<Color>,
    pub hover_border: Option<Color>,
    pub pressed_bg: Option<Color>,
    pub pressed_text: Option<Color>,
    pub pressed_border: Option<Color>,
}

/// Inline text style that can be attached to entities.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct TextStyle {
    pub size: Option<f32>,
    pub text_align: Option<TextAlign>,
    /// Font weight (100–900). Maps directly to parley `FontWeight`.
    /// Common values: 400 (Normal/Regular), 500 (Medium), 600 (Semibold), 700 (Bold).
    pub weight: Option<f32>,
    /// Relative line height multiplier (e.g., 1.35). None = default.
    pub line_height: Option<f32>,
}

/// Main-axis content distribution for flex layouts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Cross-axis alignment for flex layouts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Text alignment for label-like UI components.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Transition settings for style animation.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct StyleTransition {
    /// Duration in seconds.
    pub duration: f32,
    /// Optional easing curve for the transition.
    /// When `None`, the default easing (QuadraticInOut) is used.
    #[serde(default)]
    pub easing: Option<EaseKind>,
}

/// System accessibility preference: has the user requested reduced motion?
///
/// When set to `true`, all UI transitions are skipped (colors jump to their
/// target values instantly).  This mirrors the CSS
/// `@media (prefers-reduced-motion: reduce)` behaviour.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReducedMotion(pub bool);

/// Cached resolved style used by projectors.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ComputedStyle {
    pub layout: ResolvedLayoutStyle,
    pub colors: ResolvedColorStyle,
    pub text: ResolvedTextStyle,
    pub font_family: Option<Vec<String>>,
    pub box_shadow: Option<BoxShadow>,
    pub transition: Option<StyleTransition>,
}

/// Interpolated color state currently rendered by projectors.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CurrentColorStyle {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    pub border: Option<Color>,
    pub scale: f64,
}

/// Target color state derived from classes + inline style + pseudo state.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TargetColorStyle {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    pub border: Option<Color>,
    pub scale: f64,
}

impl Default for CurrentColorStyle {
    fn default() -> Self {
        Self {
            bg: None,
            text: None,
            border: None,
            scale: 1.0,
        }
    }
}

impl Default for TargetColorStyle {
    fn default() -> Self {
        Self {
            bg: None,
            text: None,
            border: None,
            scale: 1.0,
        }
    }
}

/// Marker identifying a `bevy_tween` runner created by the style transition pipeline.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StyleManagedTween;

/// Pseudo classes supported by selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum PseudoClass {
    Hovered,
    Pressed,
    Focused,
}

/// CSS-like selector AST for style rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Selector {
    Type(TypeId),
    TypeName(String),
    Class(String),
    PseudoClass(PseudoClass),
    And(Vec<Selector>),
    Descendant {
        ancestor: Box<Selector>,
        descendant: Box<Selector>,
    },
}

impl Selector {
    #[must_use]
    pub fn of_type<T: Component>() -> Self {
        Self::Type(TypeId::of::<T>())
    }

    #[must_use]
    pub fn class(name: impl Into<String>) -> Self {
        Self::Class(name.into())
    }

    #[must_use]
    pub fn type_name(name: impl Into<String>) -> Self {
        Self::TypeName(name.into())
    }

    #[must_use]
    pub const fn pseudo(pseudo: PseudoClass) -> Self {
        Self::PseudoClass(pseudo)
    }

    #[must_use]
    pub fn and(selectors: impl Into<Vec<Selector>>) -> Self {
        Self::And(selectors.into())
    }

    #[must_use]
    pub fn descendant(ancestor: Selector, descendant: Selector) -> Self {
        Self::Descendant {
            ancestor: Box::new(ancestor),
            descendant: Box::new(descendant),
        }
    }

    #[must_use]
    fn contains_type(&self) -> bool {
        match self {
            Selector::Type(_) | Selector::TypeName(_) => true,
            Selector::Class(_) | Selector::PseudoClass(_) => false,
            Selector::And(selectors) => selectors.iter().any(Self::contains_type),
            Selector::Descendant {
                ancestor,
                descendant,
            } => ancestor.contains_type() || descendant.contains_type(),
        }
    }

    #[must_use]
    fn contains_descendant(&self) -> bool {
        match self {
            Selector::Descendant { .. } => true,
            Selector::And(selectors) => selectors.iter().any(Self::contains_descendant),
            Selector::Type(_)
            | Selector::TypeName(_)
            | Selector::Class(_)
            | Selector::PseudoClass(_) => false,
        }
    }
}

/// Style payload set by a matching rule.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSetter {
    pub layout: LayoutStyle,
    pub colors: ColorStyle,
    pub border_brush: Option<BorderBrush>,
    pub text: TextStyle,
    pub font_family: Option<Vec<String>>,
    pub box_shadow: Option<BoxShadow>,
    pub transition: Option<StyleTransition>,
}

/// Style payload value that can be either an explicit value or a token reference.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleValue<T> {
    Value(T),
    Var(String),
}

impl<T> StyleValue<T> {
    #[must_use]
    pub fn value(value: T) -> Self {
        Self::Value(value)
    }

    #[must_use]
    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(name.into())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutStyleValue {
    pub padding: Option<StyleValue<f64>>,
    pub gap: Option<StyleValue<f64>>,
    pub corner_radius: Option<StyleValue<f64>>,
    pub border_width: Option<StyleValue<f64>>,
    pub justify_content: Option<StyleValue<JustifyContent>>,
    pub align_items: Option<StyleValue<AlignItems>>,
    pub scale: Option<StyleValue<f64>>,
    pub flex_grow: Option<StyleValue<f64>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColorStyleValue {
    pub bg: Option<StyleValue<Color>>,
    pub text: Option<StyleValue<Color>>,
    pub border: Option<StyleValue<Color>>,
    pub hover_bg: Option<StyleValue<Color>>,
    pub hover_text: Option<StyleValue<Color>>,
    pub hover_border: Option<StyleValue<Color>>,
    pub pressed_bg: Option<StyleValue<Color>>,
    pub pressed_text: Option<StyleValue<Color>>,
    pub pressed_border: Option<StyleValue<Color>>,
    pub border_brush: Option<StyleValue<BorderBrush>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextStyleValue {
    pub size: Option<StyleValue<f32>>,
    pub text_align: Option<StyleValue<TextAlign>>,
    /// Font weight as a token-value or literal (100–900).
    pub weight: Option<StyleValue<f32>>,
    /// Relative line height multiplier as a token-value or literal.
    pub line_height: Option<StyleValue<f32>>,
}

/// Token-aware style payload attached to stylesheet rules.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSetterValue {
    pub layout: LayoutStyleValue,
    pub colors: ColorStyleValue,
    pub text: TextStyleValue,
    pub font_family: Option<StyleValue<Vec<String>>>,
    pub box_shadow: Option<StyleValue<BoxShadow>>,
    pub transition: Option<StyleValue<StyleTransition>>,
}

/// Token value stored in [`StyleSheet::tokens`].
#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    Color(Color),
    Float(f64),
    FontFamily(Vec<String>),
    BoxShadow(BoxShadow),
    Transition(StyleTransition),
    /// Native top-level window backdrop material.
    Backdrop(WindowBackdropMaterial),
    /// Cubic bezier curve control points (x1, y1, x2, y2).
    Curve(f32, f32, f32, f32),
    /// Border brush (solid color or gradient).
    BorderBrush(BorderBrush),
}

/// Token and selector overrides activated for one window backdrop material.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BackdropStyle {
    pub tokens: HashMap<String, TokenValue>,
    pub rules: Vec<StyleRule>,
}

/// Theme-level backdrop selection and material-specific style overrides.
///
/// `material` commonly references [`WINDOW_BACKDROP_TOKEN`]. The selected
/// [`BackdropStyle`] is merged over the normal variant before it becomes the
/// live stylesheet, allowing one object to define which controls reveal each
/// native material.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeBackdrop {
    pub material: StyleValue<WindowBackdropMaterial>,
    pub color_scheme: WindowBackdropColorScheme,
    pub styles: HashMap<WindowBackdropMaterial, BackdropStyle>,
}

impl From<LayoutStyle> for LayoutStyleValue {
    fn from(value: LayoutStyle) -> Self {
        Self {
            padding: value.padding.map(StyleValue::value),
            gap: value.gap.map(StyleValue::value),
            corner_radius: value.corner_radius.map(StyleValue::value),
            border_width: value.border_width.map(StyleValue::value),
            justify_content: value.justify_content.map(StyleValue::value),
            align_items: value.align_items.map(StyleValue::value),
            scale: value.scale.map(StyleValue::value),
            flex_grow: value.flex_grow.map(StyleValue::value),
        }
    }
}

impl From<ColorStyle> for ColorStyleValue {
    fn from(value: ColorStyle) -> Self {
        Self {
            bg: value.bg.map(StyleValue::value),
            text: value.text.map(StyleValue::value),
            border: value.border.map(StyleValue::value),
            hover_bg: value.hover_bg.map(StyleValue::value),
            hover_text: value.hover_text.map(StyleValue::value),
            hover_border: value.hover_border.map(StyleValue::value),
            pressed_bg: value.pressed_bg.map(StyleValue::value),
            pressed_text: value.pressed_text.map(StyleValue::value),
            pressed_border: value.pressed_border.map(StyleValue::value),
            border_brush: None,
        }
    }
}

impl From<TextStyle> for TextStyleValue {
    fn from(value: TextStyle) -> Self {
        Self {
            size: value.size.map(StyleValue::value),
            text_align: value.text_align.map(StyleValue::value),
            weight: value.weight.map(StyleValue::value),
            line_height: value.line_height.map(StyleValue::value),
        }
    }
}

impl From<StyleSetter> for StyleSetterValue {
    fn from(value: StyleSetter) -> Self {
        Self {
            layout: value.layout.into(),
            colors: value.colors.into(),
            text: value.text.into(),
            font_family: value.font_family.map(StyleValue::value),
            box_shadow: value.box_shadow.map(StyleValue::value),
            transition: value.transition.map(StyleValue::value),
        }
    }
}

/// Selector + style payload.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
    pub selector: Selector,
    pub setter: StyleSetterValue,
}

impl StyleRule {
    #[must_use]
    pub fn new(selector: Selector, setter: StyleSetter) -> Self {
        Self {
            selector,
            setter: setter.into(),
        }
    }

    #[must_use]
    pub fn new_with_values(selector: Selector, setter: StyleSetterValue) -> Self {
        Self { selector, setter }
    }

    #[must_use]
    pub fn class(class_name: impl Into<String>, setter: StyleSetter) -> Self {
        Self::new(Selector::class(class_name), setter)
    }
}

/// Global class-based style table.
#[derive(Resource, Asset, TypePath, Debug, Clone, Default)]
pub struct StyleSheet {
    pub default_variant: Option<String>,
    pub font_family: Option<StyleValue<Vec<String>>>,
    pub backdrop: Option<ThemeBackdrop>,
    pub tokens: HashMap<String, TokenValue>,
    pub rules: Vec<StyleRule>,
}

/// Baseline stylesheet tier populated from the embedded built-in theme.
#[derive(Resource, Debug, Clone, Default)]
pub struct BaseStyleSheet(pub StyleSheet);

/// Active user stylesheet tier loaded from external `.ron` assets.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveStyleSheet(pub StyleSheet);

/// Handle/path of the active stylesheet asset used for runtime style hot-reload.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveStyleSheetAsset {
    pub handle: Option<Handle<StyleSheet>>,
    pub path: Option<String>,
}

/// Message cursor for [`AssetEvent<StyleSheet>`] in world-exclusive systems.
#[derive(Resource, Default)]
pub struct StyleAssetEventCursor(pub MessageCursor<AssetEvent<StyleSheet>>);

/// Selector set currently owned by the active stylesheet asset.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveStyleSheetSelectors(pub HashSet<Selector>);

/// Token names currently owned by the active stylesheet asset.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveStyleSheetTokenNames(pub HashSet<String>);

/// Registered named style variants parsed from a variant bundle.
#[derive(Resource, Debug, Clone, Default)]
pub struct RegisteredStyleVariants {
    pub default_variant: String,
    pub variants: HashMap<String, StyleSheet>,
}

/// Desired runtime style variant name.
///
/// When changed, [`sync_active_style_variant`] applies the corresponding
/// registered variant to [`BaseStyleSheet`] and the live [`StyleSheet`].
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveStyleVariant(pub Option<String>);

/// Last successfully applied runtime style variant name.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedStyleVariant(pub Option<String>);

/// Optional application override for the active theme's backdrop token.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThemeBackdropOverride(pub Option<WindowBackdropMaterial>);

/// Name-to-component-type map used by selector type tags loaded from RON assets.
#[derive(Resource, Debug, Clone, Default)]
pub struct StyleTypeRegistry {
    by_name: HashMap<String, TypeId>,
}

impl StyleTypeRegistry {
    pub fn register_type_name<T: Component>(&mut self, name: impl Into<String>) {
        self.by_name.insert(name.into(), TypeId::of::<T>());
    }

    pub fn register_type_aliases<T: Component>(&mut self) {
        let full = std::any::type_name::<T>();
        self.register_type_name::<T>(full);
        if let Some(short) = full.rsplit("::").next() {
            self.register_type_name::<T>(short);
        }
    }

    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<TypeId> {
        self.by_name.get(name).copied()
    }
}

impl StyleSheet {
    #[must_use]
    pub fn with_rule(mut self, rule: StyleRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn add_rule(&mut self, rule: StyleRule) {
        self.rules.push(rule);
    }

    #[must_use]
    pub fn with_class(mut self, class_name: impl Into<String>, setter: StyleSetter) -> Self {
        self.set_class(class_name, setter);
        self
    }

    #[must_use]
    pub fn with_class_values(
        mut self,
        class_name: impl Into<String>,
        setter: StyleSetterValue,
    ) -> Self {
        self.set_class_values(class_name, setter);
        self
    }

    pub fn set_class(&mut self, class_name: impl Into<String>, setter: StyleSetter) {
        self.set_class_values(class_name, setter.into());
    }

    pub fn set_class_values(&mut self, class_name: impl Into<String>, setter: StyleSetterValue) {
        let class_name = class_name.into();
        if let Some(existing) = self.rules.iter_mut().find(|rule| {
            matches!(&rule.selector, Selector::Class(existing_name) if existing_name == &class_name)
        }) {
            existing.setter = setter;
            return;
        }

        self.rules.push(StyleRule::new_with_values(
            Selector::class(class_name),
            setter,
        ));
    }

    #[must_use]
    pub fn get_class(&self, class_name: &str) -> Option<StyleSetter> {
        self.get_class_values(class_name)
            .map(|setter| resolve_setter_values(setter, &self.tokens))
    }

    #[must_use]
    pub fn get_class_values(&self, class_name: &str) -> Option<&StyleSetterValue> {
        self.rules.iter().find_map(|rule| {
            if matches!(&rule.selector, Selector::Class(name) if name == class_name) {
                Some(&rule.setter)
            } else {
                None
            }
        })
    }

    #[must_use]
    fn has_type_selectors(&self) -> bool {
        self.rules.iter().any(|rule| rule.selector.contains_type())
    }

    #[must_use]
    fn has_descendant_selectors(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.selector.contains_descendant())
    }
}

fn upsert_rule_by_selector(sheet: &mut StyleSheet, incoming: StyleRule) {
    if let Some(existing) = sheet
        .rules
        .iter_mut()
        .find(|rule| rule.selector == incoming.selector)
    {
        *existing = incoming;
    } else {
        sheet.rules.push(incoming);
    }
}

fn upsert_rules_by_selector(sheet: &mut StyleSheet, incoming: Vec<StyleRule>) {
    for rule in incoming {
        upsert_rule_by_selector(sheet, rule);
    }
}

fn merge_sheet_inplace(sheet: &mut StyleSheet, incoming: StyleSheet) {
    if incoming.font_family.is_some() {
        sheet.font_family = incoming.font_family;
    }
    if incoming.backdrop.is_some() {
        sheet.backdrop = incoming.backdrop;
    }
    for (name, token) in incoming.tokens {
        sheet.tokens.insert(name, token);
    }
    upsert_rules_by_selector(sheet, incoming.rules);
}

fn resolve_backdrop_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<WindowBackdropMaterial>,
) -> Option<WindowBackdropMaterial> {
    match value {
        StyleValue::Value(material) => Some(*material),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Backdrop(material)) => Some(*material),
            _ => {
                warn_missing_or_invalid_token(token, "backdrop.material", "Backdrop");
                None
            }
        },
    }
}

/// Resolve the native window material selected by a stylesheet backdrop object.
#[must_use]
pub fn resolve_theme_backdrop_material(
    stylesheet: &StyleSheet,
) -> Option<WindowBackdropMaterial> {
    stylesheet
        .backdrop
        .as_ref()
        .and_then(|backdrop| resolve_backdrop_value(&stylesheet.tokens, &backdrop.material))
}

/// Resolve the native light/dark appearance selected by a stylesheet backdrop.
#[must_use]
pub fn resolve_theme_backdrop_color_scheme(
    stylesheet: &StyleSheet,
) -> Option<WindowBackdropColorScheme> {
    stylesheet
        .backdrop
        .as_ref()
        .map(|backdrop| backdrop.color_scheme)
}

fn apply_selected_backdrop_style(stylesheet: &mut StyleSheet) {
    let Some(material) = resolve_theme_backdrop_material(stylesheet) else {
        return;
    };
    let Some(style) = stylesheet
        .backdrop
        .as_ref()
        .and_then(|backdrop| {
            backdrop.styles.get(&material).or_else(|| match material {
                WindowBackdropMaterial::Auto | WindowBackdropMaterial::MicaAlt => {
                    backdrop.styles.get(&WindowBackdropMaterial::Mica)
                }
                WindowBackdropMaterial::None
                | WindowBackdropMaterial::Mica
                | WindowBackdropMaterial::Acrylic => None,
            })
        })
        .cloned()
    else {
        return;
    };

    stylesheet.tokens.extend(style.tokens);
    upsert_rules_by_selector(stylesheet, style.rules);
}

fn apply_theme_backdrop_override(world: &World, stylesheet: &mut StyleSheet) {
    let Some(material) = world
        .get_resource::<ThemeBackdropOverride>()
        .and_then(|override_material| override_material.0)
    else {
        return;
    };

    stylesheet.tokens.insert(
        WINDOW_BACKDROP_TOKEN.to_string(),
        TokenValue::Backdrop(material),
    );
}

fn apply_base_stylesheet(world: &mut World, new_base: StyleSheet) {
    world.init_resource::<BaseStyleSheet>();
    world.init_resource::<StyleSheet>();
    world.init_resource::<ActiveStyleSheetTokenNames>();

    let (previous_base_selectors, previous_base_tokens) = {
        let previous = &world.resource::<BaseStyleSheet>().0;
        (
            previous
                .rules
                .iter()
                .map(|rule| rule.selector.clone())
                .collect::<HashSet<_>>(),
            previous.tokens.keys().cloned().collect::<HashSet<_>>(),
        )
    };

    let mut new_base = new_base;
    apply_theme_backdrop_override(world, &mut new_base);
    apply_selected_backdrop_style(&mut new_base);
    world.resource_mut::<BaseStyleSheet>().0 = new_base.clone();

    let active_font_family = world
        .get_resource::<ActiveStyleSheet>()
        .and_then(|active| active.0.font_family.clone());
    let active_backdrop = world
        .get_resource::<ActiveStyleSheet>()
        .and_then(|active| active.0.backdrop.clone());
    let active_selectors = world
        .get_resource::<ActiveStyleSheetSelectors>()
        .map(|selectors| selectors.0.clone())
        .unwrap_or_default();
    let active_tokens = world
        .get_resource::<ActiveStyleSheetTokenNames>()
        .map(|names| names.0.clone())
        .unwrap_or_default();

    let mut runtime_sheet = world.resource_mut::<StyleSheet>();
    runtime_sheet.font_family = active_font_family.or(new_base.font_family.clone());
    runtime_sheet.backdrop = active_backdrop.or(new_base.backdrop.clone());
    runtime_sheet.rules.retain(|rule| {
        !previous_base_selectors.contains(&rule.selector)
            || active_selectors.contains(&rule.selector)
    });
    runtime_sheet
        .tokens
        .retain(|name, _| !previous_base_tokens.contains(name) || active_tokens.contains(name));

    for (name, token) in new_base.tokens {
        if !active_tokens.contains(name.as_str()) {
            runtime_sheet.tokens.insert(name, token);
        }
    }

    let filtered_rules = new_base
        .rules
        .into_iter()
        .filter(|rule| !active_selectors.contains(&rule.selector))
        .collect::<Vec<_>>();
    upsert_rules_by_selector(&mut runtime_sheet, filtered_rules);
}

/// Embedded Fluent theme bundle containing multiple named variants.
pub const BUILTIN_FLUENT_THEME_RON: &str = include_str!("theme/fluent_theme.ron");

/// Register built-in ECS component type aliases usable from RON selectors.
pub fn register_builtin_style_type_aliases(world: &mut World) {
    world.init_resource::<StyleTypeRegistry>();
    let mut registry = world.resource_mut::<StyleTypeRegistry>();

    use crate::ecs::*;
    registry.register_type_aliases::<UiRoot>();
    registry.register_type_aliases::<UiOverlayRoot>();
    registry.register_type_aliases::<UiFlexColumn>();
    registry.register_type_aliases::<UiFlexRow>();
    registry.register_type_aliases::<UiGrid>();
    registry.register_type_aliases::<UiGridCell>();
    registry.register_type_aliases::<UiLabel>();
    registry.register_type_aliases::<UiButton>();
    registry.register_type_aliases::<UiBadge>();
    registry.register_type_aliases::<UiCanvas>();
    registry.register_type_aliases::<UiCanvasPosition>();
    registry.register_type_aliases::<UiCheckbox>();
    registry.register_type_aliases::<UiSlider>();
    registry.register_type_aliases::<UiSwitch>();
    registry.register_type_aliases::<UiTextInput>();
    registry.register_type_aliases::<UiPasswordInput>();
    registry.register_type_aliases::<UiMultilineTextInput>();
    registry.register_type_aliases::<UiImage>();
    registry.register_type_aliases::<UiProgressBar>();
    registry.register_type_aliases::<UiDialog>();
    registry.register_type_aliases::<UiComboBox>();
    registry.register_type_aliases::<UiDropdownMenu>();
    registry.register_type_aliases::<UiRadioGroup>();
    registry.register_type_aliases::<UiScrollView>();
    registry.register_type_aliases::<UiTabBar>();
    registry.register_type_aliases::<UiListView>();
    registry.register_type_aliases::<UiTreeNode>();
    registry.register_type_aliases::<UiTable>();
    registry.register_type_aliases::<UiDataTable>();
    registry.register_type_aliases::<UiMenuBar>();
    registry.register_type_aliases::<UiMenuBarItem>();
    registry.register_type_aliases::<UiMenuItemPanel>();
    registry.register_type_aliases::<UiTooltip>();
    registry.register_type_aliases::<UiSpinner>();
    registry.register_type_aliases::<UiColorPicker>();
    registry.register_type_aliases::<UiColorPickerPanel>();
    registry.register_type_aliases::<UiGroupBox>();
    registry.register_type_aliases::<UiSplitPane>();
    registry.register_type_aliases::<UiToast>();
    registry.register_type_aliases::<UiDatePicker>();
    registry.register_type_aliases::<UiDatePickerPanel>();
}

/// Set the active stylesheet asset path used for loading + hot-reload.
pub fn set_active_stylesheet_asset_path(world: &mut World, asset_path: impl Into<String>) {
    let asset_path = asset_path.into();
    world.init_resource::<ActiveStyleSheetAsset>();

    let needs_reload = world
        .get_resource::<ActiveStyleSheetAsset>()
        .is_none_or(|active| active.path.as_deref() != Some(asset_path.as_str()));

    if !needs_reload {
        return;
    }

    let mut active = world.resource_mut::<ActiveStyleSheetAsset>();
    active.path = Some(asset_path);
    active.handle = None;
}

/// Parse stylesheet RON text into a runtime [`StyleSheet`].
pub fn parse_stylesheet_ron(ron_text: &str) -> io::Result<StyleSheet> {
    stylesheet_from_ron_bytes(ron_text.as_bytes())
}

/// Parse and apply an active stylesheet from embedded RON text.
///
/// This updates [`ActiveStyleSheet`] and overlays the parsed rules/tokens onto
/// the runtime [`StyleSheet`] as the active tier (same precedence as file-based
/// active stylesheets), without requiring filesystem asset loading. If the
/// stylesheet declares `default_variant` and no active variant has already been
/// selected, the registered variant with that name is applied first.
pub fn apply_active_stylesheet_ron(world: &mut World, ron_text: &str) -> io::Result<()> {
    let sheet = parse_stylesheet_ron(ron_text)?;
    apply_stylesheet_default_variant(world, &sheet)?;
    apply_active_stylesheet(world, sheet);
    Ok(())
}

/// Parse a multi-variant stylesheet bundle RON into registered variants.
pub fn parse_stylesheet_variants_ron(ron_text: &str) -> io::Result<RegisteredStyleVariants> {
    stylesheet_variants_from_ron_bytes(ron_text.as_bytes())
}

/// Parse + register named stylesheet variants in the world.
pub fn register_stylesheet_variants_ron(world: &mut World, ron_text: &str) -> io::Result<()> {
    let variants = parse_stylesheet_variants_ron(ron_text)?;
    world.insert_resource(variants);
    Ok(())
}

fn apply_registered_style_variant_by_name(world: &mut World, variant_name: &str) -> io::Result<()> {
    let variants = world
        .get_resource::<RegisteredStyleVariants>()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "style variants are not registered in this world",
            )
        })?
        .clone();

    let selected = variants
        .variants
        .get(variant_name)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("style variant `{variant_name}` is not registered"),
            )
        })?;

    apply_base_stylesheet(world, selected);
    Ok(())
}

/// Set desired active style variant by name.
///
/// This only updates desired state. Call [`apply_active_style_variant`] to
/// apply immediately, or rely on [`sync_active_style_variant`] in the app loop.
pub fn set_active_style_variant_by_name(world: &mut World, variant_name: &str) {
    world.insert_resource(ActiveStyleVariant(Some(variant_name.to_string())));
}

/// Override the active theme's [`WINDOW_BACKDROP_TOKEN`] and immediately
/// reapply the current style variant.
///
/// The override persists across light/dark variant changes until cleared with
/// [`clear_theme_backdrop_material_override`].
pub fn set_theme_backdrop_material(world: &mut World, material: WindowBackdropMaterial) {
    world.insert_resource(ThemeBackdropOverride(Some(material)));
    if world
        .get_resource::<ActiveStyleVariant>()
        .is_some_and(|active| active.0.is_some())
        && let Err(error) = apply_active_style_variant(world)
    {
        tracing::warn!(
            material = material.theme_name(),
            "failed to apply theme backdrop override: {error}"
        );
    }
    reapply_active_backdrop_stylesheet(world);
}

/// Clear an application backdrop override and restore the active variant's
/// [`WINDOW_BACKDROP_TOKEN`] value.
pub fn clear_theme_backdrop_material_override(world: &mut World) {
    world.insert_resource(ThemeBackdropOverride(None));
    if world
        .get_resource::<ActiveStyleVariant>()
        .is_some_and(|active| active.0.is_some())
        && let Err(error) = apply_active_style_variant(world)
    {
        tracing::warn!("failed to restore theme backdrop material: {error}");
    }
    reapply_active_backdrop_stylesheet(world);
}

fn reapply_active_backdrop_stylesheet(world: &mut World) {
    let active = world
        .get_resource::<ActiveStyleSheet>()
        .map(|active| active.0.clone())
        .filter(|active| active.backdrop.is_some());
    if let Some(active) = active {
        apply_active_stylesheet_impl(world, active, false);
    }
}

/// Set desired active style variant to the registered default variant.
pub fn set_active_style_variant_to_registered_default(world: &mut World) -> io::Result<()> {
    let default_variant = world
        .get_resource::<RegisteredStyleVariants>()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "style variants are not registered in this world",
            )
        })?
        .default_variant
        .clone();

    set_active_style_variant_by_name(world, default_variant.as_str());
    Ok(())
}

/// Apply the currently desired active style variant immediately.
pub fn apply_active_style_variant(world: &mut World) -> io::Result<()> {
    let desired_variant = world
        .get_resource::<ActiveStyleVariant>()
        .and_then(|active| active.0.clone())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "active style variant is not set")
        })?;

    apply_registered_style_variant_by_name(world, desired_variant.as_str())?;
    world.insert_resource(AppliedStyleVariant(Some(desired_variant)));
    Ok(())
}

/// Sync desired active style variant into the runtime stylesheet.
///
/// This system is safe to run every frame and only reapplies when desired and
/// applied variants differ.
pub fn sync_active_style_variant(world: &mut World) {
    let desired_variant = world
        .get_resource::<ActiveStyleVariant>()
        .and_then(|active| active.0.clone());
    let Some(desired_variant) = desired_variant else {
        return;
    };

    let applied_variant = world
        .get_resource::<AppliedStyleVariant>()
        .and_then(|applied| applied.0.clone());

    if applied_variant.as_deref() == Some(desired_variant.as_str()) {
        return;
    }

    if let Err(error) = apply_active_style_variant(world) {
        tracing::warn!(
            desired_variant,
            "failed to apply active style variant automatically: {error}"
        );
    }
}

/// Register all embedded Fluent variants from the bundled multi-variant theme file.
pub fn register_embedded_fluent_theme_variants(world: &mut World) -> io::Result<()> {
    register_stylesheet_variants_ron(world, BUILTIN_FLUENT_THEME_RON)
}

/// Merge a baseline stylesheet RON into the base + runtime tiers.
///
/// Runtime insertion preserves active stylesheet precedence by skipping selectors
/// and tokens currently owned by the active stylesheet asset.
pub fn merge_base_stylesheet_ron(world: &mut World, ron_text: &str) -> io::Result<()> {
    let parsed = parse_stylesheet_ron(ron_text)?;
    world.init_resource::<BaseStyleSheet>();

    let mut merged = world.resource::<BaseStyleSheet>().0.clone();
    merge_sheet_inplace(&mut merged, parsed);
    apply_base_stylesheet(world, merged);

    Ok(())
}

/// Ensure the active stylesheet asset handle is loaded from the configured path.
pub fn ensure_active_stylesheet_asset_handle(world: &mut World) {
    let path = world
        .get_resource::<ActiveStyleSheetAsset>()
        .and_then(|active| active.path.clone());

    let Some(path) = path else {
        return;
    };

    let has_handle = world
        .get_resource::<ActiveStyleSheetAsset>()
        .is_some_and(|active| active.handle.is_some());
    if has_handle {
        return;
    }

    let Some(asset_server) = world.get_resource::<AssetServer>() else {
        return;
    };

    let handle = asset_server.load::<StyleSheet>(path);
    world.resource_mut::<ActiveStyleSheetAsset>().handle = Some(handle);
}

/// Apply a fully parsed active stylesheet to the runtime style tiers.
///
/// This mirrors the active-tier merge behavior used by asset-based stylesheet
/// loading and clears any active asset path/handle so future sync relies on this
/// in-memory source until another active stylesheet source is selected.
fn apply_active_stylesheet_impl(
    world: &mut World,
    mut loaded_stylesheet: StyleSheet,
    clear_asset_binding: bool,
) {
    world.init_resource::<ActiveStyleSheet>();
    world.init_resource::<ActiveStyleSheetSelectors>();
    world.init_resource::<ActiveStyleSheetTokenNames>();
    world.init_resource::<StyleSheet>();
    world.init_resource::<ActiveStyleSheetAsset>();

    let source_stylesheet = loaded_stylesheet.clone();
    if loaded_stylesheet.backdrop.is_some() {
        apply_theme_backdrop_override(world, &mut loaded_stylesheet);
        apply_selected_backdrop_style(&mut loaded_stylesheet);
    }
    world.resource_mut::<ActiveStyleSheet>().0 = source_stylesheet;

    let incoming_selectors = loaded_stylesheet
        .rules
        .iter()
        .map(|rule| rule.selector.clone())
        .collect::<HashSet<_>>();
    let incoming_token_names = loaded_stylesheet
        .tokens
        .keys()
        .cloned()
        .collect::<HashSet<_>>();

    let previous_asset_selectors = world
        .get_resource::<ActiveStyleSheetSelectors>()
        .map(|selectors| selectors.0.clone())
        .unwrap_or_default();
    let previous_asset_token_names = world
        .get_resource::<ActiveStyleSheetTokenNames>()
        .map(|names| names.0.clone())
        .unwrap_or_default();
    let base_font_family = world
        .get_resource::<BaseStyleSheet>()
        .and_then(|base| base.0.font_family.clone());
    let base_backdrop = world
        .get_resource::<BaseStyleSheet>()
        .and_then(|base| base.0.backdrop.clone());

    let mut runtime_sheet = world.resource_mut::<StyleSheet>();
    runtime_sheet
        .rules
        .retain(|rule| !previous_asset_selectors.contains(&rule.selector));
    runtime_sheet
        .tokens
        .retain(|name, _| !previous_asset_token_names.contains(name));
    runtime_sheet.font_family = loaded_stylesheet.font_family.clone().or(base_font_family);
    runtime_sheet.backdrop = loaded_stylesheet.backdrop.clone().or(base_backdrop);
    runtime_sheet.rules.extend(loaded_stylesheet.rules);
    runtime_sheet.tokens.extend(loaded_stylesheet.tokens);

    world.resource_mut::<ActiveStyleSheetSelectors>().0 = incoming_selectors;
    world.resource_mut::<ActiveStyleSheetTokenNames>().0 = incoming_token_names;

    if clear_asset_binding {
        let mut active_asset = world.resource_mut::<ActiveStyleSheetAsset>();
        active_asset.path = None;
        active_asset.handle = None;
    }
}

fn apply_stylesheet_default_variant(
    world: &mut World,
    loaded_stylesheet: &StyleSheet,
) -> io::Result<()> {
    let Some(default_variant) = loaded_stylesheet.default_variant.clone() else {
        return Ok(());
    };

    let has_active_variant = world
        .get_resource::<ActiveStyleVariant>()
        .is_some_and(|active| active.0.is_some());
    if has_active_variant {
        return Ok(());
    }

    apply_registered_style_variant_by_name(world, default_variant.as_str())?;
    world.insert_resource(ActiveStyleVariant(Some(default_variant.clone())));
    world.insert_resource(AppliedStyleVariant(Some(default_variant)));
    Ok(())
}

pub fn apply_active_stylesheet(world: &mut World, loaded_stylesheet: StyleSheet) {
    if let Err(error) = apply_stylesheet_default_variant(world, &loaded_stylesheet) {
        tracing::warn!(
            default_variant = loaded_stylesheet.default_variant.as_deref(),
            "failed to apply active stylesheet default variant: {error}"
        );
    }
    apply_active_stylesheet_impl(world, loaded_stylesheet, true);
}

/// Apply loaded stylesheet asset updates to the live [`StyleSheet`] resource.
pub fn sync_stylesheet_asset_events(world: &mut World) {
    let active_handle_id = world
        .get_resource::<ActiveStyleSheetAsset>()
        .and_then(|active| active.handle.as_ref())
        .map(|handle| handle.id());

    let Some(active_handle_id) = active_handle_id else {
        return;
    };

    if !world.contains_resource::<Messages<AssetEvent<StyleSheet>>>() {
        return;
    }

    let mut should_refresh = false;
    world.resource_scope(|world, mut cursor: Mut<StyleAssetEventCursor>| {
        let messages = world.resource::<Messages<AssetEvent<StyleSheet>>>();
        for event in cursor.0.read(messages) {
            match event {
                AssetEvent::Added { id }
                | AssetEvent::Modified { id }
                | AssetEvent::LoadedWithDependencies { id }
                    if *id == active_handle_id =>
                {
                    should_refresh = true;
                }
                _ => {}
            }
        }
    });

    if !should_refresh {
        return;
    }

    let Some(active_handle) = world
        .get_resource::<ActiveStyleSheetAsset>()
        .and_then(|active| active.handle.clone())
    else {
        return;
    };

    let Some(loaded_stylesheet) = world
        .get_resource::<Assets<StyleSheet>>()
        .and_then(|assets| assets.get(&active_handle))
        .cloned()
    else {
        return;
    };

    if let Err(error) = apply_stylesheet_default_variant(world, &loaded_stylesheet) {
        tracing::warn!(
            default_variant = loaded_stylesheet.default_variant.as_deref(),
            "failed to apply active stylesheet default variant from asset: {error}"
        );
    }
    apply_active_stylesheet_impl(world, loaded_stylesheet, false);
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResolvedLayoutStyle {
    pub padding: f64,
    pub gap: f64,
    pub corner_radius: f64,
    pub border_width: f64,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub scale: f64,
    pub flex_grow: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedColorStyle {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    /// Solid border color used for color transitions and solid-only paint paths.
    ///
    /// When [`Self::border_brush`] is a gradient, this may be `None`.
    pub border: Option<Color>,
    /// Authoritative border paint (solid or gradient). Prefer
    /// [`Self::resolved_border_brush`] when applying styles to widgets.
    pub border_brush: Option<BorderBrush>,
}

impl ResolvedColorStyle {
    /// Effective border brush for painting.
    ///
    /// Prefers an explicit [`Self::border_brush`], otherwise promotes
    /// [`Self::border`] to a solid brush.
    #[must_use]
    pub fn resolved_border_brush(&self) -> BorderBrush {
        self.border_brush
            .clone()
            .or_else(|| self.border.map(BorderBrush::Color))
            .unwrap_or(BorderBrush::Color(Color::TRANSPARENT))
    }

    /// Solid color for legacy masonry `BorderColor` paint paths.
    ///
    /// Gradients fall back to transparent (generic containers cannot paint them).
    #[must_use]
    pub fn solid_border_color(&self) -> Color {
        self.border
            .or_else(|| {
                self.border_brush
                    .as_ref()
                    .and_then(BorderBrush::as_solid_color)
            })
            .unwrap_or(Color::TRANSPARENT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedTextStyle {
    pub size: f32,
    pub text_align: TextAlign,
    pub weight: f32,
    pub line_height: f32,
}

impl Default for ResolvedTextStyle {
    fn default() -> Self {
        Self {
            size: DEFAULT_TEXT_SIZE,
            text_align: TextAlign::Start,
            weight: 400.0,
            line_height: 1.35,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedStyle {
    pub layout: ResolvedLayoutStyle,
    pub colors: ResolvedColorStyle,
    pub text: ResolvedTextStyle,
    pub font_family: Option<Vec<String>>,
    pub box_shadow: Option<BoxShadow>,
    pub transition: Option<StyleTransition>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StylePseudoState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
}

impl StylePseudoState {
    #[must_use]
    pub const fn hovered() -> Self {
        Self {
            hovered: true,
            pressed: false,
            focused: false,
        }
    }

    #[must_use]
    pub const fn pressed() -> Self {
        Self {
            hovered: true,
            pressed: true,
            focused: false,
        }
    }

    #[must_use]
    pub const fn focused() -> Self {
        Self {
            hovered: false,
            pressed: false,
            focused: true,
        }
    }
}

/// Structural interaction events emitted by ECS-backed widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiInteractionEvent {
    PointerEntered,
    PointerLeft,
    PointerPressed,
    PointerReleased,
}

fn merge_layout_values(dst: &mut LayoutStyleValue, src: &LayoutStyleValue) {
    if src.padding.is_some() {
        dst.padding = src.padding.clone();
    }
    if src.gap.is_some() {
        dst.gap = src.gap.clone();
    }
    if src.corner_radius.is_some() {
        dst.corner_radius = src.corner_radius.clone();
    }
    if src.border_width.is_some() {
        dst.border_width = src.border_width.clone();
    }
    if src.justify_content.is_some() {
        dst.justify_content = src.justify_content.clone();
    }
    if src.align_items.is_some() {
        dst.align_items = src.align_items.clone();
    }
    if src.scale.is_some() {
        dst.scale = src.scale.clone();
    }
    if src.flex_grow.is_some() {
        dst.flex_grow = src.flex_grow.clone();
    }
}

fn merge_colors_values(dst: &mut ColorStyleValue, src: &ColorStyleValue) {
    if src.bg.is_some() {
        dst.bg = src.bg.clone();
    }
    if src.text.is_some() {
        dst.text = src.text.clone();
    }
    // `border` and `border_brush` share one cascade slot for "border paint".
    // A later solid `border` clears any previous brush so hover/pressed solid
    // borders correctly override a base elevation gradient.
    if src.border.is_some() {
        dst.border = src.border.clone();
        dst.border_brush = None;
    }
    if src.border_brush.is_some() {
        dst.border_brush = src.border_brush.clone();
    }
    if src.hover_bg.is_some() {
        dst.hover_bg = src.hover_bg.clone();
    }
    if src.hover_text.is_some() {
        dst.hover_text = src.hover_text.clone();
    }
    if src.hover_border.is_some() {
        dst.hover_border = src.hover_border.clone();
    }
    if src.pressed_bg.is_some() {
        dst.pressed_bg = src.pressed_bg.clone();
    }
    if src.pressed_text.is_some() {
        dst.pressed_text = src.pressed_text.clone();
    }
    if src.pressed_border.is_some() {
        dst.pressed_border = src.pressed_border.clone();
    }
}

fn merge_text_values(dst: &mut TextStyleValue, src: &TextStyleValue) {
    if src.size.is_some() {
        dst.size = src.size.clone();
    }
    if src.text_align.is_some() {
        dst.text_align = src.text_align.clone();
    }
    if src.weight.is_some() {
        dst.weight = src.weight.clone();
    }
    if src.line_height.is_some() {
        dst.line_height = src.line_height.clone();
    }
}

fn merge_value_setter(dst: &mut StyleSetterValue, setter: &StyleSetterValue) {
    merge_layout_values(&mut dst.layout, &setter.layout);
    merge_colors_values(&mut dst.colors, &setter.colors);
    merge_text_values(&mut dst.text, &setter.text);
    if setter.font_family.is_some() {
        dst.font_family = setter.font_family.clone();
    }
    if setter.box_shadow.is_some() {
        dst.box_shadow = setter.box_shadow.clone();
    }
    if setter.transition.is_some() {
        dst.transition = setter.transition.clone();
    }
}

fn merge_inline_layout_values(dst: &mut LayoutStyleValue, src: &LayoutStyle) {
    if let Some(padding) = src.padding {
        dst.padding = Some(StyleValue::value(padding));
    }
    if let Some(gap) = src.gap {
        dst.gap = Some(StyleValue::value(gap));
    }
    if let Some(corner_radius) = src.corner_radius {
        dst.corner_radius = Some(StyleValue::value(corner_radius));
    }
    if let Some(border_width) = src.border_width {
        dst.border_width = Some(StyleValue::value(border_width));
    }
    if let Some(justify_content) = src.justify_content {
        dst.justify_content = Some(StyleValue::value(justify_content));
    }
    if let Some(align_items) = src.align_items {
        dst.align_items = Some(StyleValue::value(align_items));
    }
    if let Some(scale) = src.scale {
        dst.scale = Some(StyleValue::value(scale));
    }
    if let Some(flex_grow) = src.flex_grow {
        dst.flex_grow = Some(StyleValue::value(flex_grow));
    }
}

fn merge_inline_color_values(dst: &mut ColorStyleValue, src: &ColorStyle) {
    if let Some(bg) = src.bg {
        dst.bg = Some(StyleValue::value(bg));
    }
    if let Some(text) = src.text {
        dst.text = Some(StyleValue::value(text));
    }
    if let Some(border) = src.border {
        dst.border = Some(StyleValue::value(border));
    }
    if let Some(hover_bg) = src.hover_bg {
        dst.hover_bg = Some(StyleValue::value(hover_bg));
    }
    if let Some(hover_text) = src.hover_text {
        dst.hover_text = Some(StyleValue::value(hover_text));
    }
    if let Some(hover_border) = src.hover_border {
        dst.hover_border = Some(StyleValue::value(hover_border));
    }
    if let Some(pressed_bg) = src.pressed_bg {
        dst.pressed_bg = Some(StyleValue::value(pressed_bg));
    }
    if let Some(pressed_text) = src.pressed_text {
        dst.pressed_text = Some(StyleValue::value(pressed_text));
    }
    if let Some(pressed_border) = src.pressed_border {
        dst.pressed_border = Some(StyleValue::value(pressed_border));
    }
}

fn merge_inline_text_values(dst: &mut TextStyleValue, src: &TextStyle) {
    if let Some(size) = src.size {
        dst.size = Some(StyleValue::value(size));
    }
    if let Some(text_align) = src.text_align {
        dst.text_align = Some(StyleValue::value(text_align));
    }
    if let Some(weight) = src.weight {
        dst.weight = Some(StyleValue::value(weight));
    }
    if let Some(line_height) = src.line_height {
        dst.line_height = Some(StyleValue::value(line_height));
    }
}

fn component_matches_type(world: &World, entity: Entity, component_id: ComponentId) -> bool {
    world
        .get_entity(entity)
        .is_ok_and(|entity_ref| entity_ref.contains_id(component_id))
}

fn entity_has_matching_ancestor(
    world: &World,
    entity: Entity,
    ancestor_selector: &Selector,
) -> bool {
    let mut current = entity;

    while let Some(child_of) = world.get::<ChildOf>(current) {
        let parent = child_of.parent();
        if selector_matches_entity(world, parent, ancestor_selector) {
            return true;
        }
        current = parent;
    }

    false
}

fn selector_matches_entity(world: &World, entity: Entity, selector: &Selector) -> bool {
    match selector {
        Selector::Type(type_id) => world
            .components()
            .get_id(*type_id)
            .is_some_and(|component_id| component_matches_type(world, entity, component_id)),
        Selector::TypeName(name) => world
            .get_resource::<StyleTypeRegistry>()
            .and_then(|registry| registry.resolve(name))
            .and_then(|type_id| world.components().get_id(type_id))
            .is_some_and(|component_id| component_matches_type(world, entity, component_id)),
        Selector::Class(name) => world
            .get::<StyleClass>(entity)
            .is_some_and(|style_class| style_class.0.iter().any(|class| class == name)),
        Selector::PseudoClass(PseudoClass::Hovered) => world
            .get::<InteractionState>(entity)
            .is_some_and(|state| state.hovered),
        Selector::PseudoClass(PseudoClass::Pressed) => world
            .get::<InteractionState>(entity)
            .is_some_and(|state| state.pressed),
        Selector::PseudoClass(PseudoClass::Focused) => world
            .get::<InteractionState>(entity)
            .is_some_and(|state| state.focused),
        Selector::And(selectors) => selectors
            .iter()
            .all(|selector| selector_matches_entity(world, entity, selector)),
        Selector::Descendant {
            ancestor,
            descendant,
        } => {
            selector_matches_entity(world, entity, descendant)
                && entity_has_matching_ancestor(world, entity, ancestor)
        }
    }
}

fn selector_matches_class_context(
    world: &World,
    entity: Option<Entity>,
    pseudo_state: Option<StylePseudoState>,
    selector: &Selector,
    has_class: &impl Fn(&str) -> bool,
) -> bool {
    match selector {
        Selector::Type(type_id) => entity.is_some_and(|entity| {
            world
                .components()
                .get_id(*type_id)
                .is_some_and(|component_id| component_matches_type(world, entity, component_id))
        }),
        Selector::TypeName(name) => entity.is_some_and(|entity| {
            world
                .get_resource::<StyleTypeRegistry>()
                .and_then(|registry| registry.resolve(name))
                .and_then(|type_id| world.components().get_id(type_id))
                .is_some_and(|component_id| component_matches_type(world, entity, component_id))
        }),
        Selector::Class(name) => has_class(name),
        Selector::PseudoClass(PseudoClass::Hovered) => pseudo_state
            .map(|state| state.hovered)
            .or_else(|| {
                entity
                    .and_then(|entity| world.get::<InteractionState>(entity))
                    .map(|state| state.hovered)
            })
            .unwrap_or(false),
        Selector::PseudoClass(PseudoClass::Pressed) => pseudo_state
            .map(|state| state.pressed)
            .or_else(|| {
                entity
                    .and_then(|entity| world.get::<InteractionState>(entity))
                    .map(|state| state.pressed)
            })
            .unwrap_or(false),
        Selector::PseudoClass(PseudoClass::Focused) => pseudo_state
            .map(|state| state.focused)
            .or_else(|| {
                entity
                    .and_then(|entity| world.get::<InteractionState>(entity))
                    .map(|state| state.focused)
            })
            .unwrap_or(false),
        Selector::And(selectors) => selectors.iter().all(|selector| {
            selector_matches_class_context(world, entity, pseudo_state, selector, has_class)
        }),
        Selector::Descendant {
            ancestor,
            descendant,
        } => {
            let Some(entity) = entity else {
                return false;
            };

            selector_matches_class_context(world, Some(entity), pseudo_state, descendant, has_class)
                && entity_has_matching_ancestor(world, entity, ancestor)
        }
    }
}

fn merged_from_class_names<'a>(
    world: &World,
    entity: Option<Entity>,
    class_names: impl IntoIterator<Item = &'a str>,
) -> StyleSetterValue {
    merged_from_class_names_with_state(world, entity, None, class_names)
}

fn merged_from_class_names_with_state<'a>(
    world: &World,
    entity: Option<Entity>,
    pseudo_state: Option<StylePseudoState>,
    class_names: impl IntoIterator<Item = &'a str>,
) -> StyleSetterValue {
    let mut merged = StyleSetterValue::default();
    let Some(sheet) = world.get_resource::<StyleSheet>() else {
        return merged;
    };

    let class_set = class_names.into_iter().collect::<HashSet<_>>();
    let has_class = |class_name: &str| class_set.contains(class_name);

    for rule in &sheet.rules {
        if selector_matches_class_context(world, entity, pseudo_state, &rule.selector, &has_class) {
            merge_value_setter(&mut merged, &rule.setter);
        }
    }

    merged
}

fn merged_for_entity(world: &World, entity: Entity) -> (StyleSetterValue, bool) {
    let mut merged = StyleSetterValue::default();
    let mut matched_rule = false;

    if let Some(sheet) = world.get_resource::<StyleSheet>() {
        for rule in &sheet.rules {
            if selector_matches_entity(world, entity, &rule.selector) {
                merge_value_setter(&mut merged, &rule.setter);
                matched_rule = true;
            }
        }
    }

    if let Some(layout) = world.get::<LayoutStyle>(entity) {
        merge_inline_layout_values(&mut merged.layout, layout);
    }
    if let Some(colors) = world.get::<ColorStyle>(entity) {
        merge_inline_color_values(&mut merged.colors, colors);
    }
    if let Some(text) = world.get::<TextStyle>(entity) {
        merge_inline_text_values(&mut merged.text, text);
    }
    if let Some(transition) = world.get::<StyleTransition>(entity) {
        merged.transition = Some(StyleValue::value(*transition));
    }

    // Consolidated inline overrides (preferred).
    if let Some(inline) = world.get::<InlineStyle>(entity) {
        merge_inline_layout_values(&mut merged.layout, &inline.layout);
        merge_inline_color_values(&mut merged.colors, &inline.colors);
        merge_inline_text_values(&mut merged.text, &inline.text);
        if let Some(transition) = inline.transition {
            merged.transition = Some(StyleValue::value(transition));
        }
    }

    (merged, matched_rule)
}

fn target_colors_for_state(
    colors: &ColorStyle,
    hovered: bool,
    pressed: bool,
) -> ResolvedColorStyle {
    let mut resolved = ResolvedColorStyle {
        bg: colors.bg,
        text: colors.text,
        border: colors.border,
        border_brush: None,
    };

    if hovered {
        if colors.hover_bg.is_some() {
            resolved.bg = colors.hover_bg;
        }
        if colors.hover_text.is_some() {
            resolved.text = colors.hover_text;
        }
        if colors.hover_border.is_some() {
            resolved.border = colors.hover_border;
        }
    }

    if pressed {
        if colors.pressed_bg.is_some() {
            resolved.bg = colors.pressed_bg;
        }
        if colors.pressed_text.is_some() {
            resolved.text = colors.pressed_text;
        }
        if colors.pressed_border.is_some() {
            resolved.border = colors.pressed_border;
        }
    }

    resolved
}

fn target_colors(world: &World, entity: Entity, colors: &ColorStyle) -> ResolvedColorStyle {
    let (hovered, pressed) = world
        .get::<InteractionState>(entity)
        .map(|state| (state.hovered, state.pressed))
        .unwrap_or((false, false));

    target_colors_for_state(colors, hovered, pressed)
}

fn to_resolved_layout(layout: &LayoutStyle) -> ResolvedLayoutStyle {
    ResolvedLayoutStyle {
        padding: layout.padding.unwrap_or(0.0),
        gap: layout.gap.unwrap_or(0.0),
        corner_radius: layout.corner_radius.unwrap_or(0.0),
        border_width: layout.border_width.unwrap_or(0.0),
        justify_content: layout.justify_content.unwrap_or_default(),
        align_items: layout.align_items.unwrap_or_default(),
        scale: layout.scale.unwrap_or(1.0),
        flex_grow: layout.flex_grow.unwrap_or(0.0),
    }
}

fn to_resolved_text(text: &TextStyle) -> ResolvedTextStyle {
    ResolvedTextStyle {
        size: text.size.unwrap_or(DEFAULT_TEXT_SIZE),
        text_align: text.text_align.unwrap_or_default(),
        weight: text.weight.unwrap_or(400.0),
        line_height: text.line_height.unwrap_or(1.35),
    }
}

fn warn_missing_or_invalid_token(token: &str, field: &str, expected: &str) {
    tracing::warn!(
        token,
        field,
        expected,
        "style token missing or has incompatible type; dropping declaration"
    );
}

fn resolve_f64_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<f64>,
    field: &str,
) -> Option<f64> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Float(value)) => Some(*value),
            _ => {
                warn_missing_or_invalid_token(token, field, "Float");
                None
            }
        },
    }
}

fn resolve_f32_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<f32>,
    field: &str,
) -> Option<f32> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Float(value)) => Some(*value as f32),
            _ => {
                warn_missing_or_invalid_token(token, field, "Float");
                None
            }
        },
    }
}

fn resolve_color_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<Color>,
    field: &str,
) -> Option<Color> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Color(value)) => Some(*value),
            _ => {
                warn_missing_or_invalid_token(token, field, "Color");
                None
            }
        },
    }
}

fn resolve_font_family_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<Vec<String>>,
    field: &str,
) -> Option<Vec<String>> {
    match value {
        StyleValue::Value(value) => Some(value.clone()),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::FontFamily(value)) => Some(value.clone()),
            _ => {
                warn_missing_or_invalid_token(token, field, "FontFamily");
                None
            }
        },
    }
}

fn resolve_stylesheet_font_family(sheet: &StyleSheet) -> Option<Vec<String>> {
    sheet
        .font_family
        .as_ref()
        .and_then(|value| resolve_font_family_value(&sheet.tokens, value, "font_family"))
}

pub(crate) fn theme_default_font_family(world: &World) -> Option<Vec<String>> {
    world
        .get_resource::<StyleSheet>()
        .and_then(resolve_stylesheet_font_family)
}

fn resolve_box_shadow_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<BoxShadow>,
    field: &str,
) -> Option<BoxShadow> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::BoxShadow(value)) => Some(*value),
            _ => {
                warn_missing_or_invalid_token(token, field, "BoxShadow");
                None
            }
        },
    }
}

fn resolve_transition_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<StyleTransition>,
    field: &str,
) -> Option<StyleTransition> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Transition(value)) => Some(*value),
            Some(TokenValue::Float(value)) => Some(StyleTransition {
                duration: *value as f32,
                easing: None,
            }),
            _ => {
                warn_missing_or_invalid_token(token, field, "Transition|Float");
                None
            }
        },
    }
}

fn resolve_enum_value<T: Copy>(
    _tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<T>,
    _field: &str,
) -> Option<T> {
    match value {
        StyleValue::Value(value) => Some(*value),
        StyleValue::Var(_token) => {
            tracing::warn!(
                field = _field,
                "style enum values currently only support literal values; token reference ignored"
            );
            None
        }
    }
}

fn resolve_layout_style(
    layout: &LayoutStyleValue,
    tokens: &HashMap<String, TokenValue>,
) -> LayoutStyle {
    LayoutStyle {
        padding: layout
            .padding
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.padding")),
        gap: layout
            .gap
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.gap")),
        corner_radius: layout
            .corner_radius
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.corner_radius")),
        border_width: layout
            .border_width
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.border_width")),
        justify_content: layout
            .justify_content
            .as_ref()
            .and_then(|value| resolve_enum_value(tokens, value, "layout.justify_content")),
        align_items: layout
            .align_items
            .as_ref()
            .and_then(|value| resolve_enum_value(tokens, value, "layout.align_items")),
        scale: layout
            .scale
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.scale")),
        flex_grow: layout
            .flex_grow
            .as_ref()
            .and_then(|value| resolve_f64_value(tokens, value, "layout.flex_grow")),
    }
}

fn resolve_color_style(
    colors: &ColorStyleValue,
    tokens: &HashMap<String, TokenValue>,
) -> ColorStyle {
    ColorStyle {
        bg: colors
            .bg
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.bg")),
        text: colors
            .text
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.text")),
        // Solid border is resolved with `border_brush` in `resolve_border_paint`
        // so gradient tokens used as `border: Var(...)` do not warn here.
        border: None,
        hover_bg: colors
            .hover_bg
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.hover_bg")),
        hover_text: colors
            .hover_text
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.hover_text")),
        hover_border: colors
            .hover_border
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.hover_border")),
        pressed_bg: colors
            .pressed_bg
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.pressed_bg")),
        pressed_text: colors
            .pressed_text
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.pressed_text")),
        pressed_border: colors
            .pressed_border
            .as_ref()
            .and_then(|value| resolve_color_value(tokens, value, "colors.pressed_border")),
    }
}

fn resolve_text_style(text: &TextStyleValue, tokens: &HashMap<String, TokenValue>) -> TextStyle {
    TextStyle {
        size: text
            .size
            .as_ref()
            .and_then(|value| resolve_f32_value(tokens, value, "text.size")),
        text_align: text
            .text_align
            .as_ref()
            .and_then(|value| resolve_enum_value(tokens, value, "text.text_align")),
        weight: text
            .weight
            .as_ref()
            .and_then(|value| resolve_f32_value(tokens, value, "text.weight")),
        line_height: text
            .line_height
            .as_ref()
            .and_then(|value| resolve_f32_value(tokens, value, "text.line_height")),
    }
}

fn resolve_border_brush_value(
    tokens: &HashMap<String, TokenValue>,
    value: &StyleValue<BorderBrush>,
    field: &str,
) -> Option<BorderBrush> {
    match value {
        StyleValue::Value(brush) => Some(brush.clone()),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::BorderBrush(brush)) => Some(brush.clone()),
            Some(TokenValue::Color(color)) => Some(BorderBrush::Color(*color)),
            _ => {
                warn_missing_or_invalid_token(token, field, "BorderBrush");
                None
            }
        },
    }
}

/// Resolves the shared border-paint slot from solid `border` and/or `border_brush`.
///
/// `border: Var("border-elevation")` works when the token is a brush or color.
/// Explicit `border_brush` wins over a solid `border` from the same rule.
fn resolve_border_paint(
    colors: &ColorStyleValue,
    tokens: &HashMap<String, TokenValue>,
) -> (Option<Color>, Option<BorderBrush>) {
    let solid_from_border = colors.border.as_ref().and_then(|value| match value {
        StyleValue::Value(color) => Some(*color),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Color(color)) => Some(*color),
            Some(TokenValue::BorderBrush(BorderBrush::Color(color))) => Some(*color),
            // Gradient (or missing) handled via brush path below.
            Some(TokenValue::BorderBrush(BorderBrush::Gradient(_))) => None,
            _ => {
                // Only warn when the token is not a brush either.
                if !matches!(tokens.get(token), Some(TokenValue::BorderBrush(_))) {
                    warn_missing_or_invalid_token(token, "colors.border", "Color");
                }
                None
            }
        },
    });

    let brush_from_border = colors.border.as_ref().and_then(|value| match value {
        StyleValue::Value(color) => Some(BorderBrush::Color(*color)),
        StyleValue::Var(token) => match tokens.get(token) {
            Some(TokenValue::Color(color)) => Some(BorderBrush::Color(*color)),
            Some(TokenValue::BorderBrush(brush)) => Some(brush.clone()),
            _ => None,
        },
    });

    let brush_from_field = colors
        .border_brush
        .as_ref()
        .and_then(|value| resolve_border_brush_value(tokens, value, "colors.border_brush"));

    let brush = brush_from_field.or(brush_from_border);
    let solid = solid_from_border.or_else(|| brush.as_ref().and_then(BorderBrush::as_solid_color));

    (solid, brush)
}

fn resolve_setter_values(
    setter: &StyleSetterValue,
    tokens: &HashMap<String, TokenValue>,
) -> StyleSetter {
    let mut colors = resolve_color_style(&setter.colors, tokens);
    let (solid_border, border_brush) = resolve_border_paint(&setter.colors, tokens);
    // Override solid border with the paint-slot resolution so gradient tokens
    // used as `border: Var(...)` do not leave a stale color behind.
    colors.border = solid_border;

    StyleSetter {
        layout: resolve_layout_style(&setter.layout, tokens),
        colors,
        border_brush,
        text: resolve_text_style(&setter.text, tokens),
        font_family: setter
            .font_family
            .as_ref()
            .and_then(|value| resolve_font_family_value(tokens, value, "font_family")),
        box_shadow: setter
            .box_shadow
            .as_ref()
            .and_then(|value| resolve_box_shadow_value(tokens, value, "box_shadow")),
        transition: setter
            .transition
            .as_ref()
            .and_then(|value| resolve_transition_value(tokens, value, "transition")),
    }
}

fn has_any_style_source(world: &World, entity: Entity, matched_rule: bool) -> bool {
    matched_rule
        || world.get::<StyleClass>(entity).is_some()
        || world.get::<InlineStyle>(entity).is_some()
        || world.get::<LayoutStyle>(entity).is_some()
        || world.get::<ColorStyle>(entity).is_some()
        || world.get::<TextStyle>(entity).is_some()
        || world.get::<StyleTransition>(entity).is_some()
}

fn resolved_from_merged(
    world: &World,
    entity: Entity,
    merged: &StyleSetterValue,
    sheet: Option<&StyleSheet>,
    include_current_override: bool,
) -> ResolvedStyle {
    let empty_tokens = HashMap::new();
    let tokens = sheet.map(|sheet| &sheet.tokens).unwrap_or(&empty_tokens);
    let default_font_family = sheet.and_then(resolve_stylesheet_font_family);
    let merged = resolve_setter_values(merged, tokens);
    let mut colors = target_colors(world, entity, &merged.colors);

    // border_brush comes from the shared border-paint cascade slot.
    colors.border_brush = merged.border_brush.clone();
    // Keep solid color in sync when only a solid brush was authored.
    if colors.border.is_none() {
        colors.border = colors
            .border_brush
            .as_ref()
            .and_then(BorderBrush::as_solid_color);
    }

    if include_current_override && let Some(current) = world.get::<CurrentColorStyle>(entity) {
        if current.bg.is_some() {
            colors.bg = current.bg;
        }
        if current.text.is_some() {
            colors.text = current.text;
        }
        if current.border.is_some() {
            // Animated solid border becomes the authoritative brush.
            colors.border = current.border;
            colors.border_brush = current.border.map(BorderBrush::Color);
        }
    }

    let mut layout = to_resolved_layout(&merged.layout);
    if include_current_override && let Some(current) = world.get::<CurrentColorStyle>(entity) {
        layout.scale = current.scale;
    }

    ResolvedStyle {
        layout,
        colors,
        text: to_resolved_text(&merged.text),
        font_family: merged.font_family.clone().or(default_font_family),
        box_shadow: merged.box_shadow,
        transition: merged.transition,
    }
}

fn compute_resolved_style(world: &World, entity: Entity) -> Option<ResolvedStyle> {
    let (merged, matched_rule) = merged_for_entity(world, entity);
    if !has_any_style_source(world, entity, matched_rule) {
        return None;
    }

    Some(resolved_from_merged(
        world,
        entity,
        &merged,
        world.get_resource::<StyleSheet>(),
        false,
    ))
}

/// Resolve final style for an entity.
///
/// Cascading order:
/// 1. class styles from [`StyleSheet`] and [`StyleClass`]
/// 2. inline overrides from [`InlineStyle`] (or legacy inline components)
/// 3. pseudo classes from [`InteractionState`]
/// 4. animated override from [`CurrentColorStyle`] when present
#[must_use]
pub fn resolve_style(world: &World, entity: Entity) -> ResolvedStyle {
    if let Some(computed) = world.get::<ComputedStyle>(entity) {
        let mut style = ResolvedStyle {
            layout: computed.layout,
            colors: computed.colors.clone(),
            text: computed.text,
            font_family: computed.font_family.clone(),
            box_shadow: computed.box_shadow,
            transition: computed.transition,
        };

        if let Some(current) = world.get::<CurrentColorStyle>(entity) {
            if current.bg.is_some() {
                style.colors.bg = current.bg;
            }
            if current.text.is_some() {
                style.colors.text = current.text;
            }
            if current.border.is_some() {
                style.colors.border = current.border;
                style.colors.border_brush = current.border.map(BorderBrush::Color);
            }
            style.layout.scale = current.scale;
        }

        return style;
    }

    compute_resolved_style(world, entity).unwrap_or_else(|| ResolvedStyle {
        font_family: theme_default_font_family(world),
        ..ResolvedStyle::default()
    })
}

/// Resolve style from class names only, without inline entity overrides.
#[must_use]
pub fn resolve_style_for_classes<'a>(
    world: &World,
    class_names: impl IntoIterator<Item = &'a str>,
) -> ResolvedStyle {
    let merged = merged_from_class_names(world, None, class_names);
    let empty_tokens = HashMap::new();
    let sheet = world.get_resource::<StyleSheet>();
    let tokens = sheet.map(|sheet| &sheet.tokens).unwrap_or(&empty_tokens);
    let default_font_family = sheet.and_then(resolve_stylesheet_font_family);
    let merged = resolve_setter_values(&merged, tokens);

    let mut colors = ResolvedColorStyle {
        bg: merged.colors.bg,
        text: merged.colors.text,
        border: merged.colors.border,
        border_brush: merged.border_brush.clone(),
    };
    if colors.border.is_none() {
        colors.border = colors
            .border_brush
            .as_ref()
            .and_then(BorderBrush::as_solid_color);
    }

    ResolvedStyle {
        layout: to_resolved_layout(&merged.layout),
        colors,
        text: to_resolved_text(&merged.text),
        font_family: merged.font_family.or(default_font_family),
        box_shadow: merged.box_shadow,
        transition: merged.transition,
    }
}

/// Resolve style from class names with an explicit pseudo-state, without inline entity overrides.
#[must_use]
pub fn resolve_style_for_classes_with_state<'a>(
    world: &World,
    class_names: impl IntoIterator<Item = &'a str>,
    pseudo_state: StylePseudoState,
) -> ResolvedStyle {
    let merged = merged_from_class_names_with_state(world, None, Some(pseudo_state), class_names);
    let empty_tokens = HashMap::new();
    let sheet = world.get_resource::<StyleSheet>();
    let tokens = sheet.map(|sheet| &sheet.tokens).unwrap_or(&empty_tokens);
    let default_font_family = sheet.and_then(resolve_stylesheet_font_family);
    let merged = resolve_setter_values(&merged, tokens);

    let mut colors =
        target_colors_for_state(&merged.colors, pseudo_state.hovered, pseudo_state.pressed);
    colors.border_brush = merged.border_brush.clone();
    if colors.border.is_none() {
        colors.border = colors
            .border_brush
            .as_ref()
            .and_then(BorderBrush::as_solid_color);
    }

    ResolvedStyle {
        layout: to_resolved_layout(&merged.layout),
        colors,
        text: to_resolved_text(&merged.text),
        font_family: merged.font_family.or(default_font_family),
        box_shadow: merged.box_shadow,
        transition: merged.transition,
    }
}

/// Resolve style from class names while applying pseudo-state from a specific entity.
///
/// This is useful when a UI component's visual style is class-driven, but hover/pressed
/// state is tracked on an ECS entity via [`InteractionState`].
#[must_use]
pub fn resolve_style_for_entity_classes<'a>(
    world: &World,
    entity: Entity,
    class_names: impl IntoIterator<Item = &'a str>,
) -> ResolvedStyle {
    let merged = merged_from_class_names(world, Some(entity), class_names);
    resolved_from_merged(
        world,
        entity,
        &merged,
        world.get_resource::<StyleSheet>(),
        false,
    )
}

/// Map style-level justify-content to Masonry flex main-axis alignment.
#[must_use]
pub fn map_main_axis_alignment(justify_content: JustifyContent) -> MainAxisAlignment {
    match justify_content {
        JustifyContent::Start => MainAxisAlignment::Start,
        JustifyContent::Center => MainAxisAlignment::Center,
        JustifyContent::End => MainAxisAlignment::End,
        JustifyContent::SpaceBetween => MainAxisAlignment::SpaceBetween,
    }
}

/// Map style-level align-items to Masonry flex cross-axis alignment.
#[must_use]
pub fn map_cross_axis_alignment(align_items: AlignItems) -> CrossAxisAlignment {
    match align_items {
        AlignItems::Start => CrossAxisAlignment::Start,
        AlignItems::Center => CrossAxisAlignment::Center,
        AlignItems::End => CrossAxisAlignment::End,
        AlignItems::Stretch => CrossAxisAlignment::Stretch,
    }
}

fn map_text_alignment(text_align: TextAlign) -> ParleyTextAlign {
    match text_align {
        TextAlign::Start => ParleyTextAlign::Start,
        TextAlign::Center => ParleyTextAlign::Center,
        TextAlign::End => ParleyTextAlign::End,
    }
}

fn style_padding(value: f64) -> Padding {
    Padding::all(Length::px(value))
}

fn style_length(value: f64) -> Length {
    Length::px(value)
}

pub trait StyleFlexAlignmentExt {
    fn with_style_alignment(self, style: &ResolvedStyle) -> Self;
}

impl<Seq, State, Action> StyleFlexAlignmentExt for Flex<Seq, State, Action> {
    fn with_style_alignment(self, style: &ResolvedStyle) -> Self {
        self.main_axis_alignment(map_main_axis_alignment(style.layout.justify_content))
            .cross_axis_alignment(map_cross_axis_alignment(style.layout.align_items))
    }
}

/// Apply style-derived flex axis alignment on a flex view.
pub fn apply_flex_alignment<V>(view: V, style: &ResolvedStyle) -> V
where
    V: StyleFlexAlignmentExt,
{
    view.with_style_alignment(style)
}

/// Apply a resolved style to a view (short alias for [`apply_widget_style`]).
#[must_use]
pub fn styled<V>(view: V, style: &ResolvedStyle) -> impl WidgetView<(), ()>
where
    V: WidgetView<(), ()>,
{
    apply_widget_style(view, style)
}

/// Apply box/layout styling on any widget view.
///
/// Uses a wrapping `sized_box`, which paints via masonry's solid `BorderColor`
/// path only. Gradient borders require [`apply_direct_widget_style`] on a
/// widget that paints with [`BorderBrush`].
pub fn apply_widget_style<V>(view: V, style: &ResolvedStyle) -> impl WidgetView<(), ()>
where
    V: WidgetView<(), ()>,
{
    let scale = style.layout.scale.max(0.01);
    transformed(
        sized_box(view)
            .padding(style_padding(style.layout.padding))
            .corner_radius(style_length(style.layout.corner_radius))
            .border(
                style.colors.solid_border_color(),
                style_length(style.layout.border_width),
            )
            .background_color(style.colors.bg.unwrap_or(Color::TRANSPARENT))
            .box_shadow(style.box_shadow.unwrap_or_default()),
    )
    .scale(scale)
}

/// Apply style directly on the target widget.
///
/// This should be preferred for interactive UI components to ensure visual bounds
/// and hit-testing bounds remain identical. Border paint always goes through
/// [`BorderBrush`] (solid or gradient); solid colors also write legacy
/// `BorderColor` for masonry default `pre_paint` paths.
pub fn apply_direct_widget_style<V>(view: V, style: &ResolvedStyle) -> impl WidgetView<(), ()>
where
    V: WidgetView<(), ()>,
    V::Widget: Sized
        + UsesProperty<Padding>
        + UsesProperty<CornerRadius>
        + UsesProperty<BorderBrush>
        + UsesProperty<BorderColor>
        + UsesProperty<BorderWidth>
        + UsesProperty<Background>
        + UsesProperty<BoxShadow>,
{
    let scale = style.layout.scale.max(0.01);
    transformed(
        view.padding(style_padding(style.layout.padding))
            .corner_radius(style_length(style.layout.corner_radius))
            .border_brush(
                style.colors.resolved_border_brush(),
                style_length(style.layout.border_width),
            )
            .background_color(style.colors.bg.unwrap_or(Color::TRANSPARENT))
            .box_shadow(style.box_shadow.unwrap_or_default()),
    )
    .scale(scale)
}

fn to_target_component(style: &ResolvedStyle) -> TargetColorStyle {
    TargetColorStyle {
        bg: style.colors.bg,
        text: style.colors.text,
        border: style.colors.border,
        scale: style.layout.scale,
    }
}

fn to_current_component(colors: TargetColorStyle) -> CurrentColorStyle {
    CurrentColorStyle {
        bg: colors.bg,
        text: colors.text,
        border: colors.border,
        scale: colors.scale,
    }
}

fn ensure_current(world: &mut World, entity: Entity, current: CurrentColorStyle) {
    if let Some(mut current_component) = world.get_mut::<CurrentColorStyle>(entity) {
        *current_component = current;
    } else {
        world.entity_mut(entity).insert(current);
    }
}

fn spawn_color_style_tween(
    world: &mut World,
    entity: Entity,
    start: CurrentColorStyle,
    end: CurrentColorStyle,
    duration_secs: f32,
    easing: Option<EaseKind>,
) {
    let duration = Duration::from_secs_f32(duration_secs.max(0.0));
    let ease = easing.unwrap_or(EaseKind::QuadraticInOut);

    world.entity_mut(entity).insert((
        TimeSpan::try_from(Duration::ZERO..duration)
            .expect("style tween duration range should be valid"),
        ease,
        ComponentTween::new_target(entity, ColorStyleLens { start, end }),
        TimeRunner::new(duration),
        TimeContext::<()>::default(),
        StyleManagedTween,
    ));
}

fn clear_style_managed_tween(world: &mut World, entity: Entity) {
    if world.get::<StyleManagedTween>(entity).is_some() {
        world.entity_mut(entity).remove::<(
            TimeSpan,
            EaseKind,
            ComponentTween<ColorStyleLens>,
            TimeRunner,
            TimeContext<()>,
            TweenInterpolationValue,
            TweenPreviousValue,
            StyleManagedTween,
        )>();
    }
}

/// Consume interaction events and synchronize [`InteractionState`].
pub fn sync_ui_interaction_markers(world: &mut World) {
    let now_secs = world.resource::<Time>().elapsed_secs_f64();
    let events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiInteractionEvent>();

    for event in events {
        if world.get_entity(event.entity).is_err() {
            continue;
        }

        let before = world
            .get::<InteractionState>(event.entity)
            .copied()
            .unwrap_or_default();

        let mut after = before;
        match event.action {
            UiInteractionEvent::PointerEntered => {
                if let Some(debounce) = world.get::<HoverDebounce>(event.entity)
                    && debounce.enter_delay_secs > f32::EPSILON
                {
                    world.entity_mut(event.entity).insert(PendingHoverState {
                        entered_at_secs: now_secs,
                    });
                    after.hovered = false;
                } else {
                    after.hovered = true;
                }
            }
            UiInteractionEvent::PointerLeft => {
                after.hovered = false;
                world.entity_mut(event.entity).remove::<PendingHoverState>();
            }
            UiInteractionEvent::PointerPressed => {
                after.pressed = true;
                after.hovered = true;
                world.entity_mut(event.entity).remove::<PendingHoverState>();
            }
            UiInteractionEvent::PointerReleased => after.pressed = false,
        }

        if after != before {
            world.entity_mut(event.entity).insert(after);
            world.entity_mut(event.entity).insert(StyleDirty);
        }
    }
}

pub(crate) fn activate_debounced_hovers(
    time: Res<Time>,
    mut commands: Commands,
    query: Query<(
        Entity,
        &HoverDebounce,
        &PendingHoverState,
        Option<&InteractionState>,
    )>,
) {
    let now_secs = time.elapsed_secs_f64();

    for (entity, debounce, pending, state) in &query {
        if now_secs - pending.entered_at_secs < debounce.enter_delay_secs as f64 {
            continue;
        }

        let mut next = state.copied().unwrap_or_default();
        if !next.hovered {
            next.hovered = true;
            commands.entity(entity).insert((next, StyleDirty));
        }
        commands.entity(entity).remove::<PendingHoverState>();
    }
}

/// Incremental invalidation: marks entities that need style recomputation.
pub fn mark_style_dirty(world: &mut World) {
    let stylesheet_changed =
        world.is_resource_added::<StyleSheet>() || world.is_resource_changed::<StyleSheet>();

    let mut dirty = {
        let mut query = world.query_filtered::<Entity, Or<(
            Changed<StyleClass>,
            Changed<InlineStyle>,
            Changed<LayoutStyle>,
            Changed<ColorStyle>,
            Changed<TextStyle>,
            Changed<StyleTransition>,
            Changed<InteractionState>,
        )>>();
        query.iter(world).collect::<Vec<_>>()
    };

    let has_type_selectors = world
        .get_resource::<StyleSheet>()
        .is_some_and(StyleSheet::has_type_selectors);
    let has_descendant_selectors = world
        .get_resource::<StyleSheet>()
        .is_some_and(StyleSheet::has_descendant_selectors);

    if stylesheet_changed {
        if has_type_selectors || has_descendant_selectors {
            let mut all_entities = world.query::<Entity>();
            dirty.extend(all_entities.iter(world));
        } else {
            let mut candidates = world.query_filtered::<Entity, Or<(
                With<StyleClass>,
                With<InlineStyle>,
                With<LayoutStyle>,
                With<ColorStyle>,
                With<TextStyle>,
                With<StyleTransition>,
                With<ComputedStyle>,
            )>>();
            dirty.extend(candidates.iter(world));
        }
    }

    if has_descendant_selectors {
        let mut descendants = Vec::new();
        for entity in &dirty {
            let mut stack = vec![*entity];
            while let Some(current) = stack.pop() {
                if let Some(children) = world.get::<Children>(current) {
                    for child in children.iter() {
                        descendants.push(child);
                        stack.push(child);
                    }
                }
            }
        }
        dirty.extend(descendants);
    }

    if !has_type_selectors && !has_descendant_selectors {
        let stale = {
            let mut stale_query =
                world.query_filtered::<Entity, (With<ComputedStyle>, Without<StyleDirty>)>();
            stale_query
                .iter(world)
                .filter(|entity| {
                    world.get::<StyleClass>(*entity).is_none()
                        && world.get::<InlineStyle>(*entity).is_none()
                        && world.get::<LayoutStyle>(*entity).is_none()
                        && world.get::<ColorStyle>(*entity).is_none()
                        && world.get::<TextStyle>(*entity).is_none()
                        && world.get::<StyleTransition>(*entity).is_none()
                })
                .collect::<Vec<_>>()
        };
        dirty.extend(stale);
    }

    let mut unique = HashSet::new();
    for entity in dirty {
        if unique.insert(entity) && world.get_entity(entity).is_ok() {
            world.entity_mut(entity).insert(StyleDirty);
        }
    }
}

/// Compute and store target/current style states used by transition animation.
pub fn sync_style_targets(world: &mut World) {
    let entities = {
        let mut query = world.query_filtered::<Entity, With<StyleDirty>>();
        query.iter(world).collect::<Vec<_>>()
    };

    if entities.is_empty() {
        return;
    }

    let snapshots = {
        let world_ref: &World = world;
        entities
            .into_iter()
            .map(|entity| (entity, compute_resolved_style(world_ref, entity)))
            .collect::<Vec<_>>()
    };

    for (entity, resolved) in snapshots {
        match resolved {
            Some(resolved) => {
                if let Some(mut computed) = world.get_mut::<ComputedStyle>(entity) {
                    computed.layout = resolved.layout;
                    computed.colors = resolved.colors.clone();
                    computed.text = resolved.text;
                    computed.font_family = resolved.font_family.clone();
                    computed.box_shadow = resolved.box_shadow;
                    computed.transition = resolved.transition;
                } else {
                    world.entity_mut(entity).insert(ComputedStyle {
                        layout: resolved.layout,
                        colors: resolved.colors.clone(),
                        text: resolved.text,
                        font_family: resolved.font_family.clone(),
                        box_shadow: resolved.box_shadow,
                        transition: resolved.transition,
                    });
                }

                let target = to_target_component(&resolved);
                match resolved.transition {
                    Some(transition) => {
                        if let Some(mut target_component) =
                            world.get_mut::<TargetColorStyle>(entity)
                        {
                            *target_component = target;
                        } else {
                            world.entity_mut(entity).insert(target);
                        }

                        if world.get::<CurrentColorStyle>(entity).is_none() {
                            world
                                .entity_mut(entity)
                                .insert(to_current_component(target));
                        }

                        let end = to_current_component(target);

                        if transition.duration <= f32::EPSILON
                            || world.get_resource::<ReducedMotion>().is_some_and(|r| r.0)
                        {
                            ensure_current(world, entity, end);
                            clear_style_managed_tween(world, entity);
                        } else {
                            let start = world
                                .get::<CurrentColorStyle>(entity)
                                .copied()
                                .unwrap_or(end);

                            if start != end {
                                spawn_color_style_tween(
                                    world,
                                    entity,
                                    start,
                                    end,
                                    transition.duration,
                                    transition.easing,
                                );
                            } else {
                                clear_style_managed_tween(world, entity);
                            }
                        }
                    }
                    None => {
                        world.entity_mut(entity).remove::<TargetColorStyle>();
                        world.entity_mut(entity).remove::<CurrentColorStyle>();
                        clear_style_managed_tween(world, entity);
                    }
                }
            }
            None => {
                world.entity_mut(entity).remove::<ComputedStyle>();
                world.entity_mut(entity).remove::<TargetColorStyle>();
                world.entity_mut(entity).remove::<CurrentColorStyle>();
                clear_style_managed_tween(world, entity);
            }
        }

        world.entity_mut(entity).remove::<StyleDirty>();
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

fn unpack_rgba(color: Color) -> (u8, u8, u8, u8) {
    let rgba = color.to_rgba8();
    (rgba.r, rgba.g, rgba.b, rgba.a)
}

fn lerp_color(current: Color, target: Color, t: f32) -> Color {
    let (cr, cg, cb, ca) = unpack_rgba(current);
    let (tr, tg, tb, ta) = unpack_rgba(target);
    Color::from_rgba8(
        lerp_u8(cr, tr, t),
        lerp_u8(cg, tg, t),
        lerp_u8(cb, tb, t),
        lerp_u8(ca, ta, t),
    )
}

fn transparent_like(color: Color) -> Color {
    let rgba = color.to_rgba8();
    Color::from_rgba8(rgba.r, rgba.g, rgba.b, 0)
}

fn lerp_optional_color(start: Option<Color>, end: Option<Color>, t: f32) -> Option<Color> {
    match (start, end) {
        (Some(start), Some(end)) => Some(lerp_color(start, end, t)),
        (None, Some(end)) => Some(lerp_color(transparent_like(end), end, t)),
        (Some(start), None) => {
            if t >= 1.0 {
                None
            } else {
                Some(lerp_color(start, transparent_like(start), t))
            }
        }
        (None, None) => None,
    }
}

fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + ((end - start) * t)
}

fn lerp_f64(start: f64, end: f64, t: f32) -> f64 {
    start + ((end - start) * t as f64)
}

fn map_font_family_name(name: &str) -> FontFamilyName<'static> {
    let normalized = normalize_font_family_name(name).unwrap_or_else(|| name.trim().to_string());
    let lower = normalized.to_ascii_lowercase();
    if let Some(generic) = GenericFamily::parse(lower.as_str()) {
        FontFamilyName::Generic(generic)
    } else {
        FontFamilyName::Named(normalized.into())
    }
}

pub(crate) fn font_stack_from_style(style: &ResolvedStyle) -> Option<FontFamily<'static>> {
    let families = style.font_family.as_ref()?;
    if families.is_empty() {
        return None;
    }

    let mapped = families
        .iter()
        .filter(|name| normalize_font_family_name(name).is_some())
        .map(|name| map_font_family_name(name))
        .collect::<Vec<_>>();

    if mapped.is_empty() {
        return None;
    }

    if mapped.len() == 1 {
        Some(FontFamily::Single(mapped.into_iter().next().unwrap()))
    } else {
        Some(FontFamily::List(Cow::Owned(mapped)))
    }
}

/// Tween lens for animating computed style fields.
///
/// `font_family` is intentionally non-interpolated and only switches at the
/// end of the tween.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputedStyleLens {
    pub start: ComputedStyle,
    pub end: ComputedStyle,
}

impl Interpolator for ComputedStyleLens {
    type Item = ComputedStyle;

    fn interpolate(&self, target: &mut Self::Item, ratio: f32, _previous_value: f32) {
        let t = ratio.clamp(0.0, 1.0);

        target.layout.padding = lerp_f64(self.start.layout.padding, self.end.layout.padding, t);
        target.layout.gap = lerp_f64(self.start.layout.gap, self.end.layout.gap, t);
        target.layout.corner_radius = lerp_f64(
            self.start.layout.corner_radius,
            self.end.layout.corner_radius,
            t,
        );
        target.layout.border_width = lerp_f64(
            self.start.layout.border_width,
            self.end.layout.border_width,
            t,
        );
        target.layout.scale = lerp_f64(self.start.layout.scale, self.end.layout.scale, t);
        target.layout.justify_content = if t < 1.0 {
            self.start.layout.justify_content
        } else {
            self.end.layout.justify_content
        };
        target.layout.align_items = if t < 1.0 {
            self.start.layout.align_items
        } else {
            self.end.layout.align_items
        };

        target.colors.bg = lerp_optional_color(self.start.colors.bg, self.end.colors.bg, t);
        target.colors.text = lerp_optional_color(self.start.colors.text, self.end.colors.text, t);
        target.colors.border =
            lerp_optional_color(self.start.colors.border, self.end.colors.border, t);

        // Border brushes: lerp only when both ends are solid (or missing).
        // Gradients snap to the end brush at t == 1, otherwise keep start.
        let start_brush = self.start.colors.border_brush.as_ref();
        let end_brush = self.end.colors.border_brush.as_ref();
        let both_solidish = start_brush
            .map(|b| b.as_solid_color().is_some())
            .unwrap_or(true)
            && end_brush
                .map(|b| b.as_solid_color().is_some())
                .unwrap_or(true);
        if both_solidish {
            target.colors.border_brush = target.colors.border.map(BorderBrush::Color);
        } else if t >= 1.0 {
            target.colors.border_brush = self.end.colors.border_brush.clone();
        } else {
            target.colors.border_brush = self.start.colors.border_brush.clone();
        }

        target.text.size = lerp_f32(self.start.text.size, self.end.text.size, t);
        target.text.text_align = if t < 1.0 {
            self.start.text.text_align
        } else {
            self.end.text.text_align
        };
        target.transition = if t < 1.0 {
            self.start.transition
        } else {
            self.end.transition
        };

        // font family changes are discrete (non-interpolable)
        target.font_family = if t < 1.0 {
            self.start.font_family.clone()
        } else {
            self.end.font_family.clone()
        };
    }
}

/// Tween lens for animating [`CurrentColorStyle`] with CSS-like smooth transitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStyleLens {
    pub start: CurrentColorStyle,
    pub end: CurrentColorStyle,
}

impl Interpolator for ColorStyleLens {
    type Item = CurrentColorStyle;

    fn interpolate(&self, target: &mut Self::Item, ratio: f32, _previous_value: f32) {
        target.bg = lerp_optional_color(self.start.bg, self.end.bg, ratio);
        target.text = lerp_optional_color(self.start.text, self.end.text, ratio);
        target.border = lerp_optional_color(self.start.border, self.end.border, ratio);
        target.scale = lerp_f64(self.start.scale, self.end.scale, ratio);
    }
}

/// Style transition stepping is handled by `bevy_tween::DefaultTweenPlugins`.
///
/// This hook is intentionally kept as a no-op for schedule readability and
/// compatibility with existing system chains.
pub fn animate_style_transitions(world: &mut World) {
    let _ = world;
}

/// Apply text + box styling to a label view.
pub fn apply_label_style(view: Label, style: &ResolvedStyle) -> impl WidgetView<(), ()> {
    let mut styled = view
        .text_size(style.text.size)
        .text_alignment(map_text_alignment(style.text.text_align))
        .weight(FontWeight::new(style.text.weight))
        .line_height(LineHeight::FontSizeRelative(style.text.line_height));
    if let Some(font_stack) = font_stack_from_style(style) {
        styled = styled.font(font_stack);
    }

    let styled = if let Some(text_color) = style.colors.text {
        styled.text_color(text_color)
    } else {
        styled
    };

    styled.line_break_mode(LineBreaking::WordWrap)
}

/// Apply text + box styling to a text input view.
pub fn apply_text_input_style(
    view: TextInput<()>,
    style: &ResolvedStyle,
) -> impl WidgetView<(), ()> {
    let mut styled = view
        .text_size(style.text.size)
        .text_alignment(map_text_alignment(style.text.text_align));
    if let Some(font_stack) = font_stack_from_style(style) {
        styled = styled.font(font_stack);
    }
    styled
        .maybe_text_color(style.colors.text)
        .maybe_placeholder_color(style.colors.text.map(|color| color.with_alpha(0.72)))
}

/// Apply text-input styling directly on the widget itself.
pub fn apply_direct_text_input_style(
    view: TextInput<()>,
    style: &ResolvedStyle,
) -> impl WidgetView<(), ()> {
    let scale = style.layout.scale.max(0.01);
    let mut styled = view
        .text_size(style.text.size)
        .text_alignment(map_text_alignment(style.text.text_align));
    if let Some(font_stack) = font_stack_from_style(style) {
        styled = styled.font(font_stack);
    }
    transformed(
        styled
            .maybe_text_color(style.colors.text)
            .maybe_placeholder_color(style.colors.text.map(|color| color.with_alpha(0.72)))
            .padding(style_padding(style.layout.padding))
            .corner_radius(style_length(style.layout.corner_radius))
            .border_brush(
                style.colors.resolved_border_brush(),
                style_length(style.layout.border_width),
            )
            .background_color(style.colors.bg.unwrap_or(Color::TRANSPARENT))
            .box_shadow(style.box_shadow.unwrap_or_default()),
    )
    .scale(scale)
}

#[derive(Debug, Deserialize)]
struct StyleSheetDef {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    default_variant: Option<String>,
    #[serde(default)]
    font_family: OptionalStyleValueDef<FontFamilyStackDef>,
    #[serde(default, deserialize_with = "deserialize_optional_theme_backdrop")]
    backdrop: Option<ThemeBackdropDef>,
    #[serde(default)]
    tokens: HashMap<String, TokenDef>,
    #[serde(default)]
    rules: Vec<StyleRuleDef>,
}

#[derive(Debug, Deserialize)]
struct StyleSheetVariantsDef {
    default_variant: String,
    #[serde(default)]
    font_family: OptionalStyleValueDef<FontFamilyStackDef>,
    #[serde(default, deserialize_with = "deserialize_optional_theme_backdrop")]
    backdrop: Option<ThemeBackdropDef>,
    #[serde(default)]
    tokens: HashMap<String, TokenDef>,
    #[serde(default)]
    rules: Vec<StyleRuleDef>,
    #[serde(default)]
    variants: HashMap<String, StyleSheetDef>,
}

#[derive(Debug, Deserialize)]
struct ThemeBackdropDef {
    material: StyleValueDef<String>,
    #[serde(default)]
    color_scheme: WindowBackdropColorScheme,
    #[serde(default)]
    styles: HashMap<String, BackdropStyleDef>,
}

#[derive(Debug, Default, Deserialize)]
struct BackdropStyleDef {
    #[serde(default)]
    tokens: HashMap<String, TokenDef>,
    #[serde(default)]
    rules: Vec<StyleRuleDef>,
}

fn deserialize_optional_theme_backdrop<'de, D>(
    deserializer: D,
) -> Result<Option<ThemeBackdropDef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptionalThemeBackdropVisitor;

    impl<'de> Visitor<'de> for OptionalThemeBackdropVisitor {
        type Value = Option<ThemeBackdropDef>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a backdrop object, Some((...)), or None")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            ThemeBackdropDef::deserialize(deserializer).map(Some)
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            ThemeBackdropDef::deserialize(MapAccessDeserializer::new(map)).map(Some)
        }
    }

    deserializer.deserialize_any(OptionalThemeBackdropVisitor)
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptionalStringVisitor;

    impl<'de> Visitor<'de> for OptionalStringVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string, Some(\"...\"), or None")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            String::deserialize(deserializer).map(Some)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }

        fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
        where
            A: EnumAccess<'de>,
        {
            let (variant, variant_access) = data.variant::<String>()?;
            match variant.as_str() {
                "Some" => Ok(Some(variant_access.newtype_variant::<String>()?)),
                "None" => {
                    variant_access.unit_variant()?;
                    Ok(None)
                }
                _ => Err(de::Error::unknown_variant(&variant, &["Some", "None"])),
            }
        }
    }

    deserializer.deserialize_any(OptionalStringVisitor)
}

#[derive(Debug, Deserialize)]
struct StyleRuleDef {
    selector: SelectorDef,
    #[serde(default)]
    setter: StyleSetterDef,
}

#[derive(Debug, Deserialize)]
enum SelectorDef {
    Type(String),
    Class(String),
    PseudoClass(PseudoClass),
    And(Vec<SelectorDef>),
    Descendant {
        ancestor: Box<SelectorDef>,
        descendant: Box<SelectorDef>,
    },
}

impl From<SelectorDef> for Selector {
    fn from(value: SelectorDef) -> Self {
        match value {
            SelectorDef::Type(name) => Selector::type_name(name),
            SelectorDef::Class(name) => Selector::class(name),
            SelectorDef::PseudoClass(pseudo) => Selector::pseudo(pseudo),
            SelectorDef::And(selectors) => {
                Selector::and(selectors.into_iter().map(Into::into).collect::<Vec<_>>())
            }
            SelectorDef::Descendant {
                ancestor,
                descendant,
            } => Selector::descendant((*ancestor).into(), (*descendant).into()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct StyleSetterDef {
    #[serde(default)]
    layout: LayoutStyleDef,
    #[serde(default)]
    colors: ColorStyleDef,
    #[serde(default)]
    text: TextStyleDef,
    #[serde(default)]
    font_family: OptionalStyleValueDef<FontFamilyStackDef>,
    #[serde(default)]
    box_shadow: OptionalStyleValueDef<BoxShadowDef>,
    #[serde(default)]
    transition: OptionalStyleValueDef<StyleTransition>,
}

#[derive(Debug, Clone)]
struct OptionalLiteralValueDef<T>(Option<T>);

impl<T> Default for OptionalLiteralValueDef<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<'de, T> Deserialize<'de> for OptionalLiteralValueDef<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(Some(T::deserialize(deserializer)?)))
    }
}

impl<T> OptionalLiteralValueDef<T> {
    fn into_option(self) -> Option<T> {
        self.0
    }
}

#[derive(Debug, Default, Deserialize)]
struct LayoutStyleDef {
    #[serde(default)]
    padding: OptionalStyleValueDef<f64>,
    #[serde(default)]
    gap: OptionalStyleValueDef<f64>,
    #[serde(default)]
    corner_radius: OptionalStyleValueDef<f64>,
    #[serde(default)]
    border_width: OptionalStyleValueDef<f64>,
    #[serde(default)]
    justify_content: OptionalLiteralValueDef<JustifyContent>,
    #[serde(default)]
    align_items: OptionalLiteralValueDef<AlignItems>,
    #[serde(default)]
    scale: OptionalStyleValueDef<f64>,
    #[serde(default)]
    flex_grow: OptionalStyleValueDef<f64>,
}

impl LayoutStyleDef {
    fn into_layout_values(self) -> io::Result<LayoutStyleValue> {
        Ok(LayoutStyleValue {
            padding: into_style_value(self.padding.into_option(), Ok)?,
            gap: into_style_value(self.gap.into_option(), Ok)?,
            corner_radius: into_style_value(self.corner_radius.into_option(), Ok)?,
            border_width: into_style_value(self.border_width.into_option(), Ok)?,
            justify_content: self.justify_content.into_option().map(StyleValue::Value),
            align_items: self.align_items.into_option().map(StyleValue::Value),
            scale: into_style_value(self.scale.into_option(), Ok)?,
            flex_grow: into_style_value(self.flex_grow.into_option(), Ok)?,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct TextStyleDef {
    #[serde(default)]
    size: OptionalStyleValueDef<f32>,
    #[serde(default)]
    text_align: OptionalLiteralValueDef<TextAlign>,
    #[serde(default)]
    weight: OptionalStyleValueDef<f32>,
    #[serde(default)]
    line_height: OptionalStyleValueDef<f32>,
}

impl TextStyleDef {
    fn into_text_values(self) -> io::Result<TextStyleValue> {
        Ok(TextStyleValue {
            size: into_style_value(self.size.into_option(), Ok)?,
            text_align: self.text_align.into_option().map(StyleValue::Value),
            weight: into_style_value(self.weight.into_option(), Ok)?,
            line_height: into_style_value(self.line_height.into_option(), Ok)?,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct ColorStyleDef {
    #[serde(default)]
    bg: OptionalStyleValueDef<ColorDef>,
    #[serde(default, rename = "text")]
    text_color: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    border: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    hover_bg: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    hover_text: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    hover_border: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    pressed_bg: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    pressed_text: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    pressed_border: OptionalStyleValueDef<ColorDef>,
    #[serde(default)]
    border_brush: OptionalStyleValueDef<BrushDef>,
}

#[derive(Debug, Clone, Deserialize)]
enum ColorDef {
    Rgb(f32, f32, f32),
    Rgba(f32, f32, f32, f32),
    Rgb8(u8, u8, u8),
    Rgba8(u8, u8, u8, u8),
    Hex(String),
}

/// RON deserialization type for a border brush (solid color or gradient).
#[derive(Debug, Clone, Deserialize)]
enum BrushDef {
    Color(ColorDef),
    /// CSS-style relative linear gradient (spans the paint rect).
    ///
    /// RON: `Linear((angle: 180.0, stops: [...]))` with `angle` in degrees.
    Linear(LinearGradientDef),
    /// WinUI `LinearGradientBrush` with `MappingMode="Absolute"`.
    ///
    /// RON:
    /// `AbsoluteLinear((start: (0.0, 0.0), end: (0.0, 3.0), flip_y: false, stops: [...]))`
    AbsoluteLinear(AbsoluteLinearDef),
    /// WinUI `ControlElevationBorderBrush` / `AccentControlElevationBorderBrush`.
    ///
    /// Absolute `(0,0)→(0,extent)` (default extent 3), stops at 0.33 / 1.0,
    /// optional `flip_y` for Light/Accent (`ScaleY=-1 CenterY=0.5`).
    ///
    /// RON: `ControlElevation((flip_y: false, secondary: Rgba8(...), default: Rgba8(...)))`
    ControlElevation(ControlElevationDef),
}

/// RON deserialization type for a CSS-relative linear gradient.
#[derive(Debug, Clone, Deserialize)]
struct LinearGradientDef {
    angle: f64,
    stops: Vec<(f32, ColorDef)>,
}

/// RON deserialization type for a WinUI absolute linear gradient.
#[derive(Debug, Clone, Deserialize)]
struct AbsoluteLinearDef {
    start: (f64, f64),
    end: (f64, f64),
    #[serde(default)]
    flip_y: bool,
    stops: Vec<(f32, ColorDef)>,
}

/// RON shorthand for WinUI control / text elevation border brushes.
///
/// - Control elevation: `extent: 3`, stops `0.33` / `1.0` (default)
/// - Text control elevation: `extent: 2`, `secondary_stop: 0.5`, `flip_y: true`
#[derive(Debug, Clone, Deserialize)]
struct ControlElevationDef {
    #[serde(default)]
    flip_y: bool,
    /// Gradient line length in DIP (WinUI uses 3 for control elevation, 2 for text).
    #[serde(default = "default_elevation_extent")]
    extent: f64,
    /// Offset of the "secondary/strong" stop (control: 0.33, text: 0.5).
    #[serde(default = "default_elevation_secondary_stop")]
    secondary_stop: f32,
    /// Offset of the "default" stop (usually 1.0).
    #[serde(default = "default_elevation_default_stop")]
    default_stop: f32,
    secondary: ColorDef,
    default: ColorDef,
}

fn default_elevation_extent() -> f64 {
    3.0
}

fn default_elevation_secondary_stop() -> f32 {
    0.33
}

fn default_elevation_default_stop() -> f32 {
    1.0
}

impl LinearGradientDef {
    fn into_gradient(self) -> io::Result<Gradient> {
        let angle_rad = self.angle.to_radians();
        let mut gradient = Gradient::new_linear(angle_rad);
        // Collect stops once so with_stops replaces rather than appends.
        // ColorStopsSource is implemented for slices, not owned Vec.
        let mut stops: Vec<(f32, Color)> = Vec::with_capacity(self.stops.len());
        for (offset, color_def) in self.stops {
            stops.push((offset, color_def.into_color()?));
        }
        gradient = gradient.with_stops(stops.as_slice());
        Ok(gradient)
    }

    fn into_brush(self) -> io::Result<BorderBrush> {
        Ok(BorderBrush::Gradient(self.into_gradient()?))
    }
}

impl AbsoluteLinearDef {
    fn into_brush(self) -> io::Result<BorderBrush> {
        let mut stops = Vec::with_capacity(self.stops.len());
        for (offset, color_def) in self.stops {
            stops.push((offset, color_def.into_color()?));
        }
        Ok(BorderBrush::AbsoluteLinear(AbsoluteLinearGradient {
            start: self.start,
            end: self.end,
            flip_y: self.flip_y,
            stops,
        }))
    }
}

impl ControlElevationDef {
    fn into_brush(self) -> io::Result<BorderBrush> {
        let secondary = self.secondary.into_color()?;
        let default = self.default.into_color()?;
        let extent = if self.extent > 0.0 {
            self.extent
        } else {
            3.0
        };
        Ok(BorderBrush::AbsoluteLinear(AbsoluteLinearGradient {
            start: (0.0, 0.0),
            end: (0.0, extent),
            flip_y: self.flip_y,
            stops: vec![
                (self.secondary_stop, secondary),
                (self.default_stop, default),
            ],
        }))
    }
}

impl BrushDef {
    fn into_brush(self) -> io::Result<BorderBrush> {
        match self {
            BrushDef::Color(color_def) => Ok(BorderBrush::Color(color_def.into_color()?)),
            BrushDef::Linear(linear) => linear.into_brush(),
            BrushDef::AbsoluteLinear(linear) => linear.into_brush(),
            BrushDef::ControlElevation(elevation) => elevation.into_brush(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FontFamilyStackDef(Vec<String>);

impl FontFamilyStackDef {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

fn normalize_font_family_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let unquoted = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .or_else(|| {
            trimmed
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .unwrap_or(trimmed)
        .trim();

    if unquoted.is_empty() {
        return None;
    }

    let lower = unquoted.to_ascii_lowercase();
    let normalized = match lower.as_str() {
        "-apple-system" | "blinkmacsystemfont" => "system-ui",
        _ => unquoted,
    };

    Some(normalized.to_string())
}

fn push_unique_font_family(stack: &mut Vec<String>, name: String) {
    if !stack
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(name.as_str()))
    {
        stack.push(name);
    }
}

fn parsed_font_family_name_to_string(name: FontFamilyName<'_>) -> Option<String> {
    match name {
        FontFamilyName::Named(name) => normalize_font_family_name(name.as_ref()),
        FontFamilyName::Generic(generic) => Some(generic.to_string()),
    }
}

fn parse_css_font_family_stack(source: &str) -> Result<Vec<String>, String> {
    let mut stack = Vec::new();

    for family in FontFamilyName::parse_css_list(source) {
        let family = family.map_err(|error| error.to_string())?;
        if let Some(name) = parsed_font_family_name_to_string(family) {
            push_unique_font_family(&mut stack, name);
        }
    }

    if stack.is_empty() {
        Err("font family stack must contain at least one family".to_string())
    } else {
        Ok(stack)
    }
}

impl<'de> Deserialize<'de> for FontFamilyStackDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FontFamilyStackVisitor;

        impl<'de> Visitor<'de> for FontFamilyStackVisitor {
            type Value = FontFamilyStackDef;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a CSS font-family string or a sequence of font family names")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                parse_css_font_family_stack(value)
                    .map(FontFamilyStackDef)
                    .map_err(de::Error::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(value.as_str())
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let values = Vec::<String>::deserialize(SeqAccessDeserializer::new(seq))
                    .map_err(de::Error::custom)?;
                let mut stack = Vec::new();

                for value in values {
                    if let Some(name) = normalize_font_family_name(value.as_str()) {
                        push_unique_font_family(&mut stack, name);
                    }
                }

                if stack.is_empty() {
                    Err(de::Error::custom(
                        "font family stack must contain at least one family",
                    ))
                } else {
                    Ok(FontFamilyStackDef(stack))
                }
            }
        }

        deserializer.deserialize_any(FontFamilyStackVisitor)
    }
}

#[derive(Debug, Clone, Deserialize)]
enum TokenDef {
    Color(ColorDef),
    Float(f64),
    FontFamily(FontFamilyStackDef),
    BoxShadow(BoxShadowDef),
    Transition(StyleTransition),
    Backdrop(String),
    /// Cubic bezier curve control points.
    Curve(f32, f32, f32, f32),
    /// CSS-relative linear gradient brush token.
    ///
    /// RON: `Linear((angle: 180.0, stops: [...]))` with `angle` in degrees.
    Linear(LinearGradientDef),
    /// WinUI absolute linear gradient brush token.
    ///
    /// RON: `AbsoluteLinear((start: (0.0, 0.0), end: (0.0, 3.0), flip_y: false, stops: [...]))`
    AbsoluteLinear(AbsoluteLinearDef),
    /// WinUI ControlElevationBorderBrush shorthand token.
    ///
    /// RON: `ControlElevation((flip_y: false, secondary: Rgba8(...), default: Rgba8(...)))`
    ControlElevation(ControlElevationDef),
    /// Explicit brush token (solid or gradient).
    Brush(BrushDef),
}

impl TokenDef {
    fn into_token_value(self) -> io::Result<TokenValue> {
        match self {
            Self::Color(color) => Ok(TokenValue::Color(color.into_color()?)),
            Self::Float(value) => Ok(TokenValue::Float(value)),
            Self::FontFamily(value) => Ok(TokenValue::FontFamily(value.into_vec())),
            Self::BoxShadow(value) => Ok(TokenValue::BoxShadow(value.into_box_shadow()?)),
            Self::Transition(value) => Ok(TokenValue::Transition(value)),
            Self::Backdrop(value) => WindowBackdropMaterial::from_theme_name(&value)
                .map(TokenValue::Backdrop)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
            Self::Curve(x1, y1, x2, y2) => Ok(TokenValue::Curve(x1, y1, x2, y2)),
            Self::Linear(linear) => Ok(TokenValue::BorderBrush(linear.into_brush()?)),
            Self::AbsoluteLinear(linear) => Ok(TokenValue::BorderBrush(linear.into_brush()?)),
            Self::ControlElevation(elevation) => {
                Ok(TokenValue::BorderBrush(elevation.into_brush()?))
            }
            Self::Brush(brush) => Ok(TokenValue::BorderBrush(brush.into_brush()?)),
        }
    }
}

#[derive(Debug, Clone)]
enum StyleValueDef<T> {
    Value(T),
    Var(String),
}

impl<'de, T> Deserialize<'de> for StyleValueDef<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        fn literal_to_style_value<'de, T, U, E>(value: U) -> Result<StyleValueDef<T>, E>
        where
            T: Deserialize<'de>,
            U: IntoDeserializer<'de, de::value::Error>,
            E: de::Error,
        {
            T::deserialize(value.into_deserializer())
                .map(StyleValueDef::Value)
                .map_err(de::Error::custom)
        }

        struct StyleValueVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for StyleValueVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = StyleValueDef<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a style literal value or Var(\"token-name\") reference")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_style_value::<T, _, E>(value)
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let de = SeqAccessDeserializer::new(seq);
                let values = Vec::<ron::Value>::deserialize(de).map_err(de::Error::custom)?;

                if let Ok(value) = T::deserialize(ron::Value::Seq(values.clone())) {
                    return Ok(StyleValueDef::Value(value));
                }

                if let [ron::Value::String(token)] = values.as_slice() {
                    return Ok(StyleValueDef::Var(token.clone()));
                }

                if let [ron::Value::String(tag), ron::Value::String(token)] = values.as_slice()
                    && tag == "Var"
                {
                    return Ok(StyleValueDef::Var(token.clone()));
                }

                Err(de::Error::custom(
                    "invalid style value sequence; expected literal sequence value or Var(\"token\")",
                ))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let de = MapAccessDeserializer::new(map);
                let raw = ron::Value::deserialize(de).map_err(de::Error::custom)?;

                if let Ok(value) = T::deserialize(raw.clone()) {
                    return Ok(StyleValueDef::Value(value));
                }

                if let ron::Value::Map(entries) = &raw
                    && entries.len() == 1
                    && let Some((ron::Value::String(tag), value)) = entries.iter().next()
                    && tag == "Var"
                    && let ron::Value::String(token) = value
                {
                    return Ok(StyleValueDef::Var(token.clone()));
                }

                Err(de::Error::custom(
                    "invalid style value map; expected literal map value or Var(\"token\")",
                ))
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                let (variant, variant_access) = data.variant::<String>()?;
                match variant.as_str() {
                    "Var" => Ok(StyleValueDef::Var(
                        variant_access.newtype_variant::<String>()?,
                    )),
                    "Value" => Ok(StyleValueDef::Value(variant_access.newtype_variant::<T>()?)),
                    _ => Err(de::Error::unknown_variant(&variant, &["Var", "Value"])),
                }
            }
        }

        deserializer.deserialize_any(StyleValueVisitor(std::marker::PhantomData))
    }
}

#[derive(Debug, Clone, Default)]
enum OptionalStyleValueDef<T> {
    Style(StyleValueDef<T>),
    #[default]
    None,
}

impl<'de, T> Deserialize<'de> for OptionalStyleValueDef<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        fn literal_to_optional_style<'de, T, U, E>(value: U) -> Result<OptionalStyleValueDef<T>, E>
        where
            T: Deserialize<'de>,
            U: IntoDeserializer<'de, de::value::Error>,
            E: de::Error,
        {
            T::deserialize(value.into_deserializer())
                .map(StyleValueDef::Value)
                .map(OptionalStyleValueDef::Style)
                .map_err(de::Error::custom)
        }

        struct OptionalStyleValueVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for OptionalStyleValueVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = OptionalStyleValueDef<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an optional style value literal, Var(\"token-name\"), or None")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OptionalStyleValueDef::None)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(OptionalStyleValueDef::None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                StyleValueDef::<T>::deserialize(deserializer).map(OptionalStyleValueDef::Style)
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_char<E>(self, value: char) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                literal_to_optional_style::<T, _, E>(value)
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let de = SeqAccessDeserializer::new(seq);
                StyleValueDef::<T>::deserialize(de)
                    .map(OptionalStyleValueDef::Style)
                    .map_err(de::Error::custom)
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let de = MapAccessDeserializer::new(map);
                StyleValueDef::<T>::deserialize(de)
                    .map(OptionalStyleValueDef::Style)
                    .map_err(de::Error::custom)
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                let (variant, variant_access) = data.variant::<String>()?;
                match variant.as_str() {
                    "None" => {
                        variant_access.unit_variant()?;
                        Ok(OptionalStyleValueDef::None)
                    }
                    "Some" => Ok(OptionalStyleValueDef::Style(
                        variant_access.newtype_variant::<StyleValueDef<T>>()?,
                    )),
                    "Var" => Ok(OptionalStyleValueDef::Style(StyleValueDef::Var(
                        variant_access.newtype_variant::<String>()?,
                    ))),
                    "Value" => Ok(OptionalStyleValueDef::Style(StyleValueDef::Value(
                        variant_access.newtype_variant::<T>()?,
                    ))),
                    _ => Err(de::Error::unknown_variant(
                        &variant,
                        &["None", "Some", "Var", "Value"],
                    )),
                }
            }
        }

        deserializer.deserialize_any(OptionalStyleValueVisitor(std::marker::PhantomData))
    }
}

impl<T> OptionalStyleValueDef<T> {
    fn into_option(self) -> Option<StyleValueDef<T>> {
        match self {
            Self::Style(value) => Some(value),
            Self::None => None,
        }
    }
}

fn into_style_value<T, U>(
    value: Option<StyleValueDef<T>>,
    map: impl FnOnce(T) -> io::Result<U>,
) -> io::Result<Option<StyleValue<U>>> {
    match value {
        None => Ok(None),
        Some(StyleValueDef::Value(value)) => Ok(Some(StyleValue::Value(map(value)?))),
        Some(StyleValueDef::Var(name)) => Ok(Some(StyleValue::Var(name))),
    }
}

impl ColorDef {
    fn into_color(self) -> io::Result<Color> {
        match self {
            Self::Rgb(r, g, b) => Ok(Color::from_rgb8(
                float_color_component_to_u8(r),
                float_color_component_to_u8(g),
                float_color_component_to_u8(b),
            )),
            Self::Rgba(r, g, b, a) => Ok(Color::from_rgba8(
                float_color_component_to_u8(r),
                float_color_component_to_u8(g),
                float_color_component_to_u8(b),
                float_color_component_to_u8(a),
            )),
            Self::Rgb8(r, g, b) => Ok(Color::from_rgb8(r, g, b)),
            Self::Rgba8(r, g, b, a) => Ok(Color::from_rgba8(r, g, b, a)),
            Self::Hex(hex) => parse_hex_color(&hex),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BoxShadowDef {
    color: ColorDef,
    #[serde(default)]
    offset_x: f64,
    #[serde(default)]
    offset_y: f64,
    #[serde(default)]
    blur: f64,
}

impl BoxShadowDef {
    fn into_box_shadow(self) -> io::Result<BoxShadow> {
        Ok(
            BoxShadow::new(self.color.into_color()?, (self.offset_x, self.offset_y))
                .blur(style_length(self.blur)),
        )
    }
}

impl StyleSetterDef {
    fn into_setter(self) -> io::Result<StyleSetterValue> {
        Ok(StyleSetterValue {
            layout: self.layout.into_layout_values()?,
            colors: self.colors.into_color_style_values()?,
            text: self.text.into_text_values()?,
            font_family: into_style_value(self.font_family.into_option(), |value| {
                Ok(value.into_vec())
            })?,
            box_shadow: into_style_value(
                self.box_shadow.into_option(),
                BoxShadowDef::into_box_shadow,
            )?,
            transition: into_style_value(self.transition.into_option(), Ok)?,
        })
    }
}

impl ColorStyleDef {
    fn into_color_style_value(
        value: Option<StyleValueDef<ColorDef>>,
    ) -> io::Result<Option<StyleValue<Color>>> {
        match value {
            None => Ok(None),
            Some(StyleValueDef::Value(value)) => Ok(Some(StyleValue::Value(value.into_color()?))),
            Some(StyleValueDef::Var(name)) => {
                // `Hex("#RRGGBB[AA]")` may be deserialized as a bare string payload on some
                // enum paths, which can be misclassified as `Var("#...")`. Recover by
                // recognizing hex literals here and treating them as literal colors.
                let trimmed = name.trim();
                if trimmed.starts_with('#')
                    && let Ok(color) = parse_hex_color(trimmed)
                {
                    return Ok(Some(StyleValue::Value(color)));
                }

                Ok(Some(StyleValue::Var(name)))
            }
        }
    }

    fn into_brush_style_value(
        value: Option<StyleValueDef<BrushDef>>,
    ) -> io::Result<Option<StyleValue<BorderBrush>>> {
        match value {
            None => Ok(None),
            Some(StyleValueDef::Value(value)) => {
                Ok(Some(StyleValue::Value(value.into_brush()?)))
            }
            Some(StyleValueDef::Var(name)) => {
                Ok(Some(StyleValue::Var(name)))
            }
        }
    }

    fn into_color_style_values(self) -> io::Result<ColorStyleValue> {
        Ok(ColorStyleValue {
            bg: Self::into_color_style_value(self.bg.into_option())?,
            text: Self::into_color_style_value(self.text_color.into_option())?,
            border: Self::into_color_style_value(self.border.into_option())?,
            hover_bg: Self::into_color_style_value(self.hover_bg.into_option())?,
            hover_text: Self::into_color_style_value(self.hover_text.into_option())?,
            hover_border: Self::into_color_style_value(self.hover_border.into_option())?,
            pressed_bg: Self::into_color_style_value(self.pressed_bg.into_option())?,
            pressed_text: Self::into_color_style_value(self.pressed_text.into_option())?,
            pressed_border: Self::into_color_style_value(self.pressed_border.into_option())?,
            border_brush: Self::into_brush_style_value(self.border_brush.into_option())?,
        })
    }
}

fn float_color_component_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn parse_hex_color(hex: &str) -> io::Result<Color> {
    let hex = hex.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);

    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid hex color `{hex}`; expected #RRGGBB or #RRGGBBAA"),
        )
    };

    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| invalid())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| invalid())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| invalid())?;
            Ok(Color::from_rgb8(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| invalid())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| invalid())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| invalid())?;
            let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| invalid())?;
            Ok(Color::from_rgba8(r, g, b, a))
        }
        _ => Err(invalid()),
    }
}

fn style_rules_from_defs(rules: Vec<StyleRuleDef>) -> io::Result<Vec<StyleRule>> {
    rules
        .into_iter()
        .map(|rule| {
            Ok(StyleRule::new_with_values(
                rule.selector.into(),
                rule.setter.into_setter()?,
            ))
        })
        .collect()
}

fn token_values_from_defs(
    tokens: HashMap<String, TokenDef>,
) -> io::Result<HashMap<String, TokenValue>> {
    tokens
        .into_iter()
        .map(|(name, token)| Ok((name, token.into_token_value()?)))
        .collect()
}

impl ThemeBackdropDef {
    fn into_theme_backdrop(self) -> io::Result<ThemeBackdrop> {
        let material = match self.material {
            StyleValueDef::Value(name) => StyleValue::Value(
                WindowBackdropMaterial::from_theme_name(&name)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            ),
            StyleValueDef::Var(token) => StyleValue::Var(token),
        };
        let mut styles = HashMap::new();
        for (name, style) in self.styles {
            let material = WindowBackdropMaterial::from_theme_name(&name)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            styles.insert(
                material,
                BackdropStyle {
                    tokens: token_values_from_defs(style.tokens)?,
                    rules: style_rules_from_defs(style.rules)?,
                },
            );
        }

        Ok(ThemeBackdrop {
            material,
            color_scheme: self.color_scheme,
            styles,
        })
    }
}

fn stylesheet_from_def(parsed: StyleSheetDef) -> io::Result<StyleSheet> {
    let mut sheet = StyleSheet {
        default_variant: parsed.default_variant,
        font_family: into_style_value(parsed.font_family.into_option(), |value| {
            Ok(value.into_vec())
        })?,
        backdrop: parsed
            .backdrop
            .map(ThemeBackdropDef::into_theme_backdrop)
            .transpose()?,
        ..Default::default()
    };
    sheet.tokens = token_values_from_defs(parsed.tokens)?;
    sheet.rules = style_rules_from_defs(parsed.rules)?;

    Ok(sheet)
}

fn stylesheet_from_ron_bytes(bytes: &[u8]) -> io::Result<StyleSheet> {
    let parsed: StyleSheetDef = ron::de::from_bytes(bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse stylesheet RON: {error}"),
        )
    })?;

    stylesheet_from_def(parsed)
}

fn stylesheet_variants_from_ron_bytes(bytes: &[u8]) -> io::Result<RegisteredStyleVariants> {
    let parsed: StyleSheetVariantsDef = ron::de::from_bytes(bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse stylesheet variants RON: {error}"),
        )
    })?;

    if parsed.variants.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stylesheet variants RON must define at least one variant",
        ));
    }

    let default_variant = parsed.default_variant;
    let base_sheet = stylesheet_from_def(StyleSheetDef {
        default_variant: None,
        font_family: parsed.font_family,
        backdrop: parsed.backdrop,
        tokens: parsed.tokens,
        rules: parsed.rules,
    })?;

    let mut raw_variants = HashMap::new();
    for (name, def) in parsed.variants {
        raw_variants.insert(name, stylesheet_from_def(def)?);
    }

    if !raw_variants.contains_key(&default_variant) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("stylesheet variants RON default_variant `{default_variant}` is not defined"),
        ));
    }

    let mut variants = HashMap::new();
    for (name, variant_sheet) in raw_variants {
        let mut merged = base_sheet.clone();
        merge_sheet_inplace(&mut merged, variant_sheet);
        variants.insert(name, merged);
    }

    Ok(RegisteredStyleVariants {
        default_variant,
        variants,
    })
}

#[cfg(test)]
pub(crate) fn parse_stylesheet_ron_for_tests(ron_text: &str) -> io::Result<StyleSheet> {
    parse_stylesheet_ron(ron_text)
}

#[cfg(test)]
pub(crate) fn parse_stylesheet_variants_ron_for_tests(
    ron_text: &str,
) -> io::Result<RegisteredStyleVariants> {
    parse_stylesheet_variants_ron(ron_text)
}

/// Asset loader for stylesheet `.ron` files.
#[derive(Default, TypePath)]
pub struct StyleSheetRonLoader;

impl AssetLoader for StyleSheetRonLoader {
    type Asset = StyleSheet;
    type Settings = ();
    type Error = io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        stylesheet_from_ron_bytes(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use crate::{AppI18n, SyncTextSource};
    use crate::{AppPicusExt, PicusPlugin, UiEventQueue, UiRoot};
    use bevy_app::App;
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_window::{PrimaryWindow, Window};
    use picus_view::picus_widget::properties::ContentColor;

    #[test]
    fn plugin_registers_embedded_fluent_variants_without_activating_theme() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let active = app.world().resource::<crate::ActiveStyleSheetAsset>();
        assert!(active.path.is_none());

        let active_variant = app.world().resource::<crate::ActiveStyleVariant>();
        assert_eq!(active_variant.0.as_deref(), None);

        let applied_variant_before_update = app.world().resource::<crate::AppliedStyleVariant>();
        assert_eq!(applied_variant_before_update.0.as_deref(), None);

        let variants = app.world().resource::<crate::RegisteredStyleVariants>();
        assert!(variants.variants.contains_key("dark"));
        assert!(variants.variants.contains_key("light"));
        assert!(variants.variants.contains_key("high-contrast"));

        app.update();

        let sheet = app.world().resource::<crate::StyleSheet>();
        assert!(sheet.rules.is_empty());
        assert!(sheet.tokens.is_empty());

        let applied_variant = app.world().resource::<crate::AppliedStyleVariant>();
        assert_eq!(applied_variant.0.as_deref(), None);
    }
    #[test]
    fn no_active_theme_projects_label_text_as_transparent() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(320.0, 200.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        app.world_mut()
            .spawn((crate::UiLabel::new("Hidden text"), ChildOf(root)));

        app.update();
        app.update();

        let text_color = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary()
                .expect("primary window runtime should exist");
            let label = first_widget_by_short_name_and_debug_text(
                window_runtime.render_root.get_layer_root(0),
                "Label",
                "Hidden text",
            )
            .expect("projected label should exist");
            label.get_prop::<ContentColor>().color
        };

        assert_eq!(text_color, crate::xilem::Color::TRANSPARENT);
    }
    #[test]
    fn markdown_projects_common_blocks_into_retained_labels() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(640.0, 480.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let text_color = crate::xilem::Color::from_rgb8(0x11, 0x22, 0x33);
        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        app.world_mut().spawn((
            crate::UiMarkdown::new(
                "# Markdown title\n\nSome **bold** and [link](https://example.com).\n\n- [x] Complete\n\n| Feature | Status |\n| :-- | --: |\n| Table | Done |\n\n```rust\nlet x = 1;\n```",
            ),
            crate::ColorStyle {
                text: Some(text_color),
                ..Default::default()
            },
            ChildOf(root),
        ));

        app.update();
        app.update();

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        let window_runtime = runtime
            .primary()
            .expect("primary window runtime should exist");
        let layer_root = window_runtime.render_root.get_layer_root(0);

        let title =
            first_widget_by_short_name_and_debug_text(layer_root, "Label", "Markdown title")
                .expect("markdown heading should project as a label");
        assert_eq!(title.get_prop::<ContentColor>().color, text_color);

        for expected in [
            "bold",
            "link",
            "☑ Complete",
            "Feature",
            "Done",
            "let x = 1;",
        ] {
            assert!(
                find_widget_id_by_debug_text(layer_root, expected).is_some(),
                "markdown should project retained label text `{expected}`"
            );
        }
    }
    #[test]
    fn no_active_theme_projects_markdown_without_backend_text_color() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(320.0, 200.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        app.world_mut().spawn((
            crate::UiMarkdown::new("# Invisible title\n\nHidden paragraph"),
            ChildOf(root),
        ));

        app.update();
        app.update();

        let runtime = app.world().non_send::<crate::MasonryRuntime>();
        let window_runtime = runtime
            .primary()
            .expect("primary window runtime should exist");
        let layer_root = window_runtime.render_root.get_layer_root(0);

        assert!(
            find_widget_id_by_debug_text(layer_root, "Invisible title").is_none(),
            "un-themed markdown should not fall back to visible backend label text"
        );
    }
    #[test]
    fn no_active_theme_projects_text_input_text_as_transparent() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(320.0, 200.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        app.world_mut().spawn((
            crate::UiTextInput::new("Typed").with_placeholder("Placeholder"),
            ChildOf(root),
        ));

        app.update();
        app.update();

        let text_color = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let window_runtime = runtime
                .primary()
                .expect("primary window runtime should exist");
            let text_area = first_widget_by_short_name_and_debug_text(
                window_runtime.render_root.get_layer_root(0),
                "TextArea",
                "Typed",
            )
            .expect("projected text input should build an inner TextArea");
            text_area.get_prop::<ContentColor>().color
        };

        assert_eq!(text_color, crate::xilem::Color::TRANSPARENT);
    }
    #[test]
    fn embedded_fluent_theme_defines_priority_control_visual_styles() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        app.update();

        let badge = app.world_mut().spawn((crate::UiBadge::new("Beta"),)).id();
        let progress = app
            .world_mut()
            .spawn((crate::UiProgressBar::determinate(0.5),))
            .id();

        let badge_style = resolve_style(app.world(), badge);
        assert!(badge_style.layout.corner_radius > 20.0);
        assert!(badge_style.colors.bg.is_some());
        assert!(badge_style.colors.border.is_some());

        let progress_style = resolve_style(app.world(), progress);
        // WinUI ProgressBar shell is transparent; track/fill are template parts.
        assert_eq!(progress_style.layout.padding, 0.0);
        assert_eq!(progress_style.layout.corner_radius, 0.0);
        assert!(progress_style.colors.bg.is_some());

        let checkbox_box = crate::resolve_style_for_classes(app.world(), ["template.checkbox.box"]);
        assert!(checkbox_box.colors.bg.is_some());
        assert!(checkbox_box.colors.border.is_some());

        let switch_on = crate::resolve_style_for_classes(
            app.world(),
            ["template.switch.track", "template.switch.track.on"],
        );
        assert!(switch_on.colors.bg.is_some());
        assert!(switch_on.colors.border.is_some());

        let radio_indicator =
            crate::resolve_style_for_classes(app.world(), ["widget.radio.indicator"]);
        assert_eq!(radio_indicator.layout.border_width, 1.0);
        assert!(radio_indicator.colors.border.is_some());

        let radio_selected = crate::resolve_style_for_classes(
            app.world(),
            ["widget.radio.indicator", "widget.radio.indicator.selected"],
        );
        // Selected radio uses CircleElevation gradient for the check glyph stroke.
        assert!(
            matches!(
                radio_selected.colors.resolved_border_brush(),
                BorderBrush::Gradient(_)
            ),
            "selected radio indicator should use CircleElevation gradient"
        );
        assert!(radio_selected.colors.text.is_some());

        let date_panel =
            crate::resolve_style_for_classes(app.world(), ["overlay.date_picker.panel"]);
        assert_eq!(date_panel.layout.padding, 0.0);

        let progress_fill =
            crate::resolve_style_for_classes(app.world(), ["template.progress.fill"]);
        assert!(progress_fill.colors.bg.is_some());

        // Default UiButton uses WinUI ControlElevationBorderBrush (absolute 3 DIP ramp).
        let button = app.world_mut().spawn(crate::UiButton::new("Save")).id();
        let button_style = resolve_style(app.world(), button);
        match button_style.colors.resolved_border_brush() {
            BorderBrush::AbsoluteLinear(gradient) => {
                assert_eq!(gradient.start, (0.0, 0.0));
                assert_eq!(gradient.end, (0.0, 3.0));
                assert!(
                    !gradient.flip_y,
                    "dark ControlElevationBorderBrush should not flip_y"
                );
                assert_eq!(gradient.stops.len(), 2);
                // Geometry: Absolute (0,0)→(0,3) relative to paint target top-left.
                let rect = masonry_core::kurbo::Rect::new(10.0, 20.0, 110.0, 60.0);
                let (p0, p1) = gradient.resolve_points(rect);
                assert_eq!(p0, masonry_core::kurbo::Point::new(10.0, 20.0));
                assert_eq!(p1, masonry_core::kurbo::Point::new(10.0, 23.0));
            }
            other => panic!(
                "default UiButton border should use AbsoluteLinear elevation brush, got {other:?}"
            ),
        }

        // Light theme ControlElevationBorderBrush uses ScaleY=-1 CenterY=0.5.
        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();
        let light_button = app.world_mut().spawn(crate::UiButton::new("Save")).id();
        let light_style = resolve_style(app.world(), light_button);
        match light_style.colors.resolved_border_brush() {
            BorderBrush::AbsoluteLinear(gradient) => {
                assert!(
                    gradient.flip_y,
                    "light ControlElevationBorderBrush should flip_y"
                );
                let rect = masonry_core::kurbo::Rect::new(0.0, 0.0, 100.0, 40.0);
                let (p0, p1) = gradient.resolve_points(rect);
                assert_eq!(p0, masonry_core::kurbo::Point::new(0.0, 40.0));
                assert_eq!(p1, masonry_core::kurbo::Point::new(0.0, 37.0));
            }
            other => panic!("light UiButton border should use AbsoluteLinear, got {other:?}"),
        }
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        app.update();

        // WinUI keeps ControlElevationBorderBrush on PointerOver.
        let button_hovered = app
            .world_mut()
            .spawn((
                crate::UiButton::new("Save"),
                InteractionState {
                    hovered: true,
                    pressed: false,
                    focused: false,
                },
            ))
            .id();
        let hovered_style = resolve_style(app.world(), button_hovered);
        assert!(
            matches!(
                hovered_style.colors.resolved_border_brush(),
                BorderBrush::AbsoluteLinear(_)
            ),
            "hovered UiButton should keep elevation border, got {:?}",
            hovered_style.colors.border_brush
        );

        // WinUI ButtonBorderBrushPressed → solid ControlStrokeColorDefault.
        let button_pressed = app
            .world_mut()
            .spawn((
                crate::UiButton::new("Save"),
                InteractionState {
                    hovered: true,
                    pressed: true,
                    focused: false,
                },
            ))
            .id();
        let pressed_style = resolve_style(app.world(), button_pressed);
        assert!(
            matches!(
                pressed_style.colors.resolved_border_brush(),
                BorderBrush::Color(_)
            ),
            "pressed UiButton should use solid stroke, got {:?}",
            pressed_style.colors.border_brush
        );

        // WinUI TextControlElevationBorderBrush: Absolute 0,0→0,2 + flip_y.
        let text = app.world_mut().spawn(crate::UiTextInput::default()).id();
        let text_style = resolve_style(app.world(), text);
        match text_style.colors.resolved_border_brush() {
            BorderBrush::AbsoluteLinear(gradient) => {
                assert_eq!(gradient.end, (0.0, 2.0));
                assert!(
                    gradient.flip_y,
                    "text elevation always flips to bottom edge"
                );
                assert_eq!(gradient.stops.len(), 2);
                assert!((gradient.stops[0].0 - 0.5).abs() < f32::EPSILON);
            }
            other => panic!("UiTextInput should use text elevation AbsoluteLinear, got {other:?}"),
        }

        let text_focused = app
            .world_mut()
            .spawn((
                crate::UiTextInput::default(),
                InteractionState {
                    hovered: false,
                    pressed: false,
                    focused: true,
                },
            ))
            .id();
        let focused_style = resolve_style(app.world(), text_focused);
        assert!(
            matches!(
                focused_style.colors.resolved_border_brush(),
                BorderBrush::AbsoluteLinear(_)
            ),
            "focused text control should use focused elevation brush"
        );

        // WinUI CircleElevationBorderBrush: CSS-relative linear top→bottom.
        let circle = crate::resolve_style_for_classes(app.world(), ["template.switch.thumb"]);
        assert!(
            matches!(
                circle.colors.resolved_border_brush(),
                BorderBrush::Gradient(_)
            ),
            "switch thumb should use relative CircleElevation gradient, got {:?}",
            circle.colors.border_brush
        );
    }
    #[test]
    fn embedded_fluent_theme_defines_interactive_state_defaults() {
        fn token_color(world: &World, name: &str) -> crate::xilem::Color {
            match world
                .resource::<crate::StyleSheet>()
                .tokens
                .get(name)
                .unwrap_or_else(|| panic!("missing color token `{name}`"))
            {
                crate::TokenValue::Color(color) => *color,
                other => panic!("token `{name}` should be a color, got {other:?}"),
            }
        }

        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        app.update();

        let hovered = app
            .world_mut()
            .spawn((
                crate::UiPasswordInput::default(),
                InteractionState {
                    hovered: true,
                    pressed: false,
                    focused: false,
                },
            ))
            .id();
        let pressed = app
            .world_mut()
            .spawn((
                crate::UiTimePicker::default(),
                InteractionState {
                    hovered: false,
                    pressed: true,
                    focused: false,
                },
            ))
            .id();
        let search = app.world_mut().spawn(crate::UiSearch::default()).id();
        let search_hovered = app
            .world_mut()
            .spawn((
                crate::UiSearch::default(),
                InteractionState {
                    hovered: true,
                    pressed: false,
                    focused: false,
                },
            ))
            .id();
        let search_pressed = app
            .world_mut()
            .spawn((
                crate::UiSearch::default(),
                InteractionState {
                    hovered: false,
                    pressed: true,
                    focused: false,
                },
            ))
            .id();

        let password_hover = resolve_style(app.world(), hovered);
        // WinUI keeps TextControlElevationBorderBrush on hover (not a solid hover stroke).
        assert!(
            matches!(
                password_hover.colors.resolved_border_brush(),
                BorderBrush::AbsoluteLinear(_)
            ),
            "password hover should keep text elevation brush, got {:?}",
            password_hover.colors.border_brush
        );
        assert_eq!(
            password_hover.colors.bg,
            Some(token_color(app.world(), "fill-control-secondary"))
        );

        let time_pressed = resolve_style(app.world(), pressed);
        assert_eq!(
            time_pressed.colors.bg,
            Some(token_color(app.world(), "surface-subtle-pressed"))
        );

        assert_eq!(
            resolve_style(app.world(), search).colors.bg,
            Some(token_color(app.world(), "fill-control-default"))
        );
        assert_eq!(
            resolve_style(app.world(), search_hovered).colors.bg,
            Some(token_color(app.world(), "fill-control-secondary"))
        );
        assert_eq!(
            resolve_style(app.world(), search_pressed).colors.bg,
            Some(token_color(app.world(), "fill-control-tertiary"))
        );

        let numeric_decrease =
            resolve_style_for_entity_classes(app.world(), hovered, ["numericUpDown.decrease"]);
        assert_eq!(
            numeric_decrease.colors.bg,
            Some(token_color(app.world(), "surface-subtle-hover"))
        );

        let context_item =
            resolve_style_for_entity_classes(app.world(), pressed, ["overlay.context_menu.item"]);
        assert_eq!(
            context_item.colors.bg,
            Some(token_color(app.world(), "surface-overlay-item-pressed"))
        );

        let list_item =
            resolve_style_for_entity_classes(app.world(), hovered, ["widget.list_view.item"]);
        assert_eq!(
            list_item.colors.bg,
            Some(token_color(app.world(), "fill-subtle-secondary"))
        );

        let nav_item = resolve_style_for_entity_classes(app.world(), hovered, ["nav.item"]);
        assert_eq!(
            nav_item.colors.bg,
            Some(token_color(app.world(), "fill-subtle-secondary"))
        );

        let nav_sidebar = resolve_style_for_classes(app.world(), ["nav.sidebar"]);
        assert_eq!(
            nav_sidebar.colors.bg,
            Some(token_color(app.world(), "fill-layer-background"))
        );
        assert_eq!(nav_sidebar.layout.border_width, 0.0);

        let nav_content = resolve_style_for_classes(app.world(), ["nav.content"]);
        assert_eq!(
            nav_content.colors.bg,
            Some(token_color(app.world(), "fill-layer-background"))
        );

        let scroll_viewport =
            resolve_style_for_classes(app.world(), ["template.scroll_view.viewport"]);
        assert_eq!(scroll_viewport.layout.border_width, 0.0);

        let scroll_view = app.world_mut().spawn(crate::UiScrollView::default()).id();
        let scroll_style = resolve_style(app.world(), scroll_view);
        assert_eq!(
            scroll_style.colors.bg,
            Some(token_color(app.world(), "fill-layer-background"))
        );
        assert_eq!(scroll_style.layout.border_width, 0.0);

        let switch_on = resolve_style_for_entity_classes(
            app.world(),
            hovered,
            ["template.switch.track", "template.switch.track.on"],
        );
        assert_eq!(
            switch_on.colors.bg,
            Some(token_color(app.world(), "surface-accent-hover"))
        );
    }
    #[test]
    fn embedded_fluent_theme_does_not_style_picus_only_group_box() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        app.update();

        let group_box = app
            .world_mut()
            .spawn((crate::UiGroupBox::new("Nested group"),))
            .id();

        let group_style = resolve_style(app.world(), group_box);
        assert_eq!(group_style.layout.padding, 0.0);
        assert_eq!(group_style.layout.border_width, 0.0);
        assert!(group_style.colors.bg.is_none());
        assert!(group_style.colors.border.is_none());

        let title_style = crate::resolve_style_for_classes(app.world(), ["widget.group_box.title"]);
        assert!(title_style.colors.text.is_none());
    }
    #[test]
    fn active_style_variant_switches_automatically_without_install_calls() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();

        let light_surface = app
            .world()
            .resource::<crate::StyleSheet>()
            .tokens
            .get("surface-bg")
            .cloned()
            .expect("surface-bg should exist after active variant switch to light");

        assert_eq!(
            light_surface,
            crate::TokenValue::Color(crate::xilem::Color::from_rgb8(0xFF, 0xFF, 0xFF))
        );

        let applied_variant = app.world().resource::<crate::AppliedStyleVariant>();
        assert_eq!(applied_variant.0.as_deref(), Some("light"));
    }
    #[test]
    fn active_style_variant_light_overrides_surface_bg_token() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();

        let sheet = app.world().resource::<crate::StyleSheet>();
        let token = sheet
            .tokens
            .get("surface-bg")
            .expect("surface-bg token should exist after fluent light activation");

        assert!(matches!(
            token,
            crate::TokenValue::Color(color)
                if *color == crate::xilem::Color::from_rgb8(0xFF, 0xFF, 0xFF)
        ));
    }
    #[test]
    fn active_style_variant_high_contrast_overrides_surface_bg_token() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        crate::set_active_style_variant_by_name(app.world_mut(), "high-contrast");
        app.update();

        let sheet = app.world().resource::<crate::StyleSheet>();
        let token = sheet
            .tokens
            .get("surface-bg")
            .expect("surface-bg token should exist after fluent high-contrast activation");

        assert!(matches!(
            token,
            crate::TokenValue::Color(color)
                if *color == crate::xilem::Color::from_rgb8(0x00, 0x00, 0x00)
        ));
    }
    #[test]
    fn active_style_variant_api_switches_between_dark_light_and_high_contrast() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();

        let light_surface = app
            .world()
            .resource::<crate::StyleSheet>()
            .tokens
            .get("surface-bg")
            .cloned()
            .expect("surface-bg should exist after light activation");
        assert_eq!(
            light_surface,
            crate::TokenValue::Color(crate::xilem::Color::from_rgb8(0xFF, 0xFF, 0xFF))
        );

        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        app.update();

        let dark_surface = app
            .world()
            .resource::<crate::StyleSheet>()
            .tokens
            .get("surface-bg")
            .cloned()
            .expect("surface-bg should exist after dark activation");
        assert_eq!(
            dark_surface,
            crate::TokenValue::Color(crate::xilem::Color::from_rgb8(0x1F, 0x1F, 0x1F))
        );

        crate::set_active_style_variant_by_name(app.world_mut(), "high-contrast");
        app.update();

        let hc_surface = app
            .world()
            .resource::<crate::StyleSheet>()
            .tokens
            .get("surface-bg")
            .cloned()
            .expect("surface-bg should exist after high-contrast activation");
        assert_eq!(
            hc_surface,
            crate::TokenValue::Color(crate::xilem::Color::from_rgb8(0x00, 0x00, 0x00))
        );
    }
    #[test]
    fn load_style_sheet_ron_applies_and_persists_across_variant_switches() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).load_style_sheet_ron(
            r##"(
                rules: [
                    (
                        selector: Class("demo.embedded"),
                        setter: (
                            colors: (
                                bg: Hex("#123456"),
                            ),
                        ),
                    ),
                ],
            )"##,
        );

        let entity = app
            .world_mut()
            .spawn((crate::StyleClass(vec!["demo.embedded".to_string()]),))
            .id();

        app.update();

        let expected = crate::xilem::Color::from_rgb8(0x12, 0x34, 0x56);
        assert_eq!(resolve_style(app.world(), entity).colors.bg, Some(expected));

        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();

        assert_eq!(resolve_style(app.world(), entity).colors.bg, Some(expected));
    }
    #[test]
    fn load_style_sheet_ron_default_variant_applies_registered_variant_when_unset() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin).load_style_sheet_ron(
            r##"(
                default_variant: "light",
                rules: [
                    (
                        selector: Class("demo.uses-theme-token"),
                        setter: (
                            colors: (
                                bg: Var("surface-bg"),
                            ),
                        ),
                    ),
                ],
            )"##,
        );

        let active_variant = app.world().resource::<crate::ActiveStyleVariant>();
        assert_eq!(active_variant.0.as_deref(), Some("light"));

        let applied_variant = app.world().resource::<crate::AppliedStyleVariant>();
        assert_eq!(applied_variant.0.as_deref(), Some("light"));

        let entity = app
            .world_mut()
            .spawn((crate::StyleClass(vec!["demo.uses-theme-token".to_string()]),))
            .id();

        assert_eq!(
            resolve_style(app.world(), entity).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0xFF, 0xFF, 0xFF))
        );
    }
    #[test]
    fn load_style_sheet_ron_default_variant_preserves_existing_active_variant() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        crate::apply_active_style_variant(app.world_mut())
            .expect("embedded Fluent dark theme should apply");

        app.load_style_sheet_ron(
            r##"(
                default_variant: "light",
                rules: [],
            )"##,
        );

        let active_variant = app.world().resource::<crate::ActiveStyleVariant>();
        assert_eq!(active_variant.0.as_deref(), Some("dark"));

        let applied_variant = app.world().resource::<crate::AppliedStyleVariant>();
        assert_eq!(applied_variant.0.as_deref(), Some("dark"));
    }
    #[test]
    fn parse_stylesheet_variants_merges_default_rules_and_variant_overrides() {
        let ron_text = r##"(
            default_variant: "dark",
            rules: [
                (
                    selector: Class("demo.root"),
                    setter: (
                        colors: (
                            bg: Var("surface-bg"),
                        ),
                    ),
                ),
            ],
            variants: {
                "dark": (
                    tokens: {
                        "surface-bg": Color(Hex("#111111")),
                    },
                ),
                "light": (
                    tokens: {
                        "surface-bg": Color(Hex("#EEEEEE")),
                    },
                ),
            },
        )"##;

        let variants = crate::parse_stylesheet_variants_ron_for_tests(ron_text)
            .expect("variant bundle should parse in tests");

        assert_eq!(variants.default_variant, "dark");
        let dark = variants
            .variants
            .get("dark")
            .expect("dark variant should exist");
        let light = variants
            .variants
            .get("light")
            .expect("light variant should exist");

        assert_eq!(dark.rules.len(), 1);
        assert_eq!(light.rules.len(), 1);
        assert_eq!(
            light.tokens.get("surface-bg"),
            Some(&crate::TokenValue::Color(crate::xilem::Color::from_rgb8(
                0xEE, 0xEE, 0xEE,
            )))
        );
    }
    #[test]
    fn stylesheet_parses_backdrop_token_and_material_styles() {
        let sheet = crate::parse_stylesheet_ron_for_tests(
            r##"(
                backdrop: (
                    material: (Var: "window-backdrop"),
                    color_scheme: Dark,
                    styles: {
                        "mica": (
                            tokens: {
                                "fill-layer-background": Color(Rgba8(0, 0, 0, 0)),
                            },
                        ),
                    },
                ),
                tokens: {
                    "window-backdrop": Backdrop("mica"),
                },
            )"##,
        )
        .expect("stylesheet backdrop object should parse");

        assert_eq!(
            crate::resolve_theme_backdrop_material(&sheet),
            Some(crate::WindowBackdropMaterial::Mica)
        );
        assert_eq!(
            crate::resolve_theme_backdrop_color_scheme(&sheet),
            Some(crate::WindowBackdropColorScheme::Dark)
        );
        assert!(
            sheet
                .backdrop
                .as_ref()
                .is_some_and(|backdrop| backdrop
                    .styles
                    .contains_key(&crate::WindowBackdropMaterial::Mica))
        );
    }
    #[test]
    fn fluent_backdrop_override_updates_material_and_public_fill_tokens() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        crate::apply_active_style_variant(app.world_mut())
            .expect("embedded Fluent dark theme should apply");

        let root = app.world_mut().spawn(crate::UiRoot).id();
        assert_eq!(
            crate::resolve_theme_backdrop_material(app.world().resource::<crate::StyleSheet>()),
            Some(crate::WindowBackdropMaterial::None)
        );
        assert_eq!(
            resolve_style(app.world(), root).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0x1F, 0x1F, 0x1F))
        );

        crate::set_theme_backdrop_material(
            app.world_mut(),
            crate::WindowBackdropMaterial::Mica,
        );

        let sheet = app.world().resource::<crate::StyleSheet>();
        assert_eq!(
            crate::resolve_theme_backdrop_material(sheet),
            Some(crate::WindowBackdropMaterial::Mica)
        );
        assert_eq!(
            crate::resolve_theme_backdrop_color_scheme(sheet),
            Some(crate::WindowBackdropColorScheme::Dark)
        );
        assert_eq!(
            sheet.tokens.get(crate::WINDOW_BACKDROP_TOKEN),
            Some(&crate::TokenValue::Backdrop(
                crate::WindowBackdropMaterial::Mica
            ))
        );
        assert_eq!(
            resolve_style(app.world(), root).colors.bg,
            Some(crate::xilem::Color::TRANSPARENT)
        );
        let dialog = app
            .world_mut()
            .spawn(crate::UiDialog::new("Title", "Body"))
            .id();
        let search = app.world_mut().spawn(crate::UiSearch::default()).id();
        let nav_content = resolve_style_for_classes(app.world(), ["nav.content"]);
        let nav_sidebar = resolve_style_for_classes(app.world(), ["nav.sidebar"]);
        let dimmer = resolve_style_for_classes(app.world(), ["overlay.modal.dimmer"]);

        assert_eq!(
            resolve_style(app.world(), dialog).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0x1F, 0x1F, 0x1F)),
            "dialogs must remain opaque over native backdrops"
        );
        assert_eq!(
            resolve_style(app.world(), search).colors.bg,
            Some(crate::xilem::Color::from_rgba8(255, 255, 255, 15))
        );
        assert_eq!(nav_content.colors.bg, Some(crate::xilem::Color::TRANSPARENT));
        assert_eq!(nav_sidebar.colors.bg, Some(crate::xilem::Color::TRANSPARENT));
        assert_eq!(nav_sidebar.layout.border_width, 0.0);
        assert_eq!(
            dimmer.colors.bg,
            Some(crate::xilem::Color::from_rgba8(0, 0, 0, 77))
        );

        crate::clear_theme_backdrop_material_override(app.world_mut());
        assert_eq!(
            crate::resolve_theme_backdrop_material(app.world().resource::<crate::StyleSheet>()),
            Some(crate::WindowBackdropMaterial::None)
        );
        assert_eq!(
            resolve_style(app.world(), root).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0x1F, 0x1F, 0x1F))
        );
    }
    #[test]
    fn fluent_backdrop_color_scheme_tracks_active_variant() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");
        crate::apply_active_style_variant(app.world_mut())
            .expect("embedded Fluent dark theme should apply");
        crate::set_theme_backdrop_material(
            app.world_mut(),
            crate::WindowBackdropMaterial::Mica,
        );
        let window = app.world_mut().spawn(Window::default()).id();

        app.update();
        assert_eq!(
            app.world()
                .get::<crate::WindowBackdropColorScheme>(window),
            Some(&crate::WindowBackdropColorScheme::Dark)
        );

        crate::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();
        assert_eq!(
            app.world()
                .get::<crate::WindowBackdropColorScheme>(window),
            Some(&crate::WindowBackdropColorScheme::Light)
        );
    }
    #[test]
    fn active_stylesheet_backdrop_override_restores_unexpanded_source() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::apply_active_stylesheet_ron(
            app.world_mut(),
            r##"(
                backdrop: (
                    material: (Var: "window-backdrop"),
                    styles: {
                        "mica": (
                            tokens: {
                                "demo-layer": Color(Rgba8(0, 0, 0, 0)),
                            },
                        ),
                    },
                ),
                tokens: {
                    "window-backdrop": Backdrop("none"),
                    "demo-layer": Color(Hex("#223344")),
                },
                rules: [
                    (
                        selector: Class("demo.backdrop"),
                        setter: (colors: (bg: Var("demo-layer"))),
                    ),
                ],
            )"##,
        )
        .expect("active backdrop stylesheet should apply");
        let entity = app
            .world_mut()
            .spawn(crate::StyleClass(vec!["demo.backdrop".to_string()]))
            .id();

        assert_eq!(
            resolve_style(app.world(), entity).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0x22, 0x33, 0x44))
        );
        crate::set_theme_backdrop_material(
            app.world_mut(),
            crate::WindowBackdropMaterial::Mica,
        );
        assert_eq!(
            resolve_style(app.world(), entity).colors.bg,
            Some(crate::xilem::Color::TRANSPARENT)
        );

        crate::clear_theme_backdrop_material_override(app.world_mut());
        assert_eq!(
            resolve_style(app.world(), entity).colors.bg,
            Some(crate::xilem::Color::from_rgb8(0x22, 0x33, 0x44))
        );
    }
    #[test]
    fn embedded_fluent_variants_inherit_shared_top_level_rules() {
        let variants = crate::styling::parse_stylesheet_variants_ron_for_tests(
            crate::styling::BUILTIN_FLUENT_THEME_RON,
        )
        .expect("embedded fluent theme bundle should parse");

        let dark = variants
            .variants
            .get("dark")
            .expect("dark variant should exist");
        let light = variants
            .variants
            .get("light")
            .expect("light variant should exist");
        let high_contrast = variants
            .variants
            .get("high-contrast")
            .expect("high-contrast variant should exist");

        assert!(
            !dark.rules.is_empty(),
            "dark variant rules should be non-empty"
        );
        assert!(
            !light.rules.is_empty(),
            "light variant should inherit non-empty shared rules"
        );
        assert!(
            !high_contrast.rules.is_empty(),
            "high-contrast variant should inherit non-empty shared rules"
        );

        assert_eq!(light.rules.len(), dark.rules.len());
        assert_eq!(high_contrast.rules.len(), dark.rules.len());
    }
    #[test]
    fn apply_active_style_variant_applies_selected_registered_variant_to_runtime_sheet() {
        let ron_text = r##"(
            default_variant: "dark",
            variants: {
                "dark": (
                    tokens: {
                        "surface-bg": Color(Hex("#111111")),
                    },
                ),
                "light": (
                    tokens: {
                        "surface-bg": Color(Hex("#F8F8F8")),
                    },
                ),
            },
        )"##;

        let mut world = World::new();
        crate::register_stylesheet_variants_ron(&mut world, ron_text)
            .expect("style variants should register");
        crate::set_active_style_variant_by_name(&mut world, "light");
        crate::apply_active_style_variant(&mut world)
            .expect("registered active style variant should apply");

        let token = world
            .resource::<crate::StyleSheet>()
            .tokens
            .get("surface-bg")
            .cloned()
            .expect("surface-bg should exist after variant application");

        assert_eq!(
            token,
            crate::TokenValue::Color(crate::xilem::Color::from_rgb8(0xF8, 0xF8, 0xF8))
        );
    }
    #[test]
    fn resolve_style_for_entity_classes_applies_hover_pseudo_state() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();
        let base = crate::xilem::Color::from_rgb8(0x11, 0x22, 0x33);
        let hover = crate::xilem::Color::from_rgb8(0xAA, 0xBB, 0xCC);

        sheet.set_class(
            "test.button",
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(base),
                    hover_bg: Some(hover),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        );
        world.insert_resource(sheet);

        let entity = world
            .spawn((InteractionState {
                hovered: true,
                pressed: false,
                focused: false,
            },))
            .id();
        let resolved = resolve_style_for_entity_classes(&world, entity, ["test.button"]);

        assert_eq!(resolved.colors.bg, Some(hover));
    }
    #[test]
    fn resolve_style_without_any_style_source_has_no_theme_values() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();

        let resolved = resolve_style(&world, entity);

        assert_eq!(resolved, crate::ResolvedStyle::default());
    }
    #[test]
    fn selector_and_rule_applies_hover_and_pressed_states() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();

        let base = crate::xilem::Color::from_rgb8(0x22, 0x22, 0x22);
        let hover = crate::xilem::Color::from_rgb8(0x44, 0x44, 0x44);
        let pressed = crate::xilem::Color::from_rgb8(0x66, 0x66, 0x66);

        sheet.add_rule(StyleRule::new(
            Selector::class("test.button"),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(base),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));
        sheet.add_rule(StyleRule::new(
            Selector::and(vec![
                Selector::class("test.button"),
                Selector::pseudo(crate::PseudoClass::Hovered),
            ]),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(hover),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));
        sheet.add_rule(StyleRule::new(
            Selector::and(vec![
                Selector::class("test.button"),
                Selector::pseudo(crate::PseudoClass::Pressed),
            ]),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(pressed),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));

        world.insert_resource(sheet);

        let entity = world
            .spawn((
                crate::StyleClass(vec!["test.button".to_string()]),
                InteractionState {
                    hovered: true,
                    pressed: true,
                    focused: false,
                },
            ))
            .id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        let resolved = resolve_style(&world, entity);
        assert_eq!(resolved.colors.bg, Some(pressed));
    }
    #[test]
    fn selector_type_rule_matches_component_type() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();
        let type_color = crate::xilem::Color::from_rgb8(0x12, 0x34, 0x56);

        sheet.add_rule(StyleRule::new(
            Selector::of_type::<TypeStyled>(),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(type_color),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));
        world.insert_resource(sheet);

        let entity = world.spawn((TypeStyled,)).id();
        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        let resolved = resolve_style(&world, entity);
        assert_eq!(resolved.colors.bg, Some(type_color));
    }
    #[test]
    fn ui_root_background_uses_stylesheet_rules_and_class_overrides() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();

        let base_bg = crate::xilem::Color::from_rgb8(0x22, 0x26, 0x2F);
        let light_bg = crate::xilem::Color::from_rgb8(0xF4, 0xF7, 0xFF);

        sheet.add_rule(StyleRule::new(
            Selector::of_type::<UiRoot>(),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(base_bg),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));

        sheet.set_class(
            "theme.light",
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(light_bg),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        );

        world.insert_resource(sheet);

        let root = world
            .spawn((UiRoot, crate::StyleClass(vec!["theme.dark".to_string()])))
            .id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);
        assert_eq!(resolve_style(&world, root).colors.bg, Some(base_bg));

        world.clear_trackers();
        world
            .entity_mut(root)
            .insert(crate::StyleClass(vec!["theme.light".to_string()]));

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);
        assert_eq!(resolve_style(&world, root).colors.bg, Some(light_bg));
    }
    #[test]
    fn selector_descendant_rule_matches_nested_entity_and_updates_on_ancestor_change() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();

        let dark_bg = crate::xilem::Color::from_rgb8(0x20, 0x2A, 0x44);
        let light_bg = crate::xilem::Color::from_rgb8(0xE8, 0xEE, 0xFF);

        sheet.add_rule(StyleRule::new(
            Selector::descendant(
                Selector::class("theme.dark"),
                Selector::class("gallery.target"),
            ),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(dark_bg),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));

        sheet.add_rule(StyleRule::new(
            Selector::descendant(
                Selector::class("theme.light"),
                Selector::class("gallery.target"),
            ),
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(light_bg),
                    ..ColorStyle::default()
                },
                ..StyleSetter::default()
            },
        ));

        world.insert_resource(sheet);

        let root = world
            .spawn((crate::StyleClass(vec!["theme.dark".to_string()]),))
            .id();
        let child = world
            .spawn((
                crate::StyleClass(vec!["gallery.target".to_string()]),
                ChildOf(root),
            ))
            .id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);
        assert_eq!(resolve_style(&world, child).colors.bg, Some(dark_bg));

        world.clear_trackers();
        world
            .entity_mut(root)
            .insert(crate::StyleClass(vec!["theme.light".to_string()]));

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);
        assert_eq!(resolve_style(&world, child).colors.bg, Some(light_bg));
    }
    #[test]
    fn sync_style_targets_restarts_tween_when_current_differs_but_target_unchanged() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();

        let base = crate::xilem::Color::from_rgb8(0x20, 0x2A, 0x44);
        let mid = crate::xilem::Color::from_rgb8(0x90, 0x99, 0xB3);

        sheet.set_class(
            "test.animated",
            StyleSetter {
                colors: ColorStyle {
                    bg: Some(base),
                    ..ColorStyle::default()
                },
                transition: Some(crate::StyleTransition {
                    duration: 0.2,
                    easing: None,
                }),
                ..StyleSetter::default()
            },
        );

        world.insert_resource(sheet);

        let entity = world
            .spawn((crate::StyleClass(vec!["test.animated".to_string()]),))
            .id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        world.entity_mut(entity).insert(crate::CurrentColorStyle {
            bg: Some(mid),
            text: None,
            border: None,
            scale: 1.0,
        });
        world.entity_mut(entity).insert(crate::TargetColorStyle {
            bg: Some(base),
            text: None,
            border: None,
            scale: 1.0,
        });
        world.entity_mut(entity).insert(crate::StyleDirty);

        crate::sync_style_targets(&mut world);

        assert_eq!(
            world
                .get::<crate::TargetColorStyle>(entity)
                .and_then(|target| target.bg),
            Some(base)
        );
        assert!(world.get::<TimeRunner>(entity).is_some());
        assert!(
            world
                .get::<ComponentTween<crate::ColorStyleLens>>(entity)
                .is_some()
        );
    }
    #[test]
    fn pointer_left_does_not_clear_pressed_marker() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(bevy_time::Time::<()>::default());

        let entity = world
            .spawn((crate::InteractionState {
                hovered: true,
                pressed: true,
                focused: false,
            },))
            .id();

        world
            .resource::<UiEventQueue>()
            .push_typed(entity, crate::UiInteractionEvent::PointerLeft);

        crate::sync_ui_interaction_markers(&mut world);

        let state = world
            .get::<crate::InteractionState>(entity)
            .expect("interaction state should exist");
        assert!(!state.hovered);
        assert!(state.pressed);
    }
    #[test]
    fn debounced_hover_waits_before_setting_hovered_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());
        world.insert_resource(bevy_time::Time::<()>::default());

        let entity = world
            .spawn((crate::styling::HoverDebounce {
                enter_delay_secs: 0.05,
            },))
            .id();

        world
            .resource::<UiEventQueue>()
            .push_typed(entity, crate::UiInteractionEvent::PointerEntered);

        crate::sync_ui_interaction_markers(&mut world);

        assert!(world.get::<crate::InteractionState>(entity).is_none());

        world
            .resource_mut::<bevy_time::Time<()>>()
            .advance_by(Duration::from_millis(60));

        let mut schedule = Schedule::default();
        schedule.add_systems(crate::styling::activate_debounced_hovers);
        schedule.run(&mut world);

        let state = world
            .get::<crate::InteractionState>(entity)
            .expect("interaction state should exist after debounce elapses");
        assert!(state.hovered);
    }
    #[test]
    fn sync_style_targets_keeps_unmanaged_tween_anim() {
        let mut world = World::new();

        let duration = Duration::from_secs(1);
        let entity = world.spawn_empty().id();
        world.entity_mut(entity).insert((
            TimeSpan::try_from(Duration::ZERO..duration)
                .expect("test tween duration range should be valid"),
            EaseKind::Linear,
            ComponentTween::new_target(
                entity,
                crate::ColorStyleLens {
                    start: crate::CurrentColorStyle {
                        bg: Some(crate::xilem::Color::from_rgb8(0x10, 0x20, 0x30)),
                        text: None,
                        border: None,
                        scale: 1.0,
                    },
                    end: crate::CurrentColorStyle {
                        bg: Some(crate::xilem::Color::from_rgb8(0x40, 0x50, 0x60)),
                        text: None,
                        border: None,
                        scale: 1.0,
                    },
                },
            ),
            TimeRunner::new(duration),
            TimeContext::<()>::default(),
        ));
        world.entity_mut(entity).insert(crate::StyleDirty);

        crate::sync_style_targets(&mut world);

        assert!(world.get::<TimeRunner>(entity).is_some());
        assert!(
            world
                .get::<ComponentTween<crate::ColorStyleLens>>(entity)
                .is_some()
        );
    }
    #[test]
    fn resolve_style_for_classes_applies_font_family() {
        let mut world = World::new();
        let mut sheet = StyleSheet::default();

        sheet.set_class(
            "cjk-text",
            StyleSetter {
                font_family: Some(vec![
                    "Primary Family".to_string(),
                    "Fallback Family".to_string(),
                ]),
                ..StyleSetter::default()
            },
        );
        world.insert_resource(sheet);

        let resolved = crate::resolve_style_for_classes(&world, ["cjk-text"]);
        assert_eq!(
            resolved.font_family,
            Some(vec![
                "Primary Family".to_string(),
                "Fallback Family".to_string()
            ])
        );
    }
    #[test]
    fn stylesheet_font_family_accepts_css_stack_and_applies_as_default() {
        let sheet = crate::parse_stylesheet_ron(
            r#"(
                font_family: (Var: "font-family-base"),
                tokens: {
                    "font-family-base": FontFamily("'Segoe UI', 'Segoe UI Web (West European)', -apple-system, BlinkMacSystemFont, Roboto, 'Helvetica Neue', sans-serif"),
                },
            )"#,
        )
        .expect("stylesheet font stack should parse");

        assert_eq!(
            sheet.tokens.get("font-family-base"),
            Some(&crate::TokenValue::FontFamily(vec![
                "Segoe UI".to_string(),
                "Segoe UI Web (West European)".to_string(),
                "system-ui".to_string(),
                "Roboto".to_string(),
                "Helvetica Neue".to_string(),
                "sans-serif".to_string(),
            ]))
        );

        let mut world = World::new();
        let entity = world.spawn_empty().id();
        world.insert_resource(sheet);

        assert_eq!(
            resolve_style(&world, entity).font_family,
            Some(vec![
                "Segoe UI".to_string(),
                "Segoe UI Web (West European)".to_string(),
                "system-ui".to_string(),
                "Roboto".to_string(),
                "Helvetica Neue".to_string(),
                "sans-serif".to_string(),
            ])
        );
    }
    #[test]
    fn stylesheet_rule_font_family_accepts_css_stack_string() {
        let sheet = crate::parse_stylesheet_ron(
            r#"(
                rules: [
                    (
                        selector: Class("code"),
                        setter: (
                            font_family: "Consolas, 'Courier New', Courier, monospace",
                        ),
                    ),
                ],
            )"#,
        )
        .expect("rule font stack should parse");

        let mut world = World::new();
        world.insert_resource(sheet);

        assert_eq!(
            resolve_style_for_classes(&world, ["code"]).font_family,
            Some(vec![
                "Consolas".to_string(),
                "Courier New".to_string(),
                "Courier".to_string(),
                "monospace".to_string(),
            ])
        );
    }
    #[test]
    fn stylesheet_rule_font_family_accepts_token_reference() {
        let sheet = crate::parse_stylesheet_ron(
            r#"(
                tokens: {
                    "font-family-code": FontFamily("Consolas, monospace"),
                },
                rules: [
                    (
                        selector: Class("code"),
                        setter: (
                            font_family: (Var: "font-family-code"),
                        ),
                    ),
                ],
            )"#,
        )
        .expect("rule font stack token should parse");

        let mut world = World::new();
        world.insert_resource(sheet);

        assert_eq!(
            resolve_style_for_classes(&world, ["code"]).font_family,
            Some(vec!["Consolas".to_string(), "monospace".to_string()])
        );
    }
    #[test]
    fn fluent_theme_resolves_base_font_family_for_type_ramp() {
        let variants = crate::parse_stylesheet_variants_ron(crate::BUILTIN_FLUENT_THEME_RON)
            .expect("embedded Fluent theme should parse");
        let dark = variants
            .variants
            .get("dark")
            .cloned()
            .expect("dark Fluent variant should exist");

        let mut world = World::new();
        world.insert_resource(dark);

        let resolved = resolve_style_for_classes(&world, ["type.body1"]);
        assert_eq!(
            resolved.font_family,
            Some(vec![
                "Segoe UI".to_string(),
                "Segoe UI Web (West European)".to_string(),
                "system-ui".to_string(),
                "Roboto".to_string(),
                "Helvetica Neue".to_string(),
                "sans-serif".to_string(),
            ])
        );
    }
    #[test]
    fn computed_style_lens_keeps_font_family_until_completion() {
        let mut world = World::new();

        let start = crate::ComputedStyle {
            font_family: Some(vec!["Family A".to_string()]),
            ..crate::ComputedStyle::default()
        };
        let end = crate::ComputedStyle {
            font_family: Some(vec!["Family B".to_string()]),
            ..crate::ComputedStyle::default()
        };

        let entity = world.spawn((start.clone(),)).id();
        let lens = crate::ComputedStyleLens {
            start: start.clone(),
            end: end.clone(),
        };

        {
            let target = world
                .get_mut::<crate::ComputedStyle>(entity)
                .expect("computed style should exist");
            lens.interpolate(target.into_inner(), 0.5, 0.0);
        }

        assert_eq!(
            world
                .get::<crate::ComputedStyle>(entity)
                .and_then(|style| style.font_family.clone()),
            Some(vec!["Family A".to_string()])
        );

        {
            let target = world
                .get_mut::<crate::ComputedStyle>(entity)
                .expect("computed style should exist");
            lens.interpolate(target.into_inner(), 1.0, 0.0);
        }

        assert_eq!(
            world
                .get::<crate::ComputedStyle>(entity)
                .and_then(|style| style.font_family.clone()),
            Some(vec!["Family B".to_string()])
        );
    }
    #[test]
    fn register_i18n_bundle_stores_locale_font_stacks_in_app_i18n() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .register_i18n_bundle(
                "en-US",
                SyncTextSource::String(include_str!("../../../assets/locales/en-US/main.ftl")),
                vec!["Inter", "sans-serif"],
            )
            .register_i18n_bundle(
                "zh-CN",
                SyncTextSource::String(include_str!("../../../assets/locales/zh-CN/main.ftl")),
                vec!["Inter", "sans-serif"],
            );

        {
            let i18n = app.world().resource::<AppI18n>();
            assert_eq!(
                i18n.get_font_stack(),
                vec!["Inter".to_string(), "sans-serif".to_string()]
            );
        }

        app.world_mut().resource_mut::<AppI18n>().set_active_locale(
            "zh-CN"
                .parse()
                .expect("zh-CN locale identifier should parse"),
        );
        {
            let i18n = app.world().resource::<AppI18n>();
            assert_eq!(
                i18n.get_font_stack(),
                vec![
                    "Inter".to_string(),
                    "sans-serif".to_string()
                ]
            );
        }

        app.world_mut().resource_mut::<AppI18n>().set_active_locale(
            "ja-JP"
                .parse()
                .expect("ja-JP locale identifier should parse"),
        );
        assert_eq!(
            app.world().resource::<AppI18n>().get_font_stack(),
            vec!["Inter".to_string(), "sans-serif".to_string()]
        );
    }
    #[test]
    fn stylesheet_ron_parser_supports_tokens_and_var_values() {
        let ron = r##"(
        tokens: {
            "demo-bg": Color(Hex("#112233")),
            "radius": Float(6.0),
        },
      rules: [
        (
          selector: Class("demo.button"),
          setter: (
                                    layout: (
                                            padding: 10.0,
                                            corner_radius: Var("radius"),
                                            justify_content: Start,
                                            align_items: Center,
                                            scale: 0.97,
                                    ),
                                    text: (text_align: Center),
                                    colors: (bg: Var("demo-bg"), text: Hex("#f0f0f0")),
          ),
        ),
        (
          selector: And([Class("demo.button"), PseudoClass(Hovered)]),
          setter: (
                    colors: (hover_bg: Hex("#112233ff")),
          ),
        ),
      ],
    )"##;

        let sheet = crate::styling::parse_stylesheet_ron_for_tests(ron)
            .expect("stylesheet ron should parse");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.tokens.len(), 2);

        assert!(matches!(
            &sheet.rules[0].selector,
            crate::Selector::Class(name) if name == "demo.button"
        ));
        assert!(sheet.rules[0].setter.layout.padding.is_some());
        assert!(sheet.rules[0].setter.colors.bg.is_some());
        assert!(sheet.rules[0].setter.layout.justify_content.is_some());
        assert!(sheet.rules[0].setter.layout.align_items.is_some());
        assert!(sheet.rules[0].setter.layout.scale.is_some());
        assert!(sheet.rules[0].setter.text.text_align.is_some());
        assert!(matches!(
            sheet.rules[0].setter.colors.bg.as_ref(),
            Some(crate::StyleValue::Var(token)) if token == "demo-bg"
        ));
        assert!(matches!(
            sheet.rules[0].setter.colors.text.as_ref(),
            Some(crate::StyleValue::Value(color))
                if *color == crate::xilem::Color::from_rgb8(0xF0, 0xF0, 0xF0)
        ));

        assert!(
            matches!(&sheet.rules[1].selector, crate::Selector::And(parts) if !parts.is_empty())
        );
        assert!(matches!(
            sheet.rules[1].setter.colors.hover_bg.as_ref(),
            Some(crate::StyleValue::Value(color))
                if *color == crate::xilem::Color::from_rgba8(0x11, 0x22, 0x33, 0xFF)
        ));
    }
    #[test]
    fn stylesheet_hex_literal_for_bg_is_not_treated_as_token_var() {
        let ron = r##"(
        rules: [
            (
                selector: Class("demo.hex"),
                setter: (
                    colors: (bg: Hex("#FFFFFF14")),
                ),
            ),
        ],
    )"##;

        let sheet = crate::styling::parse_stylesheet_ron_for_tests(ron)
            .expect("stylesheet ron should parse");

        assert!(matches!(
            sheet.rules[0].setter.colors.bg.as_ref(),
            Some(crate::StyleValue::Value(color))
                if *color == crate::xilem::Color::from_rgba8(0xFF, 0xFF, 0xFF, 0x14)
        ));
    }
    #[test]
    fn embedded_fluent_theme_color_fields_do_not_parse_hex_literals_as_var_tokens() {
        let assert_not_hex_var = |value: &Option<crate::StyleValue<crate::xilem::Color>>,
                                  field: &str,
                                  selector: &str| {
            if let Some(crate::StyleValue::Var(token)) = value {
                assert!(
                    !token.trim().starts_with('#'),
                    "{selector} {field} parsed as Var token `{token}` but should be a literal color"
                );
            }
        };

        let variants = crate::styling::parse_stylesheet_variants_ron_for_tests(
            crate::styling::BUILTIN_FLUENT_THEME_RON,
        )
        .expect("embedded fluent theme bundle should parse");

        for (variant_name, sheet) in &variants.variants {
            for rule in &sheet.rules {
                let selector = format!("{variant_name}::{:?}", rule.selector);
                let colors = &rule.setter.colors;
                assert_not_hex_var(&colors.bg, "bg", &selector);
                assert_not_hex_var(&colors.text, "text", &selector);
                assert_not_hex_var(&colors.border, "border", &selector);
                assert_not_hex_var(&colors.hover_bg, "hover_bg", &selector);
                assert_not_hex_var(&colors.hover_text, "hover_text", &selector);
                assert_not_hex_var(&colors.hover_border, "hover_border", &selector);
                assert_not_hex_var(&colors.pressed_bg, "pressed_bg", &selector);
                assert_not_hex_var(&colors.pressed_text, "pressed_text", &selector);
                assert_not_hex_var(&colors.pressed_border, "pressed_border", &selector);
            }
        }
    }
    #[test]
    fn stylesheet_var_missing_token_drops_that_declaration() {
        let ron = r##"(
        rules: [
            (
                selector: Class("demo.button"),
                setter: (
                    layout: (padding: Var("missing-padding")),
                    colors: (bg: Var("missing-bg")),
                ),
            ),
        ],
    )"##;

        let sheet = crate::styling::parse_stylesheet_ron_for_tests(ron)
            .expect("stylesheet ron should parse");

        let mut world = World::new();
        world.insert_resource(sheet);

        let entity = world
            .spawn((crate::StyleClass(vec!["demo.button".to_string()]),))
            .id();
        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        let resolved = crate::resolve_style(&world, entity);
        assert_eq!(resolved.layout.padding, 0.0);
        assert_eq!(resolved.colors.bg, None);
    }
    #[test]
    fn stylesheet_box_shadow_token_parses_and_resolves() {
        let ron = r##"(
        tokens: {
            "flyout-shadow": BoxShadow((
                color: Rgba(0.0, 0.0, 0.0, 0.35),
                offset_x: 0.0,
                offset_y: 12.0,
                blur: 24.0,
            )),
        },
        rules: [
            (
                selector: Class("shadowed"),
                setter: (
                    box_shadow: Var("flyout-shadow"),
                ),
            ),
        ],
    )"##;

        let sheet = crate::styling::parse_stylesheet_ron_for_tests(ron)
            .expect("stylesheet ron should parse");

        let mut world = World::new();
        world.insert_resource(sheet);
        let entity = world
            .spawn((crate::StyleClass(vec!["shadowed".to_string()]),))
            .id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        let resolved = crate::resolve_style(&world, entity);
        let expected = crate::xilem::style::BoxShadow::new(
            crate::xilem::Color::from_rgba8(0, 0, 0, 89),
            (0.0, 12.0),
        )
        .blur(crate::masonry_core::layout::Length::px(24.0));

        assert_eq!(resolved.box_shadow, Some(expected));
    }

    #[test]
    fn missing_theme_resolves_without_framework_default_colors() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let resolved = crate::resolve_style(&world, entity);
        assert!(
            resolved.colors.bg.is_none(),
            "no theme must not inject a default background color"
        );
        assert!(
            resolved.colors.text.is_none(),
            "no theme must not inject a default text color"
        );
        assert!(
            resolved.colors.border.is_none(),
            "no theme must not inject a default border color"
        );
    }

    #[test]
    fn partial_theme_leaves_uncovered_components_transparent() {
        let ron = r#"(
            rules: [
                (
                    selector: Class("covered"),
                    setter: (
                        bg: Color((1.0, 0.0, 0.0, 1.0)),
                    ),
                ),
            ],
        )"#;
        let sheet = crate::styling::parse_stylesheet_ron_for_tests(ron)
            .expect("partial stylesheet should parse");

        let mut world = World::new();
        world.insert_resource(sheet);

        let covered = world
            .spawn(crate::StyleClass(vec!["covered".into()]))
            .id();
        let uncovered = world.spawn_empty().id();

        crate::mark_style_dirty(&mut world);
        crate::sync_style_targets(&mut world);

        let uncovered_style = crate::resolve_style(&world, uncovered);
        assert!(
            uncovered_style.colors.bg.is_none() && uncovered_style.colors.text.is_none(),
            "uncovered entities keep empty/transparent resolved colors"
        );

        // Covered class may match; partial themes must load without structural error.
        let _ = crate::resolve_style(&world, covered);
        let _ = resolve_style_for_classes(&world, ["covered"]);
    }

    #[test]
    fn invalid_stylesheet_ron_is_structural_error() {
        let err = crate::parse_stylesheet_ron("not valid ron {{{")
            .expect_err("invalid RON must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}

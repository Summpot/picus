use super::{
    core::{BuiltinUiAction, ProjectionCtx, UiView},
    utils::{VectorIcon, localized_font_stack, translate_text, vector_icon},
};
use crate::{
    ecs::{
        ButtonAppearance, ButtonIconPosition, ButtonShape, ButtonSize, LocalizeText,
        TypographyPreset, UiAvatar, UiBadge, UiButton, UiCheckbox, UiImage, UiLabel, UiLink,
        UiMultilineTextInput, UiNumericUpDown, UiPasswordInput, UiProgressBar, UiRating, UiSlider,
        UiSwitch, UiText, UiTextInput,
    },
    i18n::resolve_localized_text,
    icons::IconGlyph,
    retained_bridge::{button_view, button_with_child_view, slider_view, text_input_view},
    styling::{
        ResolvedStyle, apply_direct_widget_style, apply_label_style, apply_widget_style,
        font_stack_from_style, resolve_style, resolve_style_for_entity_classes,
    },
    widget_actions::WidgetUiAction,
};
use bevy_ecs::prelude::*;
use crate::masonry_core::{
    layout::{Dim, Length, UnitPoint},
    properties::Padding,
};
use picus_view::picus_widget::widgets::InsertNewline;
use picus_view::style::Style as _;
use picus_view::view::{
    CrossAxisAlignment, FlexExt as _, flex_row, image as xilem_image, label, sized_box,
    transformed, zstack,
};
use std::sync::Arc;
use tracing::trace;

const CHECKBOX_BOX_SIZE: f64 = 16.0;
const CHECKBOX_MARK_SIZE: f64 = 12.0;
const SWITCH_TRACK_WIDTH: f64 = 40.0;
const SWITCH_TRACK_HEIGHT: f64 = 20.0;
const SWITCH_THUMB_SIZE: f64 = 18.0;
const SWITCH_THUMB_MARGIN: f64 = (SWITCH_TRACK_HEIGHT - SWITCH_THUMB_SIZE) * 0.5;
// WinUI ProgressBar geometry (ProgressBar_themeresources.xaml):
// MinHeight=3, TrackHeight=1, CornerRadius=1.5. Distinct from Slider
// (4px track + 18px dual thumb on a 32px hit target).
//
// IMPORTANT: always set size with `.dims((w, h))`. Chaining
// `.width(...).height(...)` replaces Dimensions and resets the other axis
// to Auto, which collapses empty-label progress boxes to ~0 width.
const PROGRESS_BAR_WIDTH: f64 = 240.0;
const PROGRESS_BAR_HEIGHT: f64 = 3.0;
const PROGRESS_BAR_TRACK_HEIGHT: f64 = 1.0;
const PROGRESS_INDETERMINATE_WIDTH: f64 = 80.0;
// WinUI SliderHorizontalHeight — keep composed/wrapped slider hit target.
const SLIDER_HORIZONTAL_HEIGHT: f64 = 32.0;

fn map_text_alignment_for_input(
    text_align: crate::styling::TextAlign,
) -> crate::masonry_core::parley::Alignment {
    match text_align {
        crate::styling::TextAlign::Start => crate::masonry_core::parley::Alignment::Start,
        crate::styling::TextAlign::Center => crate::masonry_core::parley::Alignment::Center,
        crate::styling::TextAlign::End => crate::masonry_core::parley::Alignment::End,
    }
}

fn style_padding(value: f64) -> Padding {
    Padding::all(Length::px(value))
}

fn masked_text(value: &str, mask: char) -> String {
    value.chars().map(|_| mask).collect()
}

pub(crate) fn project_label(label_component: &UiLabel, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let text = resolve_localized_text(ctx.world, ctx.entity, &label_component.text);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }
    let localization_key = ctx
        .world
        .get::<LocalizeText>(ctx.entity)
        .map(|localize| localize.key.as_str());
    trace!(
        entity = ?ctx.entity,
        localization_key = ?localization_key,
        fallback_text = %label_component.text,
        resolved_text = %text,
        "projected UiLabel text"
    );
    Arc::new(apply_label_style(label(text), &style))
}

/// Render an icon glyph in a fixed-size box.
fn create_icon_view(
    icon: impl Into<IconGlyph>,
    size_px: f64,
    color: Option<crate::xilem::Color>,
) -> UiView {
    let icon = icon.into();
    let mut icon_style = ResolvedStyle::default();
    icon_style.colors.text = color;
    icon_style.text.size = (size_px * 0.90) as f32;
    icon_style.font_family = Some(icon.font_family_vec());

    Arc::new(
        sized_box(apply_label_style(
            label(icon.glyph().to_string()),
            &icon_style,
        ))
        .width(Dim::Fixed(Length::px(size_px)))
        .height(Dim::Fixed(Length::px(size_px))),
    )
}

pub(crate) fn project_button(button_component: &UiButton, ctx: ProjectionCtx<'_>) -> UiView {
    // Build variant-specific class names from the button's appearance/size/shape.
    let appearance_class = match button_component.appearance {
        ButtonAppearance::Default => "button.appearance.default",
        ButtonAppearance::Primary => "button.appearance.primary",
        ButtonAppearance::Outline => "button.appearance.outline",
        ButtonAppearance::Subtle => "button.appearance.subtle",
        ButtonAppearance::Transparent => "button.appearance.transparent",
    };
    let size_class = match button_component.size {
        ButtonSize::Small => "button.size.small",
        ButtonSize::Medium => "button.size.medium",
        ButtonSize::Large => "button.size.large",
    };
    let shape_class = match button_component.shape {
        ButtonShape::Rounded => "button.shape.rounded",
        ButtonShape::Circular => "button.shape.circular",
        ButtonShape::Square => "button.shape.square",
    };

    // Resolve style including the variant classes so theme selectors match.
    // When disabled, also include `button.disabled` for dimmed/non-interactive styling.
    let mut style = if button_component.disabled {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            [appearance_class, size_class, shape_class, "button.disabled"],
        )
    } else {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            [appearance_class, size_class, shape_class],
        )
    };
    let button_label_text = resolve_localized_text(ctx.world, ctx.entity, &button_component.label);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }
    let localization_key = ctx
        .world
        .get::<LocalizeText>(ctx.entity)
        .map(|localize| localize.key.as_str());
    trace!(
        entity = ?ctx.entity,
        localization_key = ?localization_key,
        fallback_text = %button_component.label,
        resolved_text = %button_label_text,
        disabled = button_component.disabled,
        "projected UiButton label"
    );

    let label_child: UiView = Arc::new(apply_label_style(label(button_label_text), &style));

    let content = if let Some(icon) = button_component.icon {
        let icon_size = match button_component.size {
            ButtonSize::Small => 16.0,
            ButtonSize::Medium => 20.0,
            ButtonSize::Large => 24.0,
        };
        let icon_view = create_icon_view(icon, icon_size, style.colors.text);
        match button_component.icon_position {
            ButtonIconPosition::Before => Arc::new(
                flex_row(vec![icon_view.into_any_flex(), label_child.into_any_flex()])
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(Length::px(8.0)),
            ) as UiView,
            ButtonIconPosition::After => Arc::new(
                flex_row(vec![label_child.into_any_flex(), icon_view.into_any_flex()])
                    .cross_axis_alignment(CrossAxisAlignment::Center)
                    .gap(Length::px(8.0)),
            ) as UiView,
            ButtonIconPosition::IconOnly => icon_view,
        }
    } else {
        label_child
    };

    if button_component.disabled {
        // Disabled buttons render as a styled non-interactive container so they
        // never emit click actions, accept focus, or respond to hover/press.
        Arc::new(apply_direct_widget_style(content, &style))
    } else if let Some(emit) = ctx.world.get::<crate::UiEmit>(ctx.entity) {
        Arc::new(apply_direct_widget_style(
            crate::retained_bridge::button_with_erased_child(
                ctx.entity,
                emit.type_id(),
                emit.payload(),
                content,
            ),
            &style,
        ))
    } else {
        Arc::new(apply_direct_widget_style(
            button_with_child_view(ctx.entity, BuiltinUiAction::Clicked, content),
            &style,
        ))
    }
}

pub(crate) fn project_badge(badge_component: &UiBadge, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let text = translate_text(
        ctx.world,
        badge_component.text_key.as_deref(),
        &badge_component.text,
    );

    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    Arc::new(apply_widget_style(
        apply_label_style(label(text), &style),
        &style,
    ))
}

pub(crate) fn project_checkbox(checkbox: &UiCheckbox, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let label_text = resolve_localized_text(ctx.world, ctx.entity, &checkbox.label);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let box_style = if checkbox.indeterminate {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            [
                "template.checkbox.box",
                "template.checkbox.box.indeterminate",
            ],
        )
    } else if checkbox.checked {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            ["template.checkbox.box", "template.checkbox.box.checked"],
        )
    } else {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.checkbox.box"])
    };
    let mark_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.checkbox.mark"]);
    let mark_color = mark_style.colors.text.or(style.colors.text);
    let mark_size = (mark_style.text.size as f64).clamp(10.0, CHECKBOX_MARK_SIZE);

    let box_layer: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .dims((Length::px(CHECKBOX_BOX_SIZE), Length::px(CHECKBOX_BOX_SIZE))),
        &box_style,
    ));
    let mut indicator_layers = vec![box_layer];
    if checkbox.indeterminate
        && let Some(mark_color) = mark_color
    {
        // Indeterminate renders a horizontal dash instead of a check mark.
        indicator_layers.push(vector_icon(VectorIcon::Minus, mark_size, mark_color));
    } else if checkbox.checked
        && let Some(mark_color) = mark_color
    {
        indicator_layers.push(vector_icon(VectorIcon::Check, mark_size, mark_color));
    }
    let indicator = zstack(indicator_layers).alignment(UnitPoint::CENTER);
    let label_child = apply_label_style(label(label_text), &style);

    let content = flex_row(vec![indicator.into_any_flex(), label_child.into_any_flex()])
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(style.layout.gap));

    Arc::new(apply_direct_widget_style(
        button_with_child_view(
            ctx.entity,
            WidgetUiAction::ToggleCheckbox {
                checkbox: ctx.entity,
            },
            content,
        ),
        &style,
    ))
}

pub(crate) fn project_slider(slider: &UiSlider, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    // WinUI Slider is a transparent hit-target around a 4px track + dual thumb.
    // Stretch width so flex columns assign a real track length; fixed height is
    // the WinUI 32px horizontal hit target. Theme keeps bg/border transparent.
    Arc::new(apply_widget_style(
        sized_box(
            slider_view(
                ctx.entity,
                slider.min,
                slider.max,
                slider.value,
                move |value| WidgetUiAction::SetSliderValue {
                    slider: ctx.entity,
                    value,
                },
            )
            .step(slider.step),
        )
        .dims((
            Dim::Stretch,
            Dim::Fixed(Length::px(SLIDER_HORIZONTAL_HEIGHT)),
        )),
        &style,
    ))
}

pub(crate) fn project_numeric_up_down(numeric: &UiNumericUpDown, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let dec_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["numericUpDown.decrease"]);
    let inc_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["numericUpDown.increase"]);
    let value_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["numericUpDown.value"]);

    let value_text = numeric.formatted_value();
    let value_label = apply_label_style(label(value_text), &value_style);

    let dec_btn: UiView = Arc::new(apply_direct_widget_style(
        button_view(
            ctx.entity,
            WidgetUiAction::StepNumericUpDown {
                numeric: ctx.entity,
                delta: -1.0,
            },
            "−",
        ),
        &dec_style,
    ));
    let inc_btn: UiView = Arc::new(apply_direct_widget_style(
        button_view(
            ctx.entity,
            WidgetUiAction::StepNumericUpDown {
                numeric: ctx.entity,
                delta: 1.0,
            },
            "+",
        ),
        &inc_style,
    ));

    let content = flex_row(vec![
        dec_btn.into_any_flex(),
        Arc::new(apply_widget_style(value_label, &value_style)).into_any_flex(),
        inc_btn.into_any_flex(),
    ])
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(Length::px(style.layout.gap));

    Arc::new(apply_direct_widget_style(content, &style))
}

pub(crate) fn project_switch(switch_component: &UiSwitch, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let track_style = if switch_component.on {
        resolve_style_for_entity_classes(
            ctx.world,
            ctx.entity,
            ["template.switch.track", "template.switch.track.on"],
        )
    } else {
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.switch.track"])
    };
    let thumb_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.switch.thumb"]);
    let thumb_x = if switch_component.on {
        SWITCH_TRACK_WIDTH - SWITCH_THUMB_SIZE - SWITCH_THUMB_MARGIN
    } else {
        SWITCH_THUMB_MARGIN
    };

    let track: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .dims((Length::px(SWITCH_TRACK_WIDTH), Length::px(SWITCH_TRACK_HEIGHT))),
        &track_style,
    ));
    let thumb: UiView = Arc::new(apply_widget_style(
        sized_box(label(""))
            .dims((Length::px(SWITCH_THUMB_SIZE), Length::px(SWITCH_THUMB_SIZE))),
        &thumb_style,
    ));
    let switch_visual: UiView = Arc::new(
        zstack(vec![
            track,
            Arc::new(transformed(thumb).translate((thumb_x, SWITCH_THUMB_MARGIN))),
        ])
        .alignment(UnitPoint::TOP_LEFT),
    );

    let mut items = vec![switch_visual.into_any_flex()];
    if let Some(label_text) = switch_component
        .label
        .as_ref()
        .filter(|label| !label.is_empty())
        .map(|label| resolve_localized_text(ctx.world, ctx.entity, label))
    {
        items.push(apply_label_style(label(label_text), &style).into_any_flex());
    }

    let content = flex_row(items)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(Length::px(style.layout.gap));

    Arc::new(apply_direct_widget_style(
        button_with_child_view(
            ctx.entity,
            WidgetUiAction::ToggleSwitch { switch: ctx.entity },
            content,
        ),
        &style,
    ))
}

pub(crate) fn project_progress_bar(progress: &UiProgressBar, ctx: ProjectionCtx<'_>) -> UiView {
    // WinUI: 1px ControlStrongStroke track + 3px Accent fill indicator.
    // Sized with `.dims((w, h))` so fixed width is not clobbered by a second
    // Dimensions prop (`.width().height()` leaves width as Auto → 0 for empty
    // label boxes, which left the Progress card looking like a lone white
    // slider thumb).
    let track_style =
        resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.progress.track"]);
    let (fill_width, fill_offset, fill_style) = match progress.progress {
        Some(value) => (
            PROGRESS_BAR_WIDTH * value.clamp(0.0, 1.0),
            0.0,
            resolve_style_for_entity_classes(ctx.world, ctx.entity, ["template.progress.fill"]),
        ),
        None => (
            PROGRESS_INDETERMINATE_WIDTH,
            (PROGRESS_BAR_WIDTH - PROGRESS_INDETERMINATE_WIDTH) * 0.35,
            resolve_style_for_entity_classes(
                ctx.world,
                ctx.entity,
                ["template.progress.fill", "template.progress.indeterminate"],
            ),
        ),
    };

    let track_y = (PROGRESS_BAR_HEIGHT - PROGRESS_BAR_TRACK_HEIGHT) * 0.5;
    let track: UiView = Arc::new(apply_widget_style(
        sized_box(label("")).dims((
            Length::px(PROGRESS_BAR_WIDTH),
            Length::px(PROGRESS_BAR_TRACK_HEIGHT),
        )),
        &track_style,
    ));
    let track_layer: UiView = if track_y > 0.0 {
        Arc::new(transformed(track).translate((0.0, track_y)))
    } else {
        track
    };

    let fill: UiView = Arc::new(apply_widget_style(
        sized_box(label("")).dims((
            Length::px(fill_width.max(0.0)),
            Length::px(PROGRESS_BAR_HEIGHT),
        )),
        &fill_style,
    ));
    let fill_layer = if fill_offset > 0.0 {
        Arc::new(transformed(fill).translate((fill_offset, 0.0))) as UiView
    } else {
        fill
    };

    Arc::new(apply_widget_style(
        sized_box(zstack(vec![track_layer, fill_layer]).alignment(UnitPoint::TOP_LEFT)).dims((
            Length::px(PROGRESS_BAR_WIDTH),
            Length::px(PROGRESS_BAR_HEIGHT),
        )),
        &resolve_style(ctx.world, ctx.entity),
    ))
}

const RATING_FILLED_STAR: &str = "\u{2605}"; // ★
const RATING_OUTLINE_STAR: &str = "\u{2606}"; // ☆

pub(crate) fn project_rating(rating: &UiRating, ctx: ProjectionCtx<'_>) -> UiView {
    let entity = ctx.entity;
    let _font_size = rating.size.star_font_size();
    let max_stars = rating.max.max(1);
    let current_value = rating.value;

    let _star_color = crate::xilem::Color::from_rgb8(0xE3, 0xA9, 0x5C);

    let mut star_views: Vec<UiView> = Vec::with_capacity(max_stars as usize);

    for i in 1..=max_stars {
        let star_value = f64::from(i);
        let is_filled = star_value <= current_value;

        let star_char = if is_filled {
            RATING_FILLED_STAR
        } else {
            RATING_OUTLINE_STAR
        };

        let action = WidgetUiAction::RatingChanged {
            rating: entity,
            value: star_value,
        };

        let star_view: UiView = Arc::new(button_view(entity, action, star_char));
        star_views.push(star_view);
    }

    Arc::new(
        flex_row(star_views)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap(Length::px(4.0)),
    )
}

pub(crate) fn project_text_input(input: &UiTextInput, ctx: ProjectionCtx<'_>) -> UiView {
    project_text_input_view(
        ctx,
        input.value.clone(),
        input.placeholder.clone(),
        true,
        InsertNewline::Never,
        false,
        |entity, value| WidgetUiAction::SetTextInput {
            input: entity,
            value,
        },
    )
}

pub(crate) fn project_password_input(input: &UiPasswordInput, ctx: ProjectionCtx<'_>) -> UiView {
    let mask = input.mask;
    project_text_input_view(
        ctx,
        masked_text(&input.value, mask),
        input.placeholder.clone(),
        true,
        InsertNewline::Never,
        input.read_only,
        move |entity, display_value| WidgetUiAction::SetPasswordInputDisplay {
            input: entity,
            display_value,
        },
    )
}

pub(crate) fn project_multiline_text_input(
    input: &UiMultilineTextInput,
    ctx: ProjectionCtx<'_>,
) -> UiView {
    project_text_input_view(
        ctx,
        input.value.clone(),
        input.placeholder.clone(),
        input.clip,
        InsertNewline::OnEnter,
        input.read_only,
        |entity, value| WidgetUiAction::SetMultilineTextInput {
            input: entity,
            value,
        },
    )
}

fn project_text_input_view(
    ctx: ProjectionCtx<'_>,
    value: String,
    placeholder: String,
    clip: bool,
    insert_newline: InsertNewline,
    disabled: bool,
    map_action: impl Fn(Entity, String) -> WidgetUiAction + Send + Sync + 'static,
) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let scale = style.layout.scale.max(0.01);
    let entity = ctx.entity;
    let mut styled = text_input_view(entity, value, move |value| map_action(entity, value))
        .placeholder(placeholder)
        .text_size(style.text.size)
        .text_alignment(map_text_alignment_for_input(style.text.text_align))
        .clip(clip)
        .insert_newline(insert_newline)
        .disabled(disabled);

    if let Some(font_stack) = font_stack_from_style(&style) {
        styled = styled.font(font_stack);
    }

    if let Some(text_color) = style.colors.text {
        return Arc::new(
            transformed(
                styled
                    .text_color(text_color)
                    .placeholder_color(text_color.with_alpha(0.72))
                    .padding(style_padding(style.layout.padding))
                    .corner_radius(Length::px(style.layout.corner_radius))
                    .border(
                        style
                            .colors
                            .border
                            .unwrap_or(crate::xilem::Color::TRANSPARENT),
                        Length::px(style.layout.border_width),
                    )
                    .background_color(style.colors.bg.unwrap_or(crate::xilem::Color::TRANSPARENT))
                    .box_shadow(style.box_shadow.unwrap_or_default()),
            )
            .scale(scale),
        );
    }

    Arc::new(
        transformed(
            styled
                .padding(style_padding(style.layout.padding))
                .corner_radius(Length::px(style.layout.corner_radius))
                .border(
                    style
                        .colors
                        .border
                        .unwrap_or(crate::xilem::Color::TRANSPARENT),
                    Length::px(style.layout.border_width),
                )
                .background_color(style.colors.bg.unwrap_or(crate::xilem::Color::TRANSPARENT))
                .box_shadow(style.box_shadow.unwrap_or_default()),
        )
        .scale(scale),
    )
}

pub(crate) fn project_image(image_component: &UiImage, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let Some(image_brush) = image_component.image_brush() else {
        let fallback_text = image_component.alt_text.clone().unwrap_or_default();
        return Arc::new(apply_widget_style(
            apply_label_style(label(fallback_text), &style),
            &style,
        ));
    };

    let mut image_view = xilem_image(image_brush).decorative(image_component.decorative);
    if let Some(alt_text) = &image_component.alt_text {
        image_view = image_view.alt_text(alt_text.clone());
    }

    Arc::new(apply_widget_style(
        image_view.fit(image_component.fit),
        &style,
    ))
}

const AVATAR_DEFAULT_FONT_SIZE: f32 = 14.0;

pub(crate) fn project_avatar(avatar: &UiAvatar, ctx: ProjectionCtx<'_>) -> UiView {
    let style = resolve_style(ctx.world, ctx.entity);
    let size_f64 = f64::from(avatar.size);
    let corner_radius = avatar.shape.corner_radius_for_size(avatar.size);

    // Resolve colour classes: if a named colour is given, use it;
    // otherwise derive from name hash.
    let color_class = avatar.color.as_deref().unwrap_or_else(|| {
        let idx = crate::pick_avatar_color_index(&avatar.name);
        crate::AVATAR_COLOR_CLASSES[idx]
    });

    let color_style = resolve_style_for_entity_classes(ctx.world, ctx.entity, [color_class]);

    // Get an appropriate font size: ~40% of avatar size, clamped.
    let font_size = (size_f64 as f32 * 0.40).clamp(8.0, AVATAR_DEFAULT_FONT_SIZE * 2.0);

    let mut avatar_style = style.clone();
    avatar_style.colors.bg = color_style.colors.bg.or(style.colors.bg);
    avatar_style.layout.corner_radius = corner_radius;

    let mut initials_style = style.clone();
    initials_style.colors.text = color_style.colors.text.or(style.colors.text);
    initials_style.text.size = font_size;
    initials_style.text.weight = 600.0;

    let initials = crate::get_initials(&avatar.name);

    let avatar_view: UiView = Arc::new(
        sized_box(apply_widget_style(
            zstack(vec![Arc::new(apply_label_style(
                label(initials),
                &initials_style,
            ))])
            .alignment(crate::masonry_core::layout::UnitPoint::CENTER),
            &avatar_style,
        ))
        .width(Dim::Fixed(Length::px(size_f64)))
        .height(Dim::Fixed(Length::px(size_f64))),
    );

    avatar_view
}

/// Project a `UiLink` component as interactive link text.
///
/// The link emits a `UiLinkAction` on click. Styling (color, underline) is
/// controlled by the theme engine through the `UiLink` selector rules.
pub(crate) fn project_link(link_component: &UiLink, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let text = resolve_localized_text(ctx.world, ctx.entity, &link_component.text);
    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    let label_child = apply_label_style(label(text), &style);

    Arc::new(apply_direct_widget_style(
        button_with_child_view(
            ctx.entity,
            crate::UiLinkAction::new(ctx.entity),
            label_child,
        ),
        &style,
    ))
}

/// Project a `UiText` component into a label with a typography preset.
///
/// The entity can carry a standalone `TypographyPreset` component; if present,
/// its class name is added to the entity's `StyleClass` list so the theme
/// engine applies the correct font-size / weight / line-height.
pub(crate) fn project_text(text_component: &UiText, ctx: ProjectionCtx<'_>) -> UiView {
    let mut style = resolve_style(ctx.world, ctx.entity);
    let display_text = resolve_localized_text(ctx.world, ctx.entity, &text_component.text);

    // Determine the typography preset: prefer an explicit one on the component,
    // then check for a separate TypographyPreset component, finally default.
    let preset = text_component
        .preset
        .or_else(|| ctx.world.get::<TypographyPreset>(ctx.entity).copied())
        .unwrap_or(TypographyPreset::Body1);

    // Resolve style for the type.* class and merge into the entity style.
    let class_name = preset.class_name();
    let resolved = resolve_style_for_entity_classes(ctx.world, ctx.entity, [class_name]);
    if let Some(color) = resolved.colors.text {
        style.colors.text = Some(color);
    }
    style.text.size = resolved.text.size;
    style.text.weight = resolved.text.weight;
    style.text.line_height = resolved.text.line_height;
    if let Some(font_family) = resolved.font_family {
        style.font_family = Some(font_family);
    }

    if let Some(stack) = localized_font_stack(ctx.world, ctx.entity) {
        style.font_family = Some(stack);
    }

    Arc::new(apply_label_style(label(display_text), &style))
}

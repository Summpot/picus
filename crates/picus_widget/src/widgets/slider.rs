use std::any::TypeId;

use accesskit::{ActionData, Node, Orientation, Role};
use tracing::{Span, trace_span};

use crate::core::keyboard::{Key, NamedKey};
use crate::core::pointer::PointerButton;
use crate::core::{
    AccessCtx, AccessEvent, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, PaintCtx,
    PointerButtonEvent, PointerEvent, PointerUpdate, PrePaintProps, PropertiesMut, PropertiesRef,
    Property, RegisterCtx, TextEvent, Update, UpdateCtx, UsesProperty, Widget, WidgetId, WidgetMut,
    paint_background, paint_box_shadow,
};
use crate::imaging::{Composite, GroupRef, Painter};
use crate::kurbo::{Axis, Circle, Rect, Size, Stroke};
use crate::layout::{LenReq, Length};
use crate::peniko;
use crate::properties::{
    ThumbColor, ThumbRadius, TrackColor, TrackThickness, paint_border_brush, resolve_border_brush,
};
use crate::theme;

/// A widget that allows a user to select a value from a continuous range.
///
#[doc = concat!(
    "![Slider](",
    "screenshots/slider_initial_state.png",
    ")",
)]
pub struct Slider {
    // --- Logic ---
    min: f64,
    max: f64,
    value: f64,
    step: Option<f64>,
}

// --- MARK: BUILDERS
impl Slider {
    /// Creates a new `Slider`.
    pub fn new(min: f64, max: f64, value: f64) -> Self {
        Self {
            min,
            max,
            value: value.clamp(min, max),
            step: None,
        }
    }

    /// Configures the stepping interval of the slider.
    pub fn with_step(mut self, step: f64) -> Self {
        self.set_step_internal(Some(step));
        self
    }
}

// --- MARK: METHODS
impl Slider {
    fn set_step_internal(&mut self, step: Option<f64>) {
        self.step = step.filter(|s| *s > 0.0);
        let clamped_value = self.value.clamp(self.min, self.max);
        self.value = if let Some(s) = self.step {
            ((clamped_value / s).round() * s).clamp(self.min, self.max)
        } else {
            clamped_value
        };
    }

    fn update_value_from_position(&mut self, x: f64, width: f64) -> bool {
        let progress = (x / width).clamp(0.0, 1.0);
        let new_value = self.min + progress * (self.max - self.min);
        let old_value = self.value;
        let final_value = if let Some(step) = self.step {
            ((new_value / step).round() * step).clamp(self.min, self.max)
        } else {
            new_value.clamp(self.min, self.max)
        };
        if (final_value - old_value).abs() > f64::EPSILON {
            self.value = final_value;
            true
        } else {
            false
        }
    }
}

// --- MARK: WIDGETMUT
impl Slider {
    /// Sets the current value of the slider.
    pub fn set_value(this: &mut WidgetMut<'_, Self>, value: f64) {
        let clamped_value = value.clamp(this.widget.min, this.widget.max);
        let new_value = if let Some(step) = this.widget.step {
            ((clamped_value / step).round() * step).clamp(this.widget.min, this.widget.max)
        } else {
            clamped_value
        };
        if (new_value - this.widget.value).abs() > f64::EPSILON {
            this.widget.value = new_value;
            this.ctx.request_render();
        }
    }

    /// Sets or removes the stepping interval of the slider.
    pub fn set_step(this: &mut WidgetMut<'_, Self>, step: Option<f64>) {
        let filtered_step = step.filter(|s| *s > 0.0);
        if this.widget.step != filtered_step {
            this.widget.set_step_internal(filtered_step);
            this.ctx.request_render();
        }
    }

    /// Sets the range (min and max) of the slider.
    pub fn set_range(this: &mut WidgetMut<'_, Self>, min: f64, max: f64) {
        if this.widget.min != min || this.widget.max != max {
            this.widget.min = min;
            this.widget.max = max;
            Self::set_value(this, this.widget.value);
        }
    }
}

impl UsesProperty<TrackThickness> for Slider {}
impl UsesProperty<TrackColor> for Slider {}
impl UsesProperty<ThumbColor> for Slider {}
impl UsesProperty<ThumbRadius> for Slider {}

/// A slider was moved.
#[derive(PartialEq, Debug)]
pub struct SliderMoved {
    /// The new value of the slider.
    pub value: f64,
}

// --- MARK: IMPL WIDGET
impl Widget for Slider {
    type Action = SliderMoved;

    fn accepts_focus(&self) -> bool {
        true
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }
        match event {
            PointerEvent::Down(PointerButtonEvent {
                button: Some(PointerButton::Primary) | None,
                state,
                ..
            }) => {
                ctx.request_focus();
                ctx.capture_pointer();
                let local_pos = ctx.local_position(state.position);
                let width = ctx.content_box().width();
                if self.update_value_from_position(local_pos.x, width) {
                    ctx.submit_action::<Self::Action>(SliderMoved { value: self.value });
                }
            }
            PointerEvent::Move(PointerUpdate { current, .. }) if ctx.is_active() => {
                let local_pos = ctx.local_position(current.position);
                let width = ctx.content_box().width();
                if self.update_value_from_position(local_pos.x, width) {
                    ctx.submit_action::<Self::Action>(SliderMoved { value: self.value });
                }
                ctx.request_render();
            }
            _ => {}
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if ctx.is_disabled() || !ctx.is_focus_target() {
            return;
        }

        if let TextEvent::Keyboard(key_event) = event {
            if key_event.state.is_up() {
                return;
            }

            let mut new_value = self.value;
            let step = self
                .step
                .unwrap_or((self.max - self.min) / 100.0)
                .max(f64::EPSILON);
            let big_step = step * 10.0;

            match &key_event.key {
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowDown) => {
                    new_value -= if key_event.modifiers.shift() {
                        big_step
                    } else {
                        step
                    }
                }
                Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowUp) => {
                    new_value += if key_event.modifiers.shift() {
                        big_step
                    } else {
                        step
                    }
                }
                Key::Named(NamedKey::Home) => new_value = self.min,
                Key::Named(NamedKey::End) => new_value = self.max,
                _ => return,
            }

            if new_value != self.value {
                let clamped_value = new_value.clamp(self.min, self.max);
                let final_value = if let Some(s) = self.step {
                    ((clamped_value / s).round() * s).clamp(self.min, self.max)
                } else {
                    clamped_value
                };

                if (final_value - self.value).abs() > f64::EPSILON {
                    self.value = final_value;
                    ctx.request_render();
                    ctx.submit_action::<Self::Action>(SliderMoved { value: self.value });
                }
            }
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::FocusChanged(_) | Update::HoveredChanged(_) | Update::ActiveChanged(_) => {
                ctx.request_render();
            }
            _ => {}
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if ctx.is_disabled() {
            return;
        }

        let step = self
            .step
            .unwrap_or((self.max - self.min) / 100.0)
            .max(f64::EPSILON);
        let mut new_value = self.value;

        match event.action {
            accesskit::Action::Increment => {
                new_value += step;
            }
            accesskit::Action::Decrement => {
                new_value -= step;
            }
            accesskit::Action::SetValue => match &event.data {
                Some(ActionData::NumericValue(value)) => new_value = *value,
                Some(ActionData::Value(value)) => {
                    if let Ok(value) = value.parse() {
                        new_value = value;
                    }
                }
                _ => {}
            },
            _ => return,
        }

        if (new_value - self.value).abs() > f64::EPSILON {
            let clamped_value = new_value.clamp(self.min, self.max);
            self.value = if let Some(s) = self.step {
                ((clamped_value / s).round() * s).clamp(self.min, self.max)
            } else {
                clamped_value
            };
            ctx.request_render();
            ctx.submit_action::<Self::Action>(SliderMoved { value: self.value });
        }
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn property_changed(&mut self, ctx: &mut UpdateCtx<'_>, property_type: TypeId) {
        TrackThickness::prop_changed(ctx, property_type);
        ThumbColor::prop_changed(ctx, property_type);
        ThumbRadius::prop_changed(ctx, property_type);
        if TrackColor::matches(property_type) {
            ctx.request_paint_only();
        }
    }

    fn measure(
        &mut self,
        ctx: &mut MeasureCtx<'_>,
        props: &PropertiesRef<'_>,
        axis: Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        match axis {
            Axis::Horizontal => match len_req {
                // WinUI Slider default min width is content-driven; 100px is a
                // reasonable floor when unconstrained.
                LenReq::MinContent | LenReq::MaxContent => Length::const_px(100.),
                LenReq::FitContent(space) => space,
            },
            Axis::Vertical => {
                // WinUI `SliderHorizontalHeight` = 32. Keep at least that hit
                // target so the dual-thumb remains comfortably tappable.
                let cache = ctx.property_cache();
                let thumb_radius = props.get::<ThumbRadius>(cache);
                let track_thickness = props.get::<TrackThickness>(cache);

                let thumb_length = thumb_radius.0.saturating_add(thumb_radius.0);
                let track_length = track_thickness.0;
                let content = thumb_length.max(track_length);
                content.max(Length::const_px(theme::SLIDER_HORIZONTAL_HEIGHT))
            }
        }
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

    fn pre_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        // WinUI Slider container is transparent (`SliderContainerBackground` =
        // ControlFillColorTransparent). Only paint an optional focus ring —
        // never a filled chrome box, and never on hover alone.
        let bbox = ctx.border_box();
        let cache = ctx.property_cache();
        let p = PrePaintProps::fetch(props, cache);

        paint_box_shadow(painter, bbox, p.box_shadow, p.corner_radius);
        paint_background(painter, bbox, p.background, p.border_width, p.corner_radius);

        let border_brush = resolve_border_brush(props, ctx.property_cache());
        paint_border_brush(painter, bbox, &border_brush, p.border_width, p.corner_radius);

        if ctx.is_focus_target() {
            let focus_rect = bbox.inset(2.);
            let focus_color = p.border_color.color;
            let focus_path = focus_rect.to_rounded_rect(4.);
            let focus_stroke = Stroke::new(2.).with_miter_limit(10.);
            painter
                .stroke(focus_path, &focus_stroke, focus_color)
                .draw();
        }
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        // WinUI Fluent Slider geometry (from Slider_themeresources.xaml):
        // - Track: 4px tall, corner radius 2, remaining = ControlStrongFill,
        //   filled = AccentFill
        // - Outer thumb: 18×18 solid (ControlSolidFill / fill-thumb) with
        //   elevation border
        // - Inner thumb: accent circle that scales with interaction
        //   (12 normal / 14 hover / 10 pressed)

        let cache = ctx.property_cache();
        let track_color = *props.get::<TrackColor>(cache);
        let outer_thumb_color = props.get::<ThumbColor>(cache).0;
        let track_thickness = props.get::<TrackThickness>(cache).0.get();
        let outer_radius = props.get::<ThumbRadius>(cache).0.get();
        let rim = props.get::<crate::properties::BorderColor>(cache).color;

        // Inner thumb radius matches WinUI scale relative to a 14px base:
        // Normal 12 → 0.86, PointerOver 14 → 1.0, Pressed 10 → 0.71.
        let inner_radius: f64 = if ctx.is_disabled() {
            7.0 // WinUI Disabled keeps the larger (14px) inner thumb
        } else if ctx.is_active() {
            5.0
        } else if ctx.is_hovered() {
            7.0
        } else {
            6.0
        };

        let size = ctx.content_box().size();
        let track_y = (size.height - track_thickness) / 2.0;
        let border_box = ctx.border_box();
        let track_corner = (track_thickness * 0.5).min(2.0);

        // TODO: replace with proper disabled colors
        if ctx.is_disabled() {
            const DISABLED_ALPHA: f32 = 0.4;
            painter.push_fill_clip(border_box);
            painter.push_group(
                GroupRef::new()
                    .with_composite(Composite::new(peniko::BlendMode::default(), DISABLED_ALPHA)),
            );
        }

        let span = (self.max - self.min).max(f64::EPSILON);
        let progress = ((self.value - self.min) / span).clamp(0.0, 1.0);
        let travel = (size.width - outer_radius * 2.).max(0.);
        let thumb_x = outer_radius + progress * travel;
        let thumb_y = size.height / 2.;

        // Active fill ends at the thumb center so the accent track meets the
        // outer thumb without a gap (WinUI DecreaseRect + full TrackRect).
        let track_active_frac = if size.width > f64::EPSILON {
            (thumb_x / size.width).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let track_rect = Rect::new(0., track_y, size.width, track_y + track_thickness)
            .to_rounded_rect(track_corner);
        let gradient = peniko::Gradient::new_linear((0., 0.), (size.width, 0.)).with_stops([
            (0., track_color.active),
            (track_active_frac as f32, track_color.active),
            (track_active_frac as f32, track_color.inactive),
            (1., track_color.inactive),
        ]);
        painter.fill(track_rect, &gradient).draw();

        // Outer thumb (solid fill + elevation-style rim).
        let outer_circle = Circle::new((thumb_x, thumb_y), outer_radius);
        painter.fill(outer_circle, outer_thumb_color).draw();
        painter
            .stroke(outer_circle, &Stroke::new(1.), rim)
            .draw();

        // Inner accent thumb.
        let inner_circle = Circle::new((thumb_x, thumb_y), inner_radius.min(outer_radius - 1.));
        painter.fill(inner_circle, track_color.active).draw();

        if ctx.is_disabled() {
            painter.pop_group();
            painter.pop_clip();
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Slider
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_orientation(Orientation::Horizontal);
        node.set_value(self.value.to_string());
        node.set_numeric_value(self.value);
        node.set_min_numeric_value(self.min);
        node.set_max_numeric_value(self.max);
        if let Some(step) = self.step {
            node.set_numeric_value_step(step);
        }
        node.add_action(accesskit::Action::SetValue);
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("Slider", id = id.trace())
    }
}

// --- MARK: TESTS
#[cfg(any())]
mod tests {
    use super::*;
    use crate::core::TextEvent;
    use crate::kurbo::Point;
    use crate::testing::{TestHarness, assert_render_snapshot};
    use picus_theme_test::test_property_set;

    #[test]
    fn slider_initial_state() {
        let widget = Slider::new(0.0, 100.0, 25.0).prepare();
        let mut harness = TestHarness::create_with_size(test_property_set(), widget, (200, 32));

        assert_render_snapshot!(harness, "slider_initial_state");
    }

    #[test]
    fn slider_drag_interaction() {
        let widget = Slider::new(0.0, 100.0, 25.0).prepare();
        let mut harness = TestHarness::create_with_size(test_property_set(), widget, (200, 32));
        let slider_id = harness.root_id();

        assert_render_snapshot!(harness, "slider_drag_initial_at_25");

        // 1. Move the mouse to the thumb position (25%) BEFORE clicking.
        harness.mouse_move(Point::new(50.0, 16.0));

        // 2. Press the mouse button.
        // This should not emit an action because the value does not change.
        harness.mouse_button_press(None);
        assert!(harness.pop_action::<SliderMoved>().is_none());

        // 3. Move to the new position (75%).
        harness.mouse_move(Point::new(150.0, 16.0));

        assert_eq!(
            harness.pop_action::<SliderMoved>(),
            Some((SliderMoved { value: 75.0 }, slider_id))
        );
        assert_render_snapshot!(harness, "slider_drag_to_75");

        // Release the mouse
        harness.mouse_button_release(None);
        assert_render_snapshot!(harness, "slider_drag_released_at_75");
    }

    #[test]
    fn slider_keyboard_interaction() {
        let widget = Slider::new(0.0, 100.0, 50.0).with_step(10.0).prepare();
        let mut harness = TestHarness::create_with_size(test_property_set(), widget, (200, 32));
        let slider_id = harness.root_id();

        harness.focus_on(Some(slider_id));
        assert_render_snapshot!(harness, "slider_keyboard_focused");

        harness.process_text_event(TextEvent::key_down(Key::Named(NamedKey::ArrowRight)));
        harness.process_text_event(TextEvent::key_up(Key::Named(NamedKey::ArrowRight)));

        assert_eq!(
            harness.pop_action::<SliderMoved>(),
            Some((SliderMoved { value: 60.0 }, slider_id))
        );
        assert_render_snapshot!(harness, "slider_keyboard_moved");
    }

    #[test]
    fn slider_disabled_state() {
        let mut widget = Slider::new(0.0, 100.0, 50.0).prepare();
        widget.options.disabled = true;
        let mut harness = TestHarness::create_with_size(test_property_set(), widget, (200, 32));

        assert_render_snapshot!(harness, "slider_disabled");
        assert!(harness.pop_action::<SliderMoved>().is_none());
    }
}

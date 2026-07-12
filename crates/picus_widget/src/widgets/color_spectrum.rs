use std::any::TypeId;

use accesskit::{Node, Role};
use tracing::{Span, trace_span};

use crate::core::keyboard::{Key, NamedKey};
use crate::core::pointer::{PointerButton, PointerButtonEvent};
use crate::core::{
    AccessCtx, AccessEvent, ChildrenIds, EventCtx, LayoutCtx, MeasureCtx, PaintCtx,
    PointerEvent, PointerUpdate, PrePaintProps, PropertiesMut, PropertiesRef, RegisterCtx,
    TextEvent, Update, UpdateCtx, Widget, WidgetId, WidgetMut,
    paint_background, paint_box_shadow,
};
use crate::imaging::Painter;
use crate::kurbo::{Circle, Rect, Size, Stroke};
use crate::layout::{LenReq, Length};
use crate::peniko;
use crate::properties::{paint_border_brush, resolve_border_brush};

/// The action emitted when the user drags or clicks on the color spectrum.
///
/// `s` is saturation (0..1, left→right), `v` is value/brightness (0..1, bottom→top).
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct ColorSpectrumChanged {
    /// New saturation, 0.0..=1.0.
    pub s: f64,
    /// New value (brightness), 0.0..=1.0.
    pub v: f64,
}

/// A 2-D color spectrum pad for selecting saturation and value at a fixed hue.
///
/// The user can click and drag anywhere on the pad to pick an (s, v) pair.
/// The hue is set externally via [`ColorSpectrum::set_hue`].
pub struct ColorSpectrum {
    hue: f64,
    s: f64,
    v: f64,
}

impl ColorSpectrum {
    /// Creates a new `ColorSpectrum` with the given hue (degrees 0..360),
    /// saturation (0..1), and value (0..1).
    pub fn new(hue: f64, s: f64, v: f64) -> Self {
        Self {
            hue: hue.rem_euclid(360.0),
            s: s.clamp(0.0, 1.0),
            v: v.clamp(0.0, 1.0),
        }
    }
}

// --- MARK: WIDGETMUT
impl ColorSpectrum {
    /// Sets the hue (degrees 0..360). This repaints the spectrum gradient.
    pub fn set_hue(this: &mut WidgetMut<'_, Self>, hue: f64) {
        let hue = hue.rem_euclid(360.0);
        if (this.widget.hue - hue).abs() > f64::EPSILON {
            this.widget.hue = hue;
            this.ctx.request_render();
        }
    }

    /// Sets the saturation and value. This moves the selection indicator.
    pub fn set_sv(this: &mut WidgetMut<'_, Self>, s: f64, v: f64) {
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        if (this.widget.s - s).abs() > f64::EPSILON || (this.widget.v - v).abs() > f64::EPSILON {
            this.widget.s = s;
            this.widget.v = v;
            this.ctx.request_render();
        }
    }
}

impl Widget for ColorSpectrum {
    type Action = ColorSpectrumChanged;

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
                let size = ctx.content_box().size();
                let (s, v) = position_to_sv(local_pos.x, local_pos.y, size.width, size.height);
                let changed = (self.s - s).abs() > f64::EPSILON || (self.v - v).abs() > f64::EPSILON;
                if changed {
                    self.s = s;
                    self.v = v;
                    ctx.submit_action::<Self::Action>(ColorSpectrumChanged { s, v });
                }
            }
            PointerEvent::Move(PointerUpdate { current, .. }) if ctx.is_active() => {
                let local_pos = ctx.local_position(current.position);
                let size = ctx.content_box().size();
                let (s, v) = position_to_sv(local_pos.x, local_pos.y, size.width, size.height);
                let changed = (self.s - s).abs() > f64::EPSILON || (self.v - v).abs() > f64::EPSILON;
                if changed {
                    self.s = s;
                    self.v = v;
                    ctx.submit_action::<Self::Action>(ColorSpectrumChanged { s, v });
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

            let step = 0.01;
            let mut new_s = self.s;
            let mut new_v = self.v;

            match &key_event.key {
                Key::Named(NamedKey::ArrowLeft) => new_s = (new_s - step).max(0.0),
                Key::Named(NamedKey::ArrowRight) => new_s = (new_s + step).min(1.0),
                Key::Named(NamedKey::ArrowDown) => new_v = (new_v - step).max(0.0),
                Key::Named(NamedKey::ArrowUp) => new_v = (new_v + step).min(1.0),
                Key::Named(NamedKey::Home) => {
                    new_s = 0.0;
                    new_v = 1.0;
                }
                Key::Named(NamedKey::End) => {
                    new_s = 1.0;
                    new_v = 0.0;
                }
                _ => return,
            }

            let changed =
                (self.s - new_s).abs() > f64::EPSILON || (self.v - new_v).abs() > f64::EPSILON;
            if changed {
                self.s = new_s;
                self.v = new_v;
                ctx.request_render();
                ctx.submit_action::<Self::Action>(ColorSpectrumChanged {
                    s: self.s,
                    v: self.v,
                });
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

        let step = 0.05;
        let mut new_s = self.s;
        let new_v = self.v;

        match event.action {
            accesskit::Action::Increment => new_s = (new_s + step).min(1.0),
            accesskit::Action::Decrement => new_s = (new_s - step).max(0.0),
            accesskit::Action::SetValue => match &event.data {
                Some(accesskit::ActionData::NumericValue(value)) => {
                    new_s = (*value).clamp(0.0, 1.0);
                }
                Some(accesskit::ActionData::Value(value)) => {
                    if let Ok(value) = value.parse::<f64>() {
                        new_s = value.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            },
            _ => return,
        }

        let _ = new_v; // v is not directly mapped to a single accesskit axis
        let changed = (self.s - new_s).abs() > f64::EPSILON;
        if changed {
            self.s = new_s;
            ctx.request_render();
            ctx.submit_action::<Self::Action>(ColorSpectrumChanged {
                s: self.s,
                v: self.v,
            });
        }
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: TypeId) {}

    fn measure(
        &mut self,
        _ctx: &mut MeasureCtx<'_>,
        _props: &PropertiesRef<'_>,
        _axis: crate::kurbo::Axis,
        len_req: LenReq,
        _cross_length: Option<Length>,
    ) -> Length {
        // Prefer to fill available space; fall back to 200px (square spectrum).
        match len_req {
            LenReq::FitContent(space) => space,
            _ => Length::const_px(200.0),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.set_clip_path(size.to_rect());
    }

    fn pre_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let bbox = ctx.border_box();
        let cache = ctx.property_cache();
        let p = PrePaintProps::fetch(props, cache);

        paint_box_shadow(painter, bbox, p.box_shadow, p.corner_radius);
        paint_background(painter, bbox, p.background, p.border_width, p.corner_radius);

        let border_brush = resolve_border_brush(props, ctx.property_cache());
        paint_border_brush(painter, bbox, &border_brush, p.border_width, p.corner_radius);
    }

    fn paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        painter: &mut Painter<'_>,
    ) {
        let size = ctx.content_box().size();
        let w = size.width;
        let h = size.height;

        // Clip to content box.
        painter.push_fill_clip(ctx.content_box());

        // --- Draw the SV spectrum as a smooth gradient.
        //
        // Horizontal: saturation 0→1 (left white, right pure hue color).
        // Vertical:   value 1→0 (top full color, bottom black).
        //
        // We draw three overlaid gradient rectangles:
        //   1. Pure hue color (full s, full v) as the base.
        //   2. White→transparent horizontal gradient (saturation axis).
        //   3. Transparent→black vertical gradient (value axis).
        let hue_color = hsv_to_color(self.hue, 1.0, 1.0);

        // Base: solid hue color
        painter
            .fill(Rect::new(0.0, 0.0, w, h), hue_color)
            .draw();

        // Saturation gradient: white (left) → transparent (right)
        let sat_gradient = peniko::Gradient::new_linear((0.0, 0.0), (w, 0.0))
            .with_stops([
                (0.0, peniko::Color::WHITE),
                (1.0, peniko::Color::TRANSPARENT),
            ]);
        painter.fill(Rect::new(0.0, 0.0, w, h), &sat_gradient).draw();

        // Value gradient: transparent (top) → black (bottom)
        let val_gradient = peniko::Gradient::new_linear((0.0, 0.0), (0.0, h))
            .with_stops([
                (0.0, peniko::Color::TRANSPARENT),
                (1.0, peniko::Color::BLACK),
            ]);
        painter.fill(Rect::new(0.0, 0.0, w, h), &val_gradient).draw();

        // --- Draw the selection indicator.
        let indicator_x = self.s * w;
        let indicator_y = (1.0 - self.v) * h;
        draw_spectrum_indicator(painter, indicator_x, indicator_y, 7.0);

        painter.pop_clip();
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
        node.set_value(format!("S:{:.2} V:{:.2}", self.s, self.v));
        node.set_numeric_value(self.s);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.add_action(accesskit::Action::SetValue);
        node.add_action(accesskit::Action::Increment);
        node.add_action(accesskit::Action::Decrement);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn make_trace_span(&self, id: WidgetId) -> Span {
        trace_span!("ColorSpectrum", id = id.trace())
    }
}

// --- MARK: HELPERS

/// Convert a pointer position to (s, v) coordinates.
fn position_to_sv(x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
    let s = if w > 0.0 { (x / w).clamp(0.0, 1.0) } else { 0.0 };
    let v = if h > 0.0 { (1.0 - y / h).clamp(0.0, 1.0) } else { 0.0 };
    (s, v)
}

/// Convert HSV (hue degrees, s/v in 0..1) to a peniko Color.
fn hsv_to_color(h: f64, s: f64, v: f64) -> peniko::Color {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let to_u8 = |c: f64| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    peniko::Color::from_rgb8(to_u8(r1), to_u8(g1), to_u8(b1))
}

/// Draw a circular selection indicator (black ring + white inner ring).
fn draw_spectrum_indicator(painter: &mut Painter<'_>, cx: f64, cy: f64, radius: f64) {
    painter
        .stroke(
            Circle::new((cx, cy), radius),
            &Stroke::new(2.0),
            peniko::Color::BLACK,
        )
        .draw();
    painter
        .stroke(
            Circle::new((cx, cy), radius - 2.0),
            &Stroke::new(1.5),
            peniko::Color::WHITE,
        )
        .draw();
}
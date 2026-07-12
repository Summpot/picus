//! Common widgets.

#![expect(
    missing_debug_implementations,
    reason = "Widgets are not expected to implement Debug"
)]

mod align;
mod animated_f32;
mod badge;
mod badged;
mod button;
mod canvas;
mod color_spectrum;
mod checkbox;
mod divider;
mod flex;
mod grid;
mod image;
mod label;
mod passthrough;
mod portal;
mod progress_bar;
mod prose;
mod radio_button;
mod radio_group;
mod scroll_bar;
mod sized_box;
mod slider;
mod spinner;
mod split;
mod step_input;
mod switch;
mod text_area;
mod text_input;
mod virtual_scroll;
mod zstack;

// TODO - Split off widgets and other exports?
// (e.g. actions, param types)

pub use self::align::*;
pub(crate) use self::animated_f32::*;
pub use self::badge::*;
pub use self::badged::*;
pub use self::button::*;
pub use self::canvas::*;
pub use self::checkbox::*;
pub use self::color_spectrum::*;
pub use self::divider::*;
pub use self::flex::*;
pub use self::grid::*;
pub use self::image::*;
pub use self::label::*;
pub use self::passthrough::*;
pub use self::portal::*;
pub use self::progress_bar::*;
pub use self::prose::*;
pub use self::radio_button::*;
pub use self::radio_group::*;
pub use self::scroll_bar::*;
pub use self::sized_box::*;
pub use self::slider::*;
pub use self::spinner::*;
pub use self::split::*;
pub use self::step_input::*;
pub use self::switch::*;
pub use self::text_area::*;
pub use self::text_input::*;
pub use self::virtual_scroll::*;
pub use self::zstack::*;

// --- BorderBrush UsesProperty impls ---
//
// We cannot use a blanket `impl<W: Widget> UsesProperty<BorderBrush> for W`
// because Rust's orphan rules forbid it (uncovered type parameter `W` before
// the local type `BorderBrush`). Instead, impl for each concrete widget type.

use crate::core::{UsesProperty, Widget};
use crate::properties::BorderBrush;

// Simple (non-generic) widgets:
impl UsesProperty<BorderBrush> for Align {}
impl UsesProperty<BorderBrush> for Badge {}
impl UsesProperty<BorderBrush> for Button {}
impl UsesProperty<BorderBrush> for Canvas {}
impl UsesProperty<BorderBrush> for Checkbox {}
impl UsesProperty<BorderBrush> for ColorSpectrum {}
impl UsesProperty<BorderBrush> for Divider {}
impl UsesProperty<BorderBrush> for Grid {}
impl UsesProperty<BorderBrush> for Label {}
impl UsesProperty<BorderBrush> for Passthrough {}
impl UsesProperty<BorderBrush> for ProgressBar {}
impl UsesProperty<BorderBrush> for Prose {}
impl UsesProperty<BorderBrush> for RadioButton {}
impl UsesProperty<BorderBrush> for RadioGroup {}
impl UsesProperty<BorderBrush> for ScrollBar {}
impl UsesProperty<BorderBrush> for SizedBox {}
impl UsesProperty<BorderBrush> for Slider {}
impl UsesProperty<BorderBrush> for Spinner {}
impl UsesProperty<BorderBrush> for Switch {}
impl UsesProperty<BorderBrush> for TextInput {}
impl UsesProperty<BorderBrush> for ZStack {}
impl UsesProperty<BorderBrush> for Image {}

// Generic widgets — type parameter is covered by the widget struct, so orphan
// rules are satisfied.
impl<W: Widget> UsesProperty<BorderBrush> for Portal<W> {}
impl UsesProperty<BorderBrush> for Badged {}
impl UsesProperty<BorderBrush> for Flex {}
impl<A: Widget, B: Widget> UsesProperty<BorderBrush> for Split<A, B> {}
impl<T: crate::widgets::step_input::Steppable> UsesProperty<BorderBrush> for StepInput<T> {}
impl UsesProperty<BorderBrush> for TextArea<true> {}
impl UsesProperty<BorderBrush> for TextArea<false> {}
impl UsesProperty<BorderBrush> for VirtualScroll {}

// Copyright 2018 the Xilem Authors and the Druid Authors
// SPDX-License-Identifier: Apache-2.0

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

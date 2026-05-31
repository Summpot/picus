//! Picus compatibility exports for code that previously imported `xilem`.
//!
//! Picus does not depend on the upstream `xilem` application crate; this module
//! exposes the subset needed by examples and public helpers.

pub use masonry_core::{dpi, kurbo, palette, peniko};
pub use masonry_core::{
    parley::Alignment as TextAlign,
    parley::style::FontWeight,
    peniko::{Blob, Color, ImageBrush, ImageFormat},
};
pub use masonry_winit::{app, winit};
pub use picus_view::{AnyWidgetView, Pod, ViewCtx, WidgetView, WidgetViewSequence};
pub use picus_view::{style, view};
pub use xilem_core as core;

pub use picus_view::picus_widget::widgets::InsertNewline;

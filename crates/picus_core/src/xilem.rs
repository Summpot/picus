//! Picus compatibility exports for code that previously imported `xilem`.
//!
//! Surfaces come from the `xilem` facade and `picus_widget::masonry_core`.

pub use crate::masonry_core::{dpi, kurbo, palette, peniko};
pub use crate::masonry_core::{
    parley::Alignment as TextAlign,
    parley::style::FontWeight,
    peniko::{Blob, Color, ImageBrush, ImageFormat},
};
pub use xilem::winit;
/// Subset of `masonry_winit::app` re-exported by the `xilem` facade.
pub mod app {
    pub use xilem::{EventLoop, EventLoopBuilder, WindowId};
}
pub use picus_view::{AnyWidgetView, Pod, ViewCtx, WidgetView, WidgetViewSequence};
pub use picus_view::{style, view};
pub use xilem::core as core;

pub use picus_view::picus_widget::widgets::InsertNewline;

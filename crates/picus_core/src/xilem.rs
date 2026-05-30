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
pub use xilem_core as core;
pub use xilem_masonry::{AnyWidgetView, Pod, ViewCtx, WidgetView, WidgetViewSequence};
pub use xilem_masonry::{style, view};

pub use xilem_masonry::masonry::widgets::InsertNewline;

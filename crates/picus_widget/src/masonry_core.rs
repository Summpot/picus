//! Masonry Core surface for Picus, taken from the `xilem` / `masonry` facade.
//!
//! Upstream apps use `masonry`, which re-exports `masonry_core` modules. This
//! module preserves a stable `masonry_core` path for Picus code while sourcing
//! APIs from `xilem::masonry`, and only exposes the **core** property subset
//! (widget-level properties live in [`crate::properties`]).

#![allow(missing_docs, reason = "Thin re-export layer; docs live upstream.")]

pub use anymore;
pub use xilem::masonry::doc;
pub use xilem::masonry::{
    accesskit, app, core, dpi, imaging, kurbo, layout, palette, parley, peniko, ui_events, util,
};

/// Core property types only (not the full `masonry::properties` widget set).
pub mod properties {
    pub use xilem::masonry::properties::{
        Background, BorderColor, BorderWidth, BoxShadow, CornerRadius, Dimensions, Padding,
    };

    /// Core value types used in properties (not widget-local alignment enums).
    pub mod types {
        pub use xilem::masonry::properties::types::{
            Gradient, GradientShape, RadialGradientExtent, RadialGradientShape,
        };
    }
}

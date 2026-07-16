// Copyright 2026 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! Imaging helpers for Picus (vendored from Linebender `masonry_imaging`).
//!
//! `picus_imaging` bridges Masonry paint output to concrete imaging backends:
//!
//! - flattened Masonry frames (base content + overlays)
//! - backend modules for `imaging_vello`, `imaging_vello_hybrid`,
//!   `imaging_vello_cpu`, and `imaging_skia`
//! - host-neutral texture rendering into caller-provided WGPU targets
//!
//! Desktop-only: wasm is not supported.
//!
//! # Feature flags
//!
//! - `default`: Enables the `vello` module.
//! - `imaging_vello`: Enables the `vello` module and texture rendering support.
//! - `imaging_vello_hybrid`: Enables the `vello_hybrid` module and texture rendering.
//! - `imaging_vello_cpu`: Enables the `vello_cpu` module for headless image rendering.
//! - `imaging_skia`: Enables the `skia` module and texture rendering.

// LINEBENDER LINT SET - lib.rs - v3
// See https://linebender.org/wiki/canonical-lints/
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![warn(clippy::print_stdout, clippy::print_stderr)]
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]

use imaging::record::{Glyph, Scene, ValidateError, replay, replay_transformed};
use imaging::render::RenderSource;
use imaging::{
    BlurredRoundedRect, ClipRef, ContextRef, FillRef, GlyphRunRef, GroupRef, PaintSink, Painter,
    StrokeRef,
};
use kurbo::{Affine, Rect};
use peniko::Color;

#[cfg(any(feature = "imaging_vello", feature = "imaging_vello_hybrid"))]
mod headless_wgpu;

/// Masonry helpers for rendering retained scenes with `imaging_skia`.
#[cfg(feature = "imaging_skia")]
pub mod skia;
/// Host-neutral texture rendering helpers for texture-capable backends.
pub mod texture_render;
/// Masonry helpers for rendering retained scenes with `imaging_vello`.
#[cfg(feature = "imaging_vello")]
pub mod vello;
/// Masonry helpers for rendering retained scenes with `imaging_vello_cpu`.
#[cfg(feature = "imaging_vello_cpu")]
pub mod vello_cpu;
/// Masonry helpers for rendering retained scenes with `imaging_vello_hybrid`.
#[cfg(feature = "imaging_vello_hybrid")]
pub mod vello_hybrid;

pub use imaging::render::ImageRenderer;
pub use imaging_wgpu::TextureRenderer;

/// Backend-selected helpers for headless image rendering.
pub mod image_render {
    #[cfg(all(not(feature = "imaging_vello"), feature = "imaging_skia"))]
    pub use crate::skia::{BACKEND_NAME, Renderer, new_headless_renderer};
    #[cfg(feature = "imaging_vello")]
    pub use crate::vello::{BACKEND_NAME, Renderer, new_headless_renderer};
    #[cfg(all(
        not(feature = "imaging_vello"),
        not(feature = "imaging_skia"),
        not(feature = "imaging_vello_hybrid"),
        feature = "imaging_vello_cpu"
    ))]
    pub use crate::vello_cpu::{BACKEND_NAME, Renderer, new_headless_renderer};
    #[cfg(all(
        not(feature = "imaging_vello"),
        not(feature = "imaging_skia"),
        feature = "imaging_vello_hybrid"
    ))]
    pub use crate::vello_hybrid::{BACKEND_NAME, Renderer, new_headless_renderer};

    #[cfg(not(any(
        feature = "imaging_vello",
        feature = "imaging_vello_hybrid",
        feature = "imaging_vello_cpu",
        feature = "imaging_skia"
    )))]
    pub use self::no_backend::{BACKEND_NAME, Error, Renderer, new_headless_renderer};

    #[cfg(not(any(
        feature = "imaging_vello",
        feature = "imaging_vello_hybrid",
        feature = "imaging_vello_cpu",
        feature = "imaging_skia"
    )))]
    mod no_backend {
        use imaging::render::{
            ImageBufferFormat, ImageBufferTarget, ImageRendererError, RenderSource,
        };

        /// Error returned when no image-render backend feature is enabled.
        #[derive(Debug)]
        pub struct Error;

        impl core::fmt::Display for Error {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("no imaging backend feature selected")
            }
        }

        impl std::error::Error for Error {}

        /// Placeholder renderer used when no image-render backend feature is enabled.
        #[derive(Debug)]
        pub struct Renderer;

        /// Stable diagnostics name for the backend-less stub renderer.
        pub const BACKEND_NAME: &str = "no_backend";

        /// Create the backend-less stub renderer.
        pub fn new_headless_renderer() -> Result<Renderer, Error> {
            Err(Error)
        }

        impl imaging::render::ImageRenderer for Renderer {
            fn supported_image_formats(&self) -> Vec<ImageBufferFormat> {
                Vec::new()
            }

            fn render_source_into(
                &mut self,
                _: &mut dyn RenderSource,
                _: ImageBufferTarget<'_>,
            ) -> Result<(), ImageRendererError> {
                Err(ImageRendererError::backend(Error))
            }
        }
    }
}

/// A Masonry overlay layer ready to be composited into window space.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    /// The retained scene for this layer in layer-local coordinates.
    pub scene: &'a Scene,
    /// Transform from layer-local coordinates into window coordinates.
    pub transform: Affine,
}

impl core::fmt::Debug for Layer<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Layer")
            .field("scene", &"(Scene)")
            .field("transform", &self.transform)
            .finish()
    }
}

/// A flattened Masonry frame ready to be adapted to a concrete render target.
///
/// This is intentionally a single-target convenience type for Masonry's current rendering paths.
/// Future compositor-oriented work is expected to preserve more layer structure above this level.
#[derive(Clone, Copy, Debug)]
pub struct PreparedFrame<'a> {
    /// Frame width in physical pixels.
    pub width: u32,
    /// Frame height in physical pixels.
    pub height: u32,
    /// Window scale factor.
    pub scale_factor: f64,
    /// Background color to paint before replaying scene content.
    pub background_color: Color,
    /// Base retained scene in root coordinates.
    pub base: &'a Scene,
    /// Overlay layers in painter order.
    pub overlays: &'a [Layer<'a>],
}

impl<'a> PreparedFrame<'a> {
    /// Create a flattened Masonry frame from base content plus overlays.
    pub fn new(
        width: u32,
        height: u32,
        scale_factor: f64,
        background_color: Color,
        base: &'a Scene,
        overlays: &'a [Layer<'a>],
    ) -> Self {
        Self {
            width,
            height,
            scale_factor,
            background_color,
            base,
            overlays,
        }
    }
}

impl RenderSource for PreparedFrame<'_> {
    fn validate(&self) -> Result<(), ValidateError> {
        validate_layers(self.base, self.overlays)
    }

    fn paint_into(&mut self, sink: &mut dyn PaintSink) {
        {
            let mut painter = Painter::new(sink);
            painter.fill_rect(
                Rect::new(0.0, 0.0, f64::from(self.width), f64::from(self.height)),
                self.background_color,
            );
        }

        let scale = Affine::scale(self.scale_factor);
        // Balance clip/group/context stacks before replay. Masonry can emit
        // UnclosedClips when External/IsolatedScene finishes a layer mid ancestor
        // clip (e.g. Spinner under Portal) — Vello encode rejects those scenes.
        let base = ensure_balanced_scene(self.base);
        replay_transformed(base.as_ref(), sink, scale);
        for layer in self.overlays {
            let scene = ensure_balanced_scene(layer.scene);
            replay_transformed(scene.as_ref(), sink, scale * layer.transform);
        }
    }
}

fn validate_layers(base: &Scene, overlays: &[Layer<'_>]) -> Result<(), ValidateError> {
    ensure_balanced_scene(base).validate()?;
    for layer in overlays {
        ensure_balanced_scene(layer.scene).validate()?;
    }
    Ok(())
}

/// Borrow `scene` when already well-nested; otherwise rebuild with balanced stacks.
///
/// Masonry `LayerCollector::finish_current_layer` severs the current scene when a
/// widget requests [`PaintLayerMode::External`] / `IsolatedScene`. If an ancestor
/// had pushed a clip (Portal, Label clip, etc.) the finished scene still has open
/// `PushClip` commands (`ValidateError::UnclosedClips`). Sibling content after the
/// split may get a matching `PopClip` without a push (`UnbalancedPopClip`).
///
/// Picus cannot fix Masonry's collector without a dependency patch; balancing here
/// keeps encode valid. Ancestor clipping across External slots remains a known
/// host isolation limitation (see `AncestorClip` / paint-isolation docs).
fn ensure_balanced_scene(scene: &Scene) -> std::borrow::Cow<'_, Scene> {
    if scene.validate().is_ok() {
        std::borrow::Cow::Borrowed(scene)
    } else {
        std::borrow::Cow::Owned(balance_scene_stacks(scene))
    }
}

/// Rebuild `scene` so clip / group / context stacks are well-nested.
///
/// - Excess `Pop*` commands (no matching push) are dropped.
/// - Unclosed stacks are closed at the end of the stream.
#[must_use]
pub fn balance_scene_stacks(scene: &Scene) -> Scene {
    let mut sink = StackBalancingSink::new();
    replay(scene, &mut sink);
    sink.finish()
}

/// [`PaintSink`] that records into a [`Scene`] while enforcing stack nesting.
struct StackBalancingSink {
    scene: Scene,
    clip_depth: u32,
    group_depth: u32,
    context_depth: u32,
}

impl StackBalancingSink {
    fn new() -> Self {
        Self {
            scene: Scene::new(),
            clip_depth: 0,
            group_depth: 0,
            context_depth: 0,
        }
    }

    fn finish(mut self) -> Scene {
        while self.context_depth > 0 {
            self.scene.pop_context();
            self.context_depth -= 1;
        }
        while self.group_depth > 0 {
            self.scene.pop_group();
            self.group_depth -= 1;
        }
        while self.clip_depth > 0 {
            self.scene.pop_clip();
            self.clip_depth -= 1;
        }
        self.scene
    }
}

impl PaintSink for StackBalancingSink {
    fn push_context(&mut self, context: ContextRef<'_>) {
        PaintSink::push_context(&mut self.scene, context);
        self.context_depth = self.context_depth.saturating_add(1);
    }

    fn pop_context(&mut self) {
        if self.context_depth == 0 {
            return;
        }
        PaintSink::pop_context(&mut self.scene);
        self.context_depth -= 1;
    }

    fn push_clip(&mut self, clip: ClipRef<'_>) {
        PaintSink::push_clip(&mut self.scene, clip);
        self.clip_depth = self.clip_depth.saturating_add(1);
    }

    fn pop_clip(&mut self) {
        if self.clip_depth == 0 {
            return;
        }
        PaintSink::pop_clip(&mut self.scene);
        self.clip_depth -= 1;
    }

    fn push_group(&mut self, group: GroupRef<'_>) {
        PaintSink::push_group(&mut self.scene, group);
        self.group_depth = self.group_depth.saturating_add(1);
    }

    fn pop_group(&mut self) {
        if self.group_depth == 0 {
            return;
        }
        PaintSink::pop_group(&mut self.scene);
        self.group_depth -= 1;
    }

    fn fill(&mut self, draw: FillRef<'_>) {
        PaintSink::fill(&mut self.scene, draw);
    }

    fn stroke(&mut self, draw: StrokeRef<'_>) {
        PaintSink::stroke(&mut self.scene, draw);
    }

    fn glyph_run(&mut self, draw: GlyphRunRef<'_>, glyphs: &mut dyn Iterator<Item = Glyph>) {
        PaintSink::glyph_run(&mut self.scene, draw, glyphs);
    }

    fn blurred_rounded_rect(&mut self, draw: BlurredRoundedRect) {
        PaintSink::blurred_rounded_rect(&mut self.scene, draw);
    }
}

#[cfg(test)]
mod balance_tests {
    use imaging::record::{Clip, Geometry};
    use peniko::Fill;

    use super::*;

    fn fill_clip(rect: Rect) -> Clip {
        Clip::Fill {
            transform: Affine::IDENTITY,
            shape: Geometry::Rect(rect),
            fill_rule: Fill::NonZero,
        }
    }

    #[test]
    fn balance_closes_unclosed_clips() {
        let mut scene = Scene::new();
        scene.push_clip(fill_clip(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(matches!(
            scene.validate(),
            Err(ValidateError::UnclosedClips { .. })
        ));

        let balanced = balance_scene_stacks(&scene);
        assert_eq!(balanced.validate(), Ok(()));
    }

    #[test]
    fn balance_drops_excess_pop_clips() {
        let mut scene = Scene::new();
        scene.pop_clip();
        assert!(matches!(
            scene.validate(),
            Err(ValidateError::UnbalancedPopClip { .. })
        ));

        let balanced = balance_scene_stacks(&scene);
        assert_eq!(balanced.validate(), Ok(()));
        assert!(balanced.is_empty());
    }

    #[test]
    fn balance_preserves_nested_clips() {
        let mut scene = Scene::new();
        scene.push_clip(fill_clip(Rect::new(0.0, 0.0, 20.0, 20.0)));
        scene.push_clip(fill_clip(Rect::new(1.0, 1.0, 5.0, 5.0)));
        scene.pop_clip();
        scene.pop_clip();
        assert_eq!(scene.validate(), Ok(()));

        let balanced = balance_scene_stacks(&scene);
        assert_eq!(balanced.validate(), Ok(()));
        assert_eq!(balanced.commands().len(), scene.commands().len());
    }

    #[test]
    fn ensure_balanced_borrows_valid_scene() {
        let scene = Scene::new();
        match ensure_balanced_scene(&scene) {
            std::borrow::Cow::Borrowed(_) => {}
            std::borrow::Cow::Owned(_) => panic!("valid empty scene should be borrowed"),
        }
    }
}

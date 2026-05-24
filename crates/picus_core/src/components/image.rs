use bevy_ecs::prelude::*;
use masonry::peniko::{Blob, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality};
use std::{path::Path, sync::Arc};
use xilem::Color;
use xilem_masonry::view::ObjectFit;

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Units used by [`UiImageViewBox`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiImageViewBoxUnits {
    #[default]
    Pixels,
    Fraction,
}

/// Source rectangle intent for an image.
///
/// The current Masonry image widget does not expose source-rectangle rendering,
/// so this is stored for data/model parity and future custom image projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiImageViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub units: UiImageViewBoxUnits,
}

impl UiImageViewBox {
    #[must_use]
    pub const fn pixels(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            units: UiImageViewBoxUnits::Pixels,
        }
    }

    #[must_use]
    pub const fn fraction(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            units: UiImageViewBoxUnits::Fraction,
        }
    }
}

/// Horizontal alignment intent for image content inside its box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiImageAlignmentX {
    Left,
    #[default]
    Center,
    Right,
}

/// Vertical alignment intent for image content inside its box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiImageAlignmentY {
    Top,
    #[default]
    Center,
    Bottom,
}

/// Bitmap image rendered through Masonry's native image widget.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiImage {
    pub image: Option<ImageData>,
    pub alt_text: Option<String>,
    pub decorative: bool,
    pub fit: ObjectFit,
    pub quality: ImageQuality,
    pub alpha: f32,
    pub view_box: Option<UiImageViewBox>,
    pub alignment_x: UiImageAlignmentX,
    pub alignment_y: UiImageAlignmentY,
}

impl UiImage {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            image: None,
            alt_text: None,
            decorative: false,
            fit: ObjectFit::Contain,
            quality: ImageQuality::Medium,
            alpha: 1.0,
            view_box: None,
            alignment_x: UiImageAlignmentX::Center,
            alignment_y: UiImageAlignmentY::Center,
        }
    }

    #[must_use]
    pub fn new(image: ImageData) -> Self {
        Self {
            image: Some(image),
            ..Self::empty()
        }
    }

    #[must_use]
    pub fn from_rgba8(width: u32, height: u32, rgba8: Vec<u8>) -> Self {
        Self::from_raw(
            width,
            height,
            rgba8,
            ImageFormat::Rgba8,
            ImageAlphaType::Alpha,
        )
    }

    #[must_use]
    pub fn from_bgra8(width: u32, height: u32, bgra8: Vec<u8>) -> Self {
        Self::from_raw(
            width,
            height,
            bgra8,
            ImageFormat::Bgra8,
            ImageAlphaType::Alpha,
        )
    }

    #[must_use]
    pub fn from_rgb8(width: u32, height: u32, rgb8: impl AsRef<[u8]>) -> Self {
        let mut rgba8 = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in rgb8.as_ref().chunks_exact(3) {
            rgba8.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
        Self::from_rgba8(width, height, rgba8)
    }

    #[must_use]
    pub fn from_raw(
        width: u32,
        height: u32,
        bytes: Vec<u8>,
        format: ImageFormat,
        alpha_type: ImageAlphaType,
    ) -> Self {
        Self::new(ImageData {
            data: Blob::new(Arc::new(bytes)),
            format,
            alpha_type,
            width,
            height,
        })
    }

    pub fn from_encoded_bytes(bytes: &[u8]) -> Result<Self, image::ImageError> {
        let decoded = image::load_from_memory(bytes)?.into_rgba8();
        let (width, height) = decoded.dimensions();
        Ok(Self::from_rgba8(width, height, decoded.into_raw()))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, image::ImageError> {
        let decoded = image::open(path)?.into_rgba8();
        let (width, height) = decoded.dimensions();
        Ok(Self::from_rgba8(width, height, decoded.into_raw()))
    }

    #[must_use]
    pub fn with_alt_text(mut self, alt_text: impl Into<String>) -> Self {
        self.alt_text = Some(alt_text.into());
        self
    }

    #[must_use]
    pub fn decorative(mut self, decorative: bool) -> Self {
        self.decorative = decorative;
        self
    }

    #[must_use]
    pub fn fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub fn quality(mut self, quality: ImageQuality) -> Self {
        self.quality = quality;
        self
    }

    #[must_use]
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn view_box(mut self, view_box: UiImageViewBox) -> Self {
        self.view_box = Some(view_box);
        self
    }

    #[must_use]
    pub fn alignment(mut self, x: UiImageAlignmentX, y: UiImageAlignmentY) -> Self {
        self.alignment_x = x;
        self.alignment_y = y;
        self
    }

    #[must_use]
    pub fn source_size(&self) -> Option<(u32, u32)> {
        self.image.as_ref().map(|image| (image.width, image.height))
    }

    #[must_use]
    pub fn peek_rgba8(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let image = self.image.as_ref()?;
        if x >= image.width || y >= image.height {
            return None;
        }
        let index = ((y as usize * image.width as usize) + x as usize) * 4;
        let data = image.data.data();
        let pixel = data.get(index..index + 4)?;
        match image.format {
            ImageFormat::Rgba8 => Some([pixel[0], pixel[1], pixel[2], pixel[3]]),
            ImageFormat::Bgra8 => Some([pixel[2], pixel[1], pixel[0], pixel[3]]),
            _ => None,
        }
    }

    #[must_use]
    pub fn peek_color(&self, x: u32, y: u32) -> Option<Color> {
        let [r, g, b, a] = self.peek_rgba8(x, y)?;
        Some(Color::from_rgba8(r, g, b, a))
    }

    #[must_use]
    pub fn image_brush(&self) -> Option<ImageBrush> {
        Some(
            ImageBrush::new(self.image.clone()?)
                .with_quality(self.quality)
                .with_alpha(self.alpha),
        )
    }
}

impl Default for UiImage {
    fn default() -> Self {
        Self::empty()
    }
}

impl UiComponentTemplate for UiImage {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::elements::project_image(component, ctx)
    }
}

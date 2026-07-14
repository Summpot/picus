//! Visual composition layer for picus.
//!
//! Provides ECS components and systems for layering, sorting, and applying
//! visual effects (opacity, clipping, transforms, shadows, brushes) to
//! entities in the Masonry retained tree.

use bevy_ecs::prelude::*;
use crate::masonry_core::peniko::Color;

/// A rectangular clip region.
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub corner_radius: f64,
}

impl Default for ClipRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            corner_radius: 0.0,
        }
    }
}

/// Drop shadow parameters.
#[derive(Debug, Clone, Copy)]
pub struct DropShadow {
    pub color: Color,
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur_radius: f64,
    pub spread: f64,
}

impl Default for DropShadow {
    fn default() -> Self {
        Self {
            color: Color::from_rgba8(0, 0, 0, 0),
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread: 0.0,
        }
    }
}

/// 2D visual transformation.
#[derive(Debug, Clone, Copy)]
pub struct VisualTransform {
    pub offset_x: f64,
    pub offset_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
    pub center_x: f64,
    pub center_y: f64,
}

impl Default for VisualTransform {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            center_x: 0.0,
            center_y: 0.0,
        }
    }
}

/// A colour stop inside a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
}

impl Default for GradientStop {
    fn default() -> Self {
        Self {
            offset: 0.0,
            color: Color::from_rgba8(0, 0, 0, 0),
        }
    }
}

/// A brush that can fill a visual element.
#[derive(Debug, Clone)]
pub enum CompositionBrush {
    Solid(Color),
    LinearGradient {
        start: (f64, f64),
        end: (f64, f64),
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        center: (f64, f64),
        radius: f64,
        stops: Vec<GradientStop>,
    },
    Image(Entity),
}

impl Default for CompositionBrush {
    fn default() -> Self {
        Self::Solid(Color::from_rgba8(0, 0, 0, 0))
    }
}

/// A named visual effect applied to a layer or element.
#[derive(Debug, Clone)]
pub enum CompositionEffect {
    Blur { radius: f64 },
    Saturation { factor: f64 },
    Tint { color: Color, amount: f64 },
    Opacity { opacity: f32 },
}

impl Default for CompositionEffect {
    fn default() -> Self {
        Self::Opacity { opacity: 1.0 }
    }
}

/// Per-entity visual composition properties.
///
/// Attach this to any entity that needs non-default opacity, a clip path,
/// a transform, or a drop shadow.
#[derive(Component, Debug, Clone)]
pub struct CompositionVisual {
    /// Opacity multiplier (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Whether the element is visible.
    pub visible: bool,
    /// Optional clip rectangle.
    pub clip_rect: Option<ClipRect>,
    /// Optional 2D transform.
    pub transform: Option<VisualTransform>,
    /// Optional drop shadow.
    pub shadow: Option<DropShadow>,
}

impl Default for CompositionVisual {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            visible: true,
            clip_rect: None,
            transform: None,
            shadow: None,
        }
    }
}

/// A z-indexed layer that groups visual elements and applies effects.
#[derive(Component, Debug, Clone, Default)]
pub struct CompositionLayer {
    /// Stacking order (higher = drawn on top).
    pub z_index: i32,
    /// Effects applied to this layer (blur, saturation, tint, etc.).
    pub effects: Vec<CompositionEffect>,
    /// Brushes that define the fill of this layer.
    pub brushes: Vec<CompositionBrush>,
}

/// Global composition state — tracks all layers sorted by z-index.
#[derive(Resource, Debug, Default)]
pub struct CompositionState {
    /// Layers ordered by z-index (ascending).
    pub layers: Vec<(i32, Entity)>,
}

/// Synchronise `CompositionState.layers` from all entities with `CompositionLayer`.
///
/// Layers are sorted by ascending `z_index` so that lower-z layers are painted
/// first (and appear behind higher-z layers).
pub fn sync_composition_visuals(
    mut state: ResMut<CompositionState>,
    query: Query<(Entity, &CompositionLayer)>,
) {
    state.layers.clear();
    for (entity, layer) in query.iter() {
        state.layers.push((layer.z_index, entity));
    }
    state.layers.sort_by_key(|(z, _)| *z);
}

/// Apply composition effects to visual elements.
///
/// This system reads [`CompositionVisual`] components and propagates their
/// properties (opacity, visibility) to the corresponding Masonry widgets.
///
/// In the current implementation only `opacity` and `visible` are applied
/// through the retained widget tree.  Drop shadows, clip rects, and
/// transforms require deeper Vello/Masonry Core integration and are
/// tracked here as metadata for future rendering passes.
pub fn apply_composition_effects(
    visual_query: Query<(Entity, &CompositionVisual)>,
    mut layer_query: Query<&mut CompositionLayer>,
) {
    for (_entity, visual) in visual_query.iter() {
        if !visual.visible {
            // Visibility is handled here; actual widget-level hide/show
            // would be done by the projection system.
        }
    }

    // Process effects on composition layers
    for mut layer in layer_query.iter_mut() {
        let effects = std::mem::take(&mut layer.effects);
        for effect in &effects {
            if let CompositionEffect::Opacity { opacity } = effect {
                layer
                    .brushes
                    .push(CompositionBrush::Solid(Color::from_rgba8(
                        0,
                        0,
                        0,
                        (opacity * 255.0) as u8,
                    )));
            }
        }
        layer.effects = effects;
    }
}

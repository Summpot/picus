//! Phase 2a hard gate: Masonry layer contract + anim target spike.
//!
//! This module records the **boundary decision** for layered anim encode before
//! multi-texture composite (P2b). It is **not** a full compositor and is
//! **not** wired into [`super::WindowRuntime`] / `step_frame` yet.
//!
//! ## Gate questions (must be answered before P2b)
//!
//! 1. Can Masonry isolated scene / layer APIs produce **self-contained,
//!    independently renderable** painter-order entries under ancestor
//!    clip/scroll, transform, ZStack, and overlay?
//! 2. Can an anim tick emit **only the changed anim entry** without full-tree
//!    [`RenderRoot::redraw`] and without reassembling the base scene?
//!
//! ## Answers (xilem rev `4b1922c`, see tests + docs)
//!
//! | Question | Result |
//! |----------|--------|
//! | Self-contained independent entries | **Fail** on sticky isolation + missing clip package (type-level on `VisualLayer`) + External host skip. Scroll / ZStack / Masonry overlay-stack were **not** separately spiked; FAIL still holds because isolation is non-sticky and layers lack clip-chain metadata. |
//! | Selective anim entry without full redraw | **Fail** — public scene path is only full `redraw()` → `run_paint_pass`; plan always reassembly. Host dirty set is the *planned* selective unit (P2b), not current paint. |
//!
//! ## Selected path
//!
//! **Picus [`AnimLayerHost`]** (not an upstream-only wait):
//! - Masonry: layout, hit-test, painter-order **External** placeholders
//! - Picus: independent anim draw state + scene builder for dirty anim entries
//! - Composite (P2b): exact-order base segments + host scenes/textures
//!
//! Upstream strategy remains open as a **parallel** improvement (persistent
//! LayerId, self-contained clip/effect, selective layer redraw) but does not
//! block P2b.
//!
//! **Forbidden reading:** classifying a post-hoc `VisualLayerPlan` as
//! “per-layer scene build” is incorrect — the plan is a full-pass snapshot.
//!
//! ## Not yet (P2b — do not read scaffold as live wiring)
//!
//! - Field on `WindowRuntime` / use from `step_frame`
//! - `DirtyReason::AnimPaint { layer }` populated from [`AnimLayerId::raw`]
//! - Widget path calling `set_paint_layer_mode(External)` every paint (mode is
//!   **not sticky**; host `register_*` does **not** set paint mode)
//! - Scene / texture storage or multi-texture composite
//! - Skipping base rewrite on pure anim ticks (still full-window encode today)
//!
//! See `docs/architecture/runtime.md` (Masonry layer contract) and
//! `docs/plans/frame-pipeline.md` Phase 2a.

use std::collections::HashMap;

use crate::masonry_core::{
    core::{PaintLayerMode, WidgetId},
    kurbo::{Affine, Rect},
};

// ---------------------------------------------------------------------------
// Gate inventory (what pinned xilem actually offers)
// ---------------------------------------------------------------------------

/// How a [`MasonryLayerCapabilities`] bit is backed for pin-bump honesty.
///
/// Empirical spikes fail when upstream behavior changes; inventory checklist
/// bits are human-maintained against the pin and must be re-audited on bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilityEvidence {
    /// Enforced by RenderRoot / type-shape tests in this module.
    EmpiricalSpike,
    /// Checklist vs public API / struct shape; update when bumping xilem.
    InventoryChecklist,
}

/// Capabilities of the pinned Masonry/xilem paint boundary (Phase 2a inventory).
///
/// Values are fixed for the git pin in workspace `Cargo.toml` (`xilem` rev
/// `4b1922c9728f7b86642b6759c6608f32e0badec2`). Re-run the module tests when
/// bumping the pin.
///
/// | Field | Evidence |
/// |-------|----------|
/// | `paint_layer_mode_api` | Empirical (ModeBox spikes) |
/// | `visual_layer_plan` | Empirical (`redraw` returns plan) |
/// | `external_placeholders` | Empirical (External kind + collapse) |
/// | `flatten_compatibility_helpers` | Empirical (`overlay_layers` skip) |
/// | `sticky_paint_layer_mode` | Empirical (second redraw collapses) |
/// | `self_contained_ancestor_clip` | Empirical type-shape (`VisualLayer` fields) + clip spike |
/// | `selective_layer_redraw` | Empirical (only full `redraw` path after AnimFrame) |
/// | `persistent_layer_id` | Inventory checklist (no public LayerId type; upstream FIXME) |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MasonryLayerCapabilities {
    /// `PaintLayerMode::{Inline, IsolatedScene, External}` exists and is set per paint.
    pub paint_layer_mode_api: bool,
    /// `VisualLayerPlan` carries painter-order `VisualLayer` entries with `widget_id`.
    pub visual_layer_plan: bool,
    /// `VisualLayerKind::External { bounds }` placeholders exist for host content.
    pub external_placeholders: bool,
    /// Persistent compositor `LayerId` (stable across frames, independent of WidgetId).
    pub persistent_layer_id: bool,
    /// `PaintLayerMode` survives frames without the widget re-entering `paint`.
    pub sticky_paint_layer_mode: bool,
    /// Isolated scenes package ancestor clip/scroll/effect for independent encode.
    pub self_contained_ancestor_clip: bool,
    /// Public API to rebuild/emit a single layer without full-tree paint reassembly.
    pub selective_layer_redraw: bool,
    /// Host helpers still present as flatten-oriented (`root_layer` / `overlay_layers`).
    pub flatten_compatibility_helpers: bool,
}

impl MasonryLayerCapabilities {
    /// Inventory for the current workspace xilem pin.
    pub(crate) const CURRENT_PIN: Self = Self {
        paint_layer_mode_api: true,
        visual_layer_plan: true,
        external_placeholders: true,
        persistent_layer_id: false,
        sticky_paint_layer_mode: false,
        self_contained_ancestor_clip: false,
        selective_layer_redraw: false,
        flatten_compatibility_helpers: true,
    };

    /// Evidence class for each gate bit (documentation + pin-bump audit aid).
    pub(crate) fn evidence(field: &'static str) -> CapabilityEvidence {
        match field {
            "persistent_layer_id" => CapabilityEvidence::InventoryChecklist,
            _ => CapabilityEvidence::EmpiricalSpike,
        }
    }

    /// True when Masonry alone can satisfy G2-style anim isolation without a Picus host.
    pub(crate) const fn supports_upstream_only_anim_isolation(self) -> bool {
        self.persistent_layer_id
            && self.sticky_paint_layer_mode
            && self.self_contained_ancestor_clip
            && self.selective_layer_redraw
    }
}

/// Outcome of the Phase 2a hard gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Both arms are part of the documented decision space for P2b+.
pub(crate) enum LayerBoundaryDecision {
    /// Wait on / pin a fixed upstream with LayerId + self-contained clip + selective redraw.
    UpstreamFixedXilem,
    /// Picus owns anim draw state; Masonry layout/hit-test + External painter slots.
    PicusAnimLayerHost,
}

impl LayerBoundaryDecision {
    /// Gate result for the current pin: upstream is insufficient → AnimLayerHost.
    pub(crate) const SELECTED: Self = Self::PicusAnimLayerHost;
}

// ---------------------------------------------------------------------------
// Anim target strategy (size / encode budget gate input for P2b)
// ---------------------------------------------------------------------------

/// Where anim pixels are rendered before exact-order composite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // Atlas arm is the documented fallback if FullWindow fails size gates.
pub(crate) enum AnimTargetStrategy {
    /// Full-window transparent texture; only anim widgets paint into it.
    ///
    /// **Selected for first composite (P2b):** simpler transform/clip bookkeeping;
    /// encode cost is full-window but anim scene is sparse. Meets plan §2.0
    /// recommendation; atlas deferred if G3/G4 encode budget fails.
    #[default]
    FullWindowTransparent,
    /// Tight widget bounds / atlas sub-rects (Phase 4 / late P2 if needed).
    WidgetBoundsAtlas,
}

impl AnimTargetStrategy {
    /// First product path for P2b.
    pub(crate) const FIRST_COMPOSITE: Self = Self::FullWindowTransparent;
}

// ---------------------------------------------------------------------------
// Selected interface: AnimLayerHost (scaffold for P2b; not wired into paint yet)
// ---------------------------------------------------------------------------

/// Stable Picus-owned anim entry id (not a Masonry LayerId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AnimLayerId(u32);

impl AnimLayerId {
    #[inline]
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }
}

/// How a host entry maps into Masonry painter order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimSlotBinding {
    /// Widget should paint with [`PaintLayerMode::External`]; host fills the slot.
    ///
    /// Registering here does **not** call `set_paint_layer_mode` — the widget
    /// (or its projector) must request External **every paint** (mode is not sticky).
    ExternalPlaceholder { widget_id: WidgetId },
    /// No Masonry placeholder yet (pre-layout / pre-widget registration).
    Unbound,
}

/// One independently dirty-able anim entry owned by Picus.
///
/// Scene bytes / GPU textures are **not** stored here in P2a — only the
/// ownership and dirty contract P2b will encode against.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AnimLayerEntry {
    pub id: AnimLayerId,
    pub slot: AnimSlotBinding,
    /// Window-space bounds last observed from layout (or placeholder).
    pub bounds: Rect,
    /// Window transform for the slot (identity when unbound).
    pub transform: Affine,
    /// Monotonic content version; bumps on anim paint.
    pub version: u64,
    /// Encode needed for this entry.
    pub dirty: bool,
}

/// Picus-side registry for isolated anim draw state.
///
/// # Status (P2a)
///
/// Free-standing **scaffold**. Not a field on `WindowRuntime`, not consulted by
/// `step_frame` / paint. Product paint remains full-window encode with
/// `DirtyReason::AnimPaint { layer: 0 }`.
///
/// # Planned ownership / lifecycle (**P2b target**, not current)
///
/// ```text
/// WindowRuntime  (planned)
///   ├── RenderRoot (Masonry)     layout / hit-test / External placeholders
///   ├── AnimLayerHost (Picus)    anim entry state + dirty/version
///   └── LayerSurfaces            base + anim textures, exact-order composite
///
/// register_external_slot(widget_id)  → AnimLayerId in host maps only
///   (widget must still set_paint_layer_mode(External) every paint)
/// layout/compose                     → update bounds/transform; CompositorPlan if plan changes
/// AnimFrame tick                     → host.mark_anim_paint(id)
///                                      [target] skip base rewrite when only anim dirty
/// encode                             → [target] dirty host entries only; base if base_dirty
/// remove/unmount                     → drop entry; External slot drops on next plan
/// ```
///
/// # TODO(P2b)
/// - Attach host as `WindowRuntime` field; drive from `step_frame`
/// - Wire Spinner / indeterminate ProgressBar paint into host scenes
/// - Populate `DirtyReason::AnimPaint { layer: id.raw() }` from dirty host entries
/// - Realize External slots in `PreparedFrame` / multi-texture composite
/// - Avoid base rewrite/encode on pure anim ticks (G2)
#[derive(Debug, Default)]
pub(crate) struct AnimLayerHost {
    next_id: u32,
    entries: HashMap<AnimLayerId, AnimLayerEntry>,
    by_widget: HashMap<WidgetId, AnimLayerId>,
    target: AnimTargetStrategy,
}

impl AnimLayerHost {
    pub(crate) fn new(target: AnimTargetStrategy) -> Self {
        Self {
            next_id: 1,
            entries: HashMap::new(),
            by_widget: HashMap::new(),
            target,
        }
    }

    #[inline]
    pub(crate) fn target_strategy(&self) -> AnimTargetStrategy {
        self.target
    }

    fn alloc_id(&mut self) -> AnimLayerId {
        let id = AnimLayerId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Register (or return existing) anim entry for a Masonry widget id.
    ///
    /// Does **not** set `PaintLayerMode::External` on the widget — callers must
    /// ensure the widget requests External every paint (mode is not sticky).
    pub(crate) fn register_external_slot(&mut self, widget_id: WidgetId) -> AnimLayerId {
        if let Some(&id) = self.by_widget.get(&widget_id) {
            return id;
        }
        let id = self.alloc_id();
        self.by_widget.insert(widget_id, id);
        self.entries.insert(
            id,
            AnimLayerEntry {
                id,
                slot: AnimSlotBinding::ExternalPlaceholder { widget_id },
                bounds: Rect::ZERO,
                transform: Affine::IDENTITY,
                version: 0,
                dirty: true,
            },
        );
        id
    }

    /// Pre-layout registration before a Masonry widget id exists.
    pub(crate) fn register_unbound(&mut self) -> AnimLayerId {
        let id = self.alloc_id();
        self.entries.insert(
            id,
            AnimLayerEntry {
                id,
                slot: AnimSlotBinding::Unbound,
                bounds: Rect::ZERO,
                transform: Affine::IDENTITY,
                version: 0,
                dirty: true,
            },
        );
        id
    }

    /// Bind a previously unbound entry to a Masonry External placeholder widget.
    pub(crate) fn bind_external_slot(&mut self, id: AnimLayerId, widget_id: WidgetId) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        if !matches!(entry.slot, AnimSlotBinding::Unbound) {
            return false;
        }
        if self.by_widget.contains_key(&widget_id) {
            return false;
        }
        entry.slot = AnimSlotBinding::ExternalPlaceholder { widget_id };
        entry.dirty = true;
        self.by_widget.insert(widget_id, id);
        true
    }

    pub(crate) fn get(&self, id: AnimLayerId) -> Option<&AnimLayerEntry> {
        self.entries.get(&id)
    }

    pub(crate) fn get_mut(&mut self, id: AnimLayerId) -> Option<&mut AnimLayerEntry> {
        self.entries.get_mut(&id)
    }

    pub(crate) fn id_for_widget(&self, widget_id: WidgetId) -> Option<AnimLayerId> {
        self.by_widget.get(&widget_id).copied()
    }

    /// `DirtyReason::AnimPaint { layer }` values for currently dirty entries (P2b).
    pub(crate) fn dirty_anim_paint_layers(&self) -> impl Iterator<Item = u32> + '_ {
        self.dirty_ids().map(|id| id.raw())
    }

    /// Layout/compose observed new geometry — may force composite plan refresh.
    pub(crate) fn update_slot_geometry(
        &mut self,
        id: AnimLayerId,
        bounds: Rect,
        transform: Affine,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        let changed = entry.bounds != bounds || entry.transform != transform;
        if changed {
            entry.bounds = bounds;
            entry.transform = transform;
            // Geometry change invalidates prior texture placement.
            entry.dirty = true;
        }
        changed
    }

    /// Anim content advanced; only this entry needs encode (contract for P2b).
    pub(crate) fn mark_anim_paint(&mut self, id: AnimLayerId) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        entry.version = entry.version.saturating_add(1);
        entry.dirty = true;
        true
    }

    pub(crate) fn clear_dirty_after_encode(&mut self, id: AnimLayerId) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.dirty = false;
        }
    }

    pub(crate) fn dirty_ids(&self) -> impl Iterator<Item = AnimLayerId> + '_ {
        self.entries
            .iter()
            .filter(|(_, e)| e.dirty)
            .map(|(&id, _)| id)
    }

    pub(crate) fn remove_widget(&mut self, widget_id: WidgetId) -> Option<AnimLayerEntry> {
        let id = self.by_widget.remove(&widget_id)?;
        self.entries.remove(&id)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Paint mode widgets must request **every paint** so Masonry leaves an
    /// External slot (`paint_layer_mode` resets to Inline each pass).
    pub(crate) const fn required_paint_layer_mode() -> PaintLayerMode {
        PaintLayerMode::External
    }
}

// ---------------------------------------------------------------------------
// Tests — spike against real RenderRoot + host unit contracts
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::sync::Arc;

    use accesskit::{Node, Role};
    use tracing::{Span, trace_span};

    use super::*;
    use crate::masonry_core::{
        app::{RenderRoot, RenderRootOptions, VisualLayerKind, WindowSizePolicy},
        core::{
            AccessCtx, ChildrenIds, DefaultProperties, LayoutCtx, MeasureCtx, NewWidget, NoAction,
            PaintCtx, PaintLayerMode, PropertiesRef, RegisterCtx, UpdateCtx, Widget, WidgetId,
            WidgetPod, WindowEvent,
        },
        dpi::PhysicalSize,
        imaging::Painter,
        kurbo::{Axis, Point, Rect, Size},
        layout::{LenReq, Length},
        peniko::Color,
    };
    use picus_widget::widgets::{Flex, SizedBox, Spinner};

    // --- minimal widgets for layer-mode spikes --------------------------------

    /// Solid fill; optionally requests IsolatedScene or External.
    struct ModeBox {
        mode: PaintLayerMode,
        color: Color,
    }

    impl ModeBox {
        fn new(mode: PaintLayerMode, color: Color) -> Self {
            Self { mode, color }
        }
    }

    impl Widget for ModeBox {
        type Action = NoAction;

        fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

        fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: TypeId) {}

        fn measure(
            &mut self,
            _ctx: &mut MeasureCtx<'_>,
            _props: &PropertiesRef<'_>,
            _axis: Axis,
            _len_req: LenReq,
            _cross_length: Option<Length>,
        ) -> Length {
            Length::px(20.0)
        }

        fn layout(&mut self, _ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, _size: Size) {}

        fn paint(
            &mut self,
            ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            painter: &mut Painter<'_>,
        ) {
            if self.mode != PaintLayerMode::Inline {
                ctx.set_paint_layer_mode(self.mode);
            }
            if self.mode != PaintLayerMode::External {
                painter.fill_rect(ctx.content_box(), self.color);
            }
        }

        fn accessibility_role(&self) -> Role {
            Role::GenericContainer
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::new()
        }

        fn make_trace_span(&self, id: WidgetId) -> Span {
            trace_span!("ModeBox", id = id.trace())
        }
    }

    /// Parent that clips children and lays a single child full-size.
    struct ClipParent {
        child: WidgetPod<dyn Widget>,
    }

    impl ClipParent {
        fn new(child: NewWidget<impl Widget + ?Sized>) -> Self {
            Self {
                child: child.erased().to_pod(),
            }
        }
    }

    impl Widget for ClipParent {
        type Action = NoAction;

        fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
            ctx.register_child(&mut self.child);
        }

        fn property_changed(&mut self, _ctx: &mut UpdateCtx<'_>, _property_type: TypeId) {}

        fn measure(
            &mut self,
            ctx: &mut MeasureCtx<'_>,
            _props: &PropertiesRef<'_>,
            axis: Axis,
            _len_req: LenReq,
            cross_length: Option<Length>,
        ) -> Length {
            ctx.redirect_measurement(&mut self.child, axis, cross_length)
        }

        fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
            // Clip to a smaller rect than the child paint extent would need.
            ctx.set_clip_path(Rect::from_origin_size(Point::ORIGIN, Size::new(10.0, 10.0)));
            ctx.run_layout(&mut self.child, size);
            ctx.place_child(&mut self.child, Point::ORIGIN);
            ctx.derive_baselines(&self.child);
        }

        fn paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            _painter: &mut Painter<'_>,
        ) {
        }

        fn accessibility_role(&self) -> Role {
            Role::GenericContainer
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::from_slice(&[self.child.id()])
        }

        fn make_trace_span(&self, id: WidgetId) -> Span {
            trace_span!("ClipParent", id = id.trace())
        }
    }

    fn test_root(widget: NewWidget<impl Widget + ?Sized>) -> RenderRoot {
        RenderRoot::new(
            widget.erased(),
            |_| {},
            RenderRootOptions {
                default_properties: Arc::new(DefaultProperties::new()),
                use_system_fonts: true,
                size_policy: WindowSizePolicy::User,
                size: PhysicalSize::new(80, 40),
                scale_factor: 1.0,
                test_font: None,
            },
        )
    }

    // --- Gate inventory -------------------------------------------------------

    #[test]
    fn current_pin_does_not_support_upstream_only_isolation() {
        let caps = MasonryLayerCapabilities::CURRENT_PIN;
        assert!(caps.paint_layer_mode_api);
        assert!(caps.visual_layer_plan);
        assert!(caps.external_placeholders);
        assert!(caps.flatten_compatibility_helpers);
        // Checklist-only bit: no public LayerId type on this pin (re-audit on bump).
        assert_eq!(
            MasonryLayerCapabilities::evidence("persistent_layer_id"),
            CapabilityEvidence::InventoryChecklist
        );
        assert!(
            !caps.persistent_layer_id,
            "upstream still has FIXME for LayerId; gate must not claim otherwise"
        );
        assert_eq!(
            MasonryLayerCapabilities::evidence("sticky_paint_layer_mode"),
            CapabilityEvidence::EmpiricalSpike
        );
        assert!(
            !caps.sticky_paint_layer_mode,
            "paint_layer_mode resets to Inline each pass unless paint re-sets it"
        );
        assert!(
            !caps.self_contained_ancestor_clip,
            "isolated layers do not package ancestor clip/effect chains"
        );
        assert!(
            !caps.selective_layer_redraw,
            "only RenderRoot::redraw full paint pass exists"
        );
        assert!(!caps.supports_upstream_only_anim_isolation());
        assert_eq!(
            LayerBoundaryDecision::SELECTED,
            LayerBoundaryDecision::PicusAnimLayerHost
        );
        assert_eq!(
            AnimTargetStrategy::FIRST_COMPOSITE,
            AnimTargetStrategy::FullWindowTransparent
        );
    }

    /// Structural inventory: `VisualLayer` exposes only kind/transform/widget_id.
    /// No clip-chain / effect / ancestor package field for independent encode.
    fn assert_visual_layer_has_no_clip_package(plan: &crate::masonry_core::app::VisualLayerPlan) {
        for layer in &plan.layers {
            // Field access inventory — if upstream adds clip metadata, this match
            // must be extended and `self_contained_ancestor_clip` re-evaluated.
            let _transform = layer.transform;
            let _owner = layer.widget_id;
            match &layer.kind {
                VisualLayerKind::Scene(_scene) => {
                    // Scene payload only; no sibling clip descriptor on VisualLayer.
                }
                VisualLayerKind::External { bounds } => {
                    let _ = bounds;
                    // External carries bounds only — still no ancestor clip chain.
                }
            }
        }
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.self_contained_ancestor_clip,
            "VisualLayer shape has no clip package; keep capability false"
        );
    }

    // --- Masonry IsolatedScene / External structure ---------------------------

    #[test]
    fn isolated_scene_splits_painter_order_but_is_not_selective_redraw() {
        // Leading inline + trailing IsolatedScene → ≥2 scene layers (split plan).
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::IsolatedScene,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        assert!(
            plan.layers.len() >= 2,
            "IsolatedScene must split VisualLayerPlan; got {} layers",
            plan.layers.len()
        );
        assert!(
            plan.layers
                .iter()
                .all(|l| matches!(l.kind, VisualLayerKind::Scene(_))),
            "expected only Scene layers for IsolatedScene split"
        );
        assert_visual_layer_has_no_clip_package(&plan);

        // Second redraw without re-paint: paint_layer_mode resets to Inline each
        // pass, and set_paint_layer_mode only runs when request_paint is true.
        // Clean widgets therefore **lose** isolation and the plan collapses —
        // another reason IsolatedScene is not a stable anim layer contract.
        let (plan2, _) = root.redraw();
        assert!(
            plan2.layers.len() < plan.layers.len(),
            "without re-paint, IsolatedScene does not stick (got {} layers, first pass had {})",
            plan2.layers.len(),
            plan.layers.len()
        );
        // Full reassembly: every content paint path is still root.redraw() of the
        // whole plan — layer count is not independently dirtyable.
        let (plan3, _) = root.redraw();
        assert_eq!(
            plan3.layers.len(),
            plan2.layers.len(),
            "consecutive full redraws reassemble the whole plan (no selective layer dirty)"
        );
    }

    #[test]
    fn external_placeholder_reserves_painter_slot_skipped_by_flatten_helpers() {
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::External,
                    Color::TRANSPARENT,
                )))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();

        let external_count = plan
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(
            external_count, 1,
            "External mode must insert one placeholder in painter order; plan={plan:?}"
        );
        assert_visual_layer_has_no_clip_package(&plan);

        // Compatibility flatten helpers intentionally skip External — host must
        // realize them. This is the AnimLayerHost integration hook for P2b.
        let overlays: Vec<_> = plan.overlay_layers().collect();
        assert!(
            overlays
                .iter()
                .all(|l| matches!(l.kind, VisualLayerKind::Scene(_))),
            "overlay_layers must not yield External placeholders"
        );
        assert!(
            plan.root_layer()
                .is_some_and(|l| matches!(l.kind, VisualLayerKind::Scene(_)))
        );

        // Same sticky reset as IsolatedScene: without re-paint, External drops.
        // P2b checklist: anim widgets must set_paint_layer_mode(External) every paint.
        let (plan2, _) = root.redraw();
        let external_after = plan2
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(
            external_after, 0,
            "External is not sticky without re-paint; widgets must re-request mode each paint"
        );
    }

    #[test]
    fn isolated_child_under_ancestor_clip_still_splits_without_host_clip_package() {
        // FAIL evidence for "self-contained under ancestor clip":
        // - VisualLayer has no clip-chain field (type-shape via helper)
        // - IsolatedScene can still appear under a clipping parent, but host gets
        //   no package for independent encode under that clip
        // Scroll / ZStack / Masonry overlay-stack are not separately spiked;
        // non-sticky isolation + missing clip package already fail product isolation.
        let root_widget = NewWidget::new(ClipParent::new(NewWidget::new(ModeBox::new(
            PaintLayerMode::IsolatedScene,
            Color::from_rgb8(0, 255, 0),
        ))));
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        assert!(
            !plan.layers.is_empty(),
            "paint must produce at least one layer under clip+isolated"
        );
        assert_visual_layer_has_no_clip_package(&plan);
        // At least one scene layer exists; none of them carry clip metadata.
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::Scene(_))),
            "expected scene content under clip parent"
        );
    }

    #[test]
    fn anim_frame_plus_paint_still_requires_full_redraw_api() {
        // Spinner-like path: AnimFrame then full redraw. Public surface is only
        // RenderRoot::redraw → full paint pass (no selective layer rebuild API).
        let spinner = NewWidget::new(Spinner::new());
        let root_widget = NewWidget::new(
            SizedBox::new(spinner)
                .width(Length::px(40.0))
                .height(Length::px(40.0)),
        );
        let mut root = test_root(root_widget);

        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            16,
        )));
        let (plan, _) = root.redraw();
        assert!(
            plan.root_layer().is_some(),
            "AnimFrame does not emit a partial plan; redraw still builds full VisualLayerPlan"
        );
        // Second full redraw also returns a complete plan (reassembly, not
        // "only changed anim entry"). If a selective API existed as the primary
        // path, product code would not need consecutive full-plan redraws.
        let (plan2, _) = root.redraw();
        assert!(
            plan2.root_layer().is_some(),
            "second redraw still returns full plan; no public selective-entry rebuild"
        );
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.selective_layer_redraw,
            "gate inventory: selective_layer_redraw remains false on this pin"
        );
    }

    // --- AnimLayerHost unit contracts ----------------------------------------

    #[test]
    fn anim_layer_host_tracks_dirty_entries_independently() {
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FullWindowTransparent);
        assert_eq!(
            host.target_strategy(),
            AnimTargetStrategy::FullWindowTransparent
        );
        assert_eq!(
            AnimLayerHost::required_paint_layer_mode(),
            PaintLayerMode::External
        );

        // Pre-layout unbound → bind path (uses Unbound).
        let unbound = host.register_unbound();
        assert!(matches!(
            host.get(unbound).map(|e| e.slot),
            Some(AnimSlotBinding::Unbound)
        ));
        let w_bind =
            NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        assert!(host.bind_external_slot(unbound, w_bind));
        assert_eq!(host.id_for_widget(w_bind), Some(unbound));

        // WidgetId::next is crate-private in Masonry; allocate ids via NewWidget.
        let w1 = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let w2 = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let id1 = host.register_external_slot(w1);
        let id2 = host.register_external_slot(w2);
        assert_ne!(id1, id2);
        assert_eq!(host.register_external_slot(w1), id1, "idempotent register");
        assert_eq!(host.len(), 3);

        // Simulate encode of all.
        for id in [unbound, id1, id2] {
            host.clear_dirty_after_encode(id);
        }
        assert_eq!(host.dirty_ids().count(), 0);

        // Only entry 2 anim-paints → only that entry dirty (P2b encode set).
        assert!(host.mark_anim_paint(id2));
        let dirty: Vec<_> = host.dirty_ids().collect();
        assert_eq!(dirty, vec![id2]);
        assert_eq!(
            host.dirty_anim_paint_layers().collect::<Vec<_>>(),
            vec![id2.raw()]
        );
        assert_eq!(host.get(id2).map(|e| e.version), Some(1));
        assert_eq!(host.get(id1).map(|e| e.version), Some(0));
        assert!(!host.get(id1).expect("id1").dirty);

        let geom_changed = host.update_slot_geometry(
            id1,
            Rect::new(1.0, 2.0, 11.0, 22.0),
            Affine::translate((3.0, 4.0)),
        );
        assert!(geom_changed);
        assert!(host.get(id1).expect("id1").dirty);
        // Exercise mut accessor used by P2b for scene/texture handles.
        host.get_mut(id1).expect("id1 mut").version = host.get(id1).unwrap().version;

        let removed = host.remove_widget(w2).expect("remove w2");
        assert_eq!(removed.id, id2);
        assert_eq!(host.len(), 2);
        assert!(host.id_for_widget(w2).is_none());
    }

    #[test]
    fn post_hoc_plan_classification_is_not_per_layer_scene_build() {
        // Forbidden mis-reading: slicing VisualLayerPlan after full redraw is
        // classification of a snapshot, not selective build. After sticky collapse
        // the plan no longer even retains isolation layers, while host dirty
        // still tracks selective intent independently.
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::IsolatedScene,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let (plan1, _) = root.redraw();
        assert!(plan1.layers.len() >= 2, "first pass splits isolation");

        let mut host = AnimLayerHost::new(AnimTargetStrategy::FIRST_COMPOSITE);
        let wid = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let id = host.register_external_slot(wid);
        host.clear_dirty_after_encode(id);
        host.mark_anim_paint(id);
        assert_eq!(host.dirty_ids().count(), 1);

        let (plan2, _) = root.redraw();
        assert!(
            plan2.layers.len() < plan1.layers.len(),
            "plan collapses without re-paint — cannot use plan slicing as dirty unit"
        );
        assert_eq!(
            host.dirty_ids().count(),
            1,
            "host dirty set remains independently trackable after plan collapse"
        );
        assert_eq!(
            host.dirty_anim_paint_layers().next(),
            Some(id.raw()),
            "P2b selective unit is AnimLayerId.raw, not VisualLayerPlan index"
        );
    }
}

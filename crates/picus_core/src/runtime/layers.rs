//! Phase 2a hard gate: Masonry layer contract + anim target spike.
//!
//! This module records the **boundary decision** for layered anim encode before
//! multi-texture composite (P2b). It is **not** a full compositor.
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
//! | Self-contained independent entries | **Fail** — isolation can split the plan on a paint pass, but mode resets to Inline when the widget is not re-painted; ancestor clip/effect is not packaged; no persistent `LayerId`; hosts flatten via `root_layer`/`overlay_layers` |
//! | Selective anim entry without full redraw | **Fail** — only full `redraw()` → `run_paint_pass`; per-widget `scene_cache` skips re-record but still walks/reassembles the whole plan |
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

/// Capabilities of the pinned Masonry/xilem paint boundary (Phase 2a inventory).
///
/// Values are fixed for the git pin in workspace `Cargo.toml` (`xilem` rev
/// `4b1922c9728f7b86642b6759c6608f32e0badec2`). Re-run the module tests when
/// bumping the pin.
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
    #[allow(dead_code)] // Used by P2b dirty routing (`AnimPaint { layer }`).
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }
}

/// How a host entry maps into Masonry painter order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `Unbound` covers pre-layout registration in P2b.
pub(crate) enum AnimSlotBinding {
    /// Widget paints with [`PaintLayerMode::External`]; host fills the slot.
    ExternalPlaceholder { widget_id: WidgetId },
    /// No Masonry placeholder yet (registration before first paint / layout).
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
/// # Ownership / lifecycle (P2a contract)
///
/// ```text
/// WindowRuntime
///   ├── RenderRoot (Masonry)     layout / hit-test / External placeholders
///   ├── AnimLayerHost (Picus)    anim entry state + dirty/version
///   └── (P2b) LayerSurfaces      base + anim textures, exact-order composite
///
/// register(widget) ──► AnimLayerId + External paint mode on widget path
/// layout/compose    ──► update bounds/transform; CompositorPlan if plan changes
/// AnimFrame tick    ──► host marks entry dirty; NO base rewrite (target)
/// encode            ──► only dirty host entries (P2b); base only if base_dirty
/// remove/unmount    ──► drop entry; External slot disappears on next plan
/// ```
///
/// # TODO(P2b)
/// - Wire Spinner / indeterminate ProgressBar paint into host scenes
/// - Drive `DirtyReason::AnimPaint { layer }` from `AnimLayerId`
/// - Realize External slots in `PreparedFrame` / multi-texture composite
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

    /// Register (or return existing) anim entry for a Masonry widget id.
    pub(crate) fn register_external_slot(&mut self, widget_id: WidgetId) -> AnimLayerId {
        if let Some(&id) = self.by_widget.get(&widget_id) {
            return id;
        }
        let id = AnimLayerId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
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

    pub(crate) fn get(&self, id: AnimLayerId) -> Option<&AnimLayerEntry> {
        self.entries.get(&id)
    }

    #[allow(dead_code)] // P2b will mutate scene/texture handles on entries.
    pub(crate) fn get_mut(&mut self, id: AnimLayerId) -> Option<&mut AnimLayerEntry> {
        self.entries.get_mut(&id)
    }

    pub(crate) fn id_for_widget(&self, widget_id: WidgetId) -> Option<AnimLayerId> {
        self.by_widget.get(&widget_id).copied()
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

    /// Paint mode widgets should request so Masonry leaves an External slot.
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
        assert!(
            !caps.persistent_layer_id,
            "upstream still has FIXME for LayerId; gate must not claim otherwise"
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
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.selective_layer_redraw,
            "no public API to rebuild a single layer; only full redraw()"
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
    }

    #[test]
    fn isolated_child_under_ancestor_clip_still_splits_without_host_clip_package() {
        // Documents failure of "self-contained under ancestor clip":
        // Masonry will split IsolatedScene, but does not give Picus a clip-chain
        // descriptor on the layer — host would have to re-derive clip from the tree.
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
        // Capability flag is the authoritative gate bit for clip packaging.
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.self_contained_ancestor_clip,
            "must not claim ancestor clip is packaged on isolated layers"
        );
    }

    #[test]
    fn anim_frame_plus_paint_still_requires_full_redraw_api() {
        // Spinner-like path: AnimFrame then full redraw. No selective entry API.
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
        // Spinner arms anim + paint_only; host still has only full-plan redraw.
        let (plan, _) = root.redraw();
        assert!(
            plan.root_layer().is_some(),
            "AnimFrame does not emit a partial plan; redraw still builds full VisualLayerPlan"
        );
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.selective_layer_redraw,
            "gate: no selective layer redraw on current pin"
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

        // WidgetId::next is crate-private in Masonry; allocate ids via NewWidget.
        let w1 = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let w2 = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let id1 = host.register_external_slot(w1);
        let id2 = host.register_external_slot(w2);
        assert_ne!(id1, id2);
        assert_eq!(host.register_external_slot(w1), id1, "idempotent register");
        assert_eq!(host.len(), 2);

        // Simulate encode of both.
        host.clear_dirty_after_encode(id1);
        host.clear_dirty_after_encode(id2);
        assert_eq!(host.dirty_ids().count(), 0);

        // Only entry 2 anim-paints → only that entry dirty (P2b encode set).
        assert!(host.mark_anim_paint(id2));
        let dirty: Vec<_> = host.dirty_ids().collect();
        assert_eq!(dirty, vec![id2]);
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

        let removed = host.remove_widget(w2).expect("remove w2");
        assert_eq!(removed.id, id2);
        assert_eq!(host.len(), 1);
        assert!(host.id_for_widget(w2).is_none());
    }

    #[test]
    fn post_hoc_plan_classification_is_not_per_layer_scene_build() {
        // Document the forbidden mis-reading: inspecting VisualLayerPlan after
        // full redraw is a classification of a snapshot, not selective build.
        let note = "VisualLayerPlan is a full-pass painter-order snapshot; \
                    classifying layers post-hoc is not per-layer scene build";
        assert!(
            !MasonryLayerCapabilities::CURRENT_PIN.selective_layer_redraw,
            "{note}"
        );
        // Host dirty set is the Picus-side selective unit of work for P2b.
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FIRST_COMPOSITE);
        let wid = NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let id = host.register_external_slot(wid);
        host.clear_dirty_after_encode(id);
        host.mark_anim_paint(id);
        assert_eq!(
            host.dirty_ids().count(),
            1,
            "selective work is AnimLayerHost dirty set, not VisualLayerPlan slicing"
        );
    }
}

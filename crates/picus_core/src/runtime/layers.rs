//! Masonry layer contract (P2a) + ordered compositor (P2b) + Spinner (P2c) +
//! indeterminate ProgressBar (P2d) anim entries + public [`PaintIsolation`] (P3).
//!
//! ## Phase 2a gate (closed)
//!
//! Masonry alone cannot provide sticky isolation, self-contained ancestor clip,
//! or selective layer redraw on the pinned xilem rev. Selected path:
//! **Picus [`AnimLayerHost`]** + External painter slots +
//! [`AnimTargetStrategy::WidgetBoundsTexture`] for the product composite.
//!
//! ## Phase 2b (this module + `picus_surface`)
//!
//! Painter-order [`CompositorPlan`] of [`CompositorEntry`] values with stable
//! [`LayerId`]s. Entry kinds are **not** a fixed Base→Overlay→Anim stack —
//! order follows Masonry `VisualLayerPlan` (cached segments may appear both
//! before and after an anim/external slot).
//!
//! - [`LayerRegistry`] owns the plan + host; GPU textures live in
//!   `picus_surface` intermediate layer targets keyed by [`LayerId::raw`].
//! - Dirty/version: encode only entries whose content version advanced or
//!   structure (layout/clip/order/metrics) invalidated them.
//! - Resize/DPI bumps [`LayerRegistry::metrics_generation`]; all entry targets
//!   rebuild atomically — never mix old-size textures with a new plan.
//!
//! ## Phase 2c / 2d (Spinner + indeterminate ProgressBar)
//!
//! Widgets declare [`PaintIsolation`] (P3 public API). [`PaintIsolation::AnimEntry`]
//! applies as Masonry External every paint. **Promotion** is isolation-keyed
//! (`promotes_to_anim_host`); **discovery** of isolation still downcasts known
//! types (`paint_isolation()` on Spinner / ProgressBar) — not gallery/entity
//! hardcode, but not open third-party discovery either. Host **scene paint** is
//! separately type-dispatched (arms / segment). G2 selective anim path unchanged.
//!
//! ## Phase 3 ([`PaintIsolation`])
//!
//! - Public enum: [`PaintIsolation::{Inline, AnimEntry}`] in `picus_widget`
//!   (painter slot, not global top layer).
//! - Spinner / indeterminate ProgressBar default `AnimEntry`; determinate bar `Inline`.
//! - `register_external_widgets_from_visual` promotes External → Anim when the
//!   allowlisted live widget reports `AnimEntry` (stable [`AnimLayerId`] /
//!   compositor [`LayerId`]). Unknown External stays transparent placeholder.
//! - **Known limitation:** custom widgets that only `AnimEntry.apply` are not
//!   discovered; path forward is trait / TypeId host-painter registry (no
//!   inventory/linkme). See `docs/guide/paint-isolation.md`.
//!
//! ## Delivered vs not yet
//!
//! - **G10:** product path has no default anim present throttle; `PICUS_ANIM_PRESENT_HZ`
//!   is diagnostic opt-in only (positive Hz caps anim-driven presents).
//! - **G2 unit contracts:** Spinner + indeterminate ProgressBar host paths in-tree.
//! - **Not yet:** open custom-AnimEntry discovery; full PresentMon G3/G4 protocol
//!   numbers (baseline tables may still be placeholders — do not invent fake numbers).
//!
//! See `docs/guide/paint-isolation.md`, `docs/architecture/runtime.md`, and
//! `docs/plans/frame-pipeline.md`.

use std::collections::HashMap;

use crate::masonry_core::{
    app::{RenderRoot, VisualLayer, VisualLayerKind, VisualLayerPlan},
    core::{PaintLayerMode, WidgetId},
    imaging::{PaintSink, Painter, record::Scene},
    kurbo::{Affine, Rect, Size},
    peniko::color::{AlphaColor, Srgb},
};
use picus_widget::PaintIsolation;
use picus_widget::properties::{BarColor, BorderWidth, ContentColor, CornerRadius};
use picus_widget::widgets::{ProgressBar, Spinner};

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
    /// Compatibility/debug strategy that retains the original full-window target.
    FullWindowTransparent,
    /// Tight per-widget target placed back into painter order by the surface
    /// region compositor. Atlas packing can be added later without changing the
    /// host/window-space contract.
    #[default]
    WidgetBoundsTexture,
}

impl AnimTargetStrategy {
    /// Default product path after G2 product-path validation.
    pub(crate) const FIRST_COMPOSITE: Self = Self::WidgetBoundsTexture;
}

// ---------------------------------------------------------------------------
// AnimLayerHost — Picus anim entry state (wired via LayerRegistry in P2b)
// ---------------------------------------------------------------------------

/// Stable Picus-owned anim entry id (not a compositor [`LayerId`]).
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
/// GPU textures are **not** stored here — `picus_surface` holds intermediate
/// targets keyed by compositor [`LayerId`]. This type tracks ownership, dirty,
/// and the host-built [`Scene`] in window space for either target strategy.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AnimLayerEntry {
    pub id: AnimLayerId,
    pub slot: AnimSlotBinding,
    /// Window-space bounds last observed from layout (or placeholder).
    pub bounds: Rect,
    /// Window transform for the slot (identity when unbound).
    pub transform: Affine,
    /// Monotonic content version; bumps on anim paint / visual phase change.
    pub version: u64,
    /// Encode needed for this entry.
    pub dirty: bool,
    /// Host-built scene in **window space**; target cropping is applied later.
    pub scene: Scene,
    /// Last discrete visual phase baked into `scene` (Spinner: 0..12).
    pub visual_phase: Option<u8>,
    /// Last continuous phase baked into `scene` (ProgressBar indeterminate ∈ [0,1)).
    pub continuous_phase: Option<f64>,
}

/// Result of host scene sync for one anim widget (Spinner or ProgressBar).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct AnimWidgetSyncResult {
    /// Host content version advanced (needs encode).
    pub version_bumped: bool,
    /// Window bounds/transform changed (may invalidate base layout order).
    pub geometry_changed: bool,
    /// Discrete phase present on the host after the call (Spinner ack).
    pub visual_phase: Option<u8>,
    /// Continuous phase present on the host after the call (ProgressBar ack).
    pub continuous_phase: Option<f64>,
}

/// Aggregate of [`AnimWidgetSyncResult`] over a host sync pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AnimSyncSummary {
    /// Any host entry version advanced.
    pub any_version_bumped: bool,
    /// Any anim widget geometry moved (forces full base path — Issue 11).
    pub any_geometry_changed: bool,
}

/// Picus-side registry for isolated anim draw state.
///
/// Owned by [`LayerRegistry`] on [`super::WindowRuntime`]. Product widgets that
/// emit External every paint (Spinner in P2c, indeterminate ProgressBar in P2d)
/// register here; pure infrastructure frames may have an empty host and a single
/// [`CompositorEntryKind::CachedScene`].
///
/// ```text
/// WindowRuntime
///   ├── RenderRoot (Masonry)     layout / hit-test / External placeholders
///   ├── LayerRegistry
///   │     ├── AnimLayerHost      anim entry state + dirty/version
///   │     └── CompositorPlan     painter-order LayerId entries
///   └── ExternalWindowSurface    layer textures + ordered composite
/// ```
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
                scene: Scene::new(),
                visual_phase: None,
                continuous_phase: None,
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
                scene: Scene::new(),
                visual_phase: None,
                continuous_phase: None,
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

    /// Sync a Spinner anim scene in window space; bumps version only when the
    /// 12-step visual phase changes, geometry changes, or the scene was empty.
    pub(crate) fn sync_spinner_scene(
        &mut self,
        id: AnimLayerId,
        t: f64,
        size: Size,
        color: AlphaColor<Srgb>,
        window_transform: Affine,
        window_bounds: Rect,
    ) -> AnimWidgetSyncResult {
        let phase = Spinner::visual_phase(t);
        let Some(entry) = self.entries.get_mut(&id) else {
            return AnimWidgetSyncResult::default();
        };
        let phase_changed = entry.visual_phase != Some(phase);
        let geom_changed = entry.bounds != window_bounds || entry.transform != window_transform;
        let needs_build = phase_changed || geom_changed || entry.scene.is_empty();
        entry.bounds = window_bounds;
        entry.transform = window_transform;
        if !needs_build {
            // Still report phase so callers can ack the widget without rebuild.
            return AnimWidgetSyncResult {
                version_bumped: false,
                geometry_changed: false,
                visual_phase: Some(phase),
                continuous_phase: None,
            };
        }
        let mut local = Scene::new();
        {
            let sink: &mut dyn PaintSink = &mut local;
            let mut painter = Painter::new(sink);
            Spinner::paint_arms(&mut painter, size, t, color);
        }
        let mut scene = Scene::new();
        scene.append_transformed(&local, window_transform);
        entry.scene = scene;
        entry.visual_phase = Some(phase);
        entry.continuous_phase = None;
        entry.version = entry.version.saturating_add(1);
        entry.dirty = true;
        AnimWidgetSyncResult {
            version_bumped: true,
            geometry_changed: geom_changed,
            visual_phase: Some(phase),
            continuous_phase: None,
        }
    }

    /// Sync an indeterminate ProgressBar segment scene in window space.
    ///
    /// Bumps version when continuous phase changes, geometry changes, or the
    /// scene was empty. Theme property values are supplied by the caller.
    pub(crate) fn sync_progress_indeterminate_scene(
        &mut self,
        id: AnimLayerId,
        phase: f64,
        border_box: Rect,
        bar_color: AlphaColor<Srgb>,
        border_width: BorderWidth,
        corner_radius: CornerRadius,
        window_transform: Affine,
        window_bounds: Rect,
    ) -> AnimWidgetSyncResult {
        let Some(entry) = self.entries.get_mut(&id) else {
            return AnimWidgetSyncResult::default();
        };
        let phase_changed = entry.continuous_phase != Some(phase);
        let geom_changed = entry.bounds != window_bounds || entry.transform != window_transform;
        let needs_build = phase_changed || geom_changed || entry.scene.is_empty();
        entry.bounds = window_bounds;
        entry.transform = window_transform;
        if !needs_build {
            return AnimWidgetSyncResult {
                version_bumped: false,
                geometry_changed: false,
                visual_phase: None,
                continuous_phase: Some(phase),
            };
        }
        let mut local = Scene::new();
        {
            let sink: &mut dyn PaintSink = &mut local;
            let mut painter = Painter::new(sink);
            ProgressBar::paint_indeterminate_segment(
                &mut painter,
                border_box,
                phase,
                bar_color,
                &border_width,
                &corner_radius,
            );
        }
        let mut scene = Scene::new();
        scene.append_transformed(&local, window_transform);
        entry.scene = scene;
        entry.continuous_phase = Some(phase);
        entry.visual_phase = None;
        entry.version = entry.version.saturating_add(1);
        entry.dirty = true;
        AnimWidgetSyncResult {
            version_bumped: true,
            geometry_changed: geom_changed,
            visual_phase: None,
            continuous_phase: Some(phase),
        }
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

    /// Masonry widget ids currently bound to host anim entries.
    pub(crate) fn widget_ids(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.by_widget.keys().copied()
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
// Ordered compositor plan (P2.1–P2.2, P2.4, P2.6)
// ---------------------------------------------------------------------------

/// Stable compositor identity for one painter-order entry (Picus-owned).
///
/// Independent of [`WidgetId`] and [`AnimLayerId`]. Survives plan rebuilds when
/// the semantic identity ([`EntryIdentity`]) matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LayerId(u64);

impl LayerId {
    #[inline]
    pub(crate) const fn raw(self) -> u64 {
        self.0
    }
}

/// Kind of compositor entry. Stored **in Masonry painter order** — never regrouped
/// into a fixed Base→Overlay→Anim stack (P2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CompositorEntryKind {
    /// Cached Masonry scene segment (base content or split around anim slots).
    CachedScene,
    /// High-frequency anim content owned by [`AnimLayerHost`].
    Anim,
    /// Masonry overlay scene layer (tooltip / popup stack).
    Overlay,
    /// External placeholder without a bound host anim entry yet.
    External,
}

impl CompositorEntryKind {
    /// Attribute encode cost to base vs anim timing buckets (P2.5).
    #[inline]
    pub(crate) const fn is_anim_encode(self) -> bool {
        matches!(self, Self::Anim)
    }
}

/// Semantic key used to reuse [`LayerId`] across plan rebuilds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EntryIdentity {
    /// Contiguous cached-scene run index among CachedScene segments (0, 1, …).
    CachedSegment(u32),
    /// Overlay scene run index among Overlay segments.
    OverlaySegment(u32),
    /// Bound anim host entry.
    Anim(AnimLayerId),
    /// External placeholder widget without host binding.
    ExternalWidget(WidgetId),
}

/// Full ancestor clip package for independent encode (entry self-containment).
///
/// Upstream `VisualLayer` does not supply clip chains; Picus stores the package
/// on the entry. Empty means “no additional clip beyond the render target”.
///
/// **P2b status:** always [`AncestorClip::none`] at rebuild — intentionally
/// unpopulated until host/scene isolation supplies real clip chains (P2c+).
/// Encode/composite do not yet apply this field.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct AncestorClip {
    /// Clip rects in window space, outer → inner.
    pub rects: Vec<Rect>,
}

impl AncestorClip {
    #[inline]
    pub(crate) fn none() -> Self {
        Self::default()
    }

    #[inline]
    pub(crate) fn from_rects(rects: impl IntoIterator<Item = Rect>) -> Self {
        Self {
            rects: rects.into_iter().collect(),
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

/// Opacity / effect package carried with each entry (self-contained encode).
///
/// **P2b status:** always [`LayerEffect::OPAQUE`] at rebuild — intentionally
/// unpopulated until isolation supplies per-entry opacity. Composite does not
/// yet modulate by this field (would require blend-factor or Vello params).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayerEffect {
    pub opacity: f32,
}

impl Default for LayerEffect {
    fn default() -> Self {
        Self { opacity: 1.0 }
    }
}

impl LayerEffect {
    pub(crate) const OPAQUE: Self = Self { opacity: 1.0 };
}

/// One painter-order compositor entry (P2.2 contract).
///
/// Carries bounds, transform, full ancestor clip, opacity/effect, and content
/// version so encode can be independent of other entries once textures exist.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompositorEntry {
    pub id: LayerId,
    pub identity: EntryIdentity,
    pub kind: CompositorEntryKind,
    pub bounds: Rect,
    pub transform: Affine,
    pub ancestor_clip: AncestorClip,
    pub effect: LayerEffect,
    /// Monotonic content version; encode when differs from [`Self::encoded_version`].
    pub content_version: u64,
    /// Last successfully encoded content version (`None` = never encoded / FirstPaint).
    pub encoded_version: Option<u64>,
    /// Layout, clip, order, or metrics change — forces re-encode even if version matches.
    pub structure_dirty: bool,
    pub anim_id: Option<AnimLayerId>,
    pub widget_id: Option<WidgetId>,
}

impl CompositorEntry {
    /// True when this entry must be re-encoded before composite (P2.4).
    #[inline]
    pub(crate) fn needs_encode(&self) -> bool {
        self.structure_dirty || self.encoded_version != Some(self.content_version)
    }

    /// Mark encode succeeded at the current content version (only after present).
    pub(crate) fn mark_encoded(&mut self) {
        self.encoded_version = Some(self.content_version);
        self.structure_dirty = false;
    }

    /// Invalidate structure (layout/clip/order/metrics) without bumping content.
    pub(crate) fn invalidate_structure(&mut self) {
        self.structure_dirty = true;
    }

    /// Bump content version (pixel change).
    pub(crate) fn bump_content(&mut self) {
        self.content_version = self.content_version.saturating_add(1);
    }
}

/// Painter-order plan for one window (not Base→Overlay→Anim grouped).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CompositorPlan {
    entries: Vec<CompositorEntry>,
    /// Bumps when entry order/identity set changes.
    plan_version: u64,
}

impl CompositorPlan {
    #[inline]
    pub(crate) fn entries(&self) -> &[CompositorEntry] {
        &self.entries
    }

    #[inline]
    pub(crate) fn entries_mut(&mut self) -> &mut [CompositorEntry] {
        &mut self.entries
    }

    #[inline]
    #[allow(dead_code)] // Diagnostics / P2c plan-diff hooks.
    pub(crate) fn plan_version(&self) -> u64 {
        self.plan_version
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Single full-window cached scene — common path until anim widgets register.
    #[inline]
    pub(crate) fn is_single_cached(&self) -> bool {
        matches!(
            self.entries.as_slice(),
            [CompositorEntry {
                kind: CompositorEntryKind::CachedScene,
                ..
            }]
        )
    }

    /// True when an Anim entry sits between other entries (cached/overlay/external).
    pub(crate) fn has_anim_between_cached_segments(&self) -> bool {
        let mut saw_cached_before = false;
        let mut saw_anim = false;
        for e in &self.entries {
            match e.kind {
                CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay => {
                    if saw_anim && saw_cached_before {
                        return true;
                    }
                    if !saw_anim {
                        saw_cached_before = true;
                    } else {
                        return true;
                    }
                }
                CompositorEntryKind::Anim | CompositorEntryKind::External => {
                    if saw_cached_before {
                        saw_anim = true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn get(&self, id: LayerId) -> Option<&CompositorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: LayerId) -> Option<&mut CompositorEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Layer ids that need encode this frame (P2.4).
    pub(crate) fn dirty_encode_ids(&self) -> impl Iterator<Item = LayerId> + '_ {
        self.entries
            .iter()
            .filter(|e| e.needs_encode())
            .map(|e| e.id)
    }

    /// Invalidate every entry (FirstPaint / metrics rebuild).
    pub(crate) fn invalidate_all_structure(&mut self) {
        for e in &mut self.entries {
            e.invalidate_structure();
        }
    }

    /// Clear encode-dirty flags after successful present only.
    #[allow(dead_code)] // Prefer LayerRegistry::clear_dirty_after_successful_present.
    pub(crate) fn mark_all_encoded_that_were_dirty(&mut self) {
        for e in &mut self.entries {
            if e.needs_encode() {
                e.mark_encoded();
            }
        }
    }
}

/// Coalesced visual-plan run used by both plan rebuild and ordered encode.
///
/// Scene layers coalesce; each External is a singleton. Shared so
/// `rebuild_from_visual_plan` and `encode_ordered_composite` cannot drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VisualRun {
    /// Indices into `VisualLayerPlan::layers` (all Scene).
    Scenes(Vec<usize>),
    /// Index of a single External layer.
    External(usize),
}

#[derive(Debug, Clone, PartialEq)]
struct CachedSceneLayer {
    scene: Scene,
    transform: Affine,
}

/// Split a Masonry visual plan into painter-order runs (Issue 3: single source).
pub(crate) fn coalesce_visual_runs(visual: &VisualLayerPlan) -> Vec<VisualRun> {
    let mut runs: Vec<VisualRun> = Vec::new();
    for (idx, layer) in visual.layers.iter().enumerate() {
        match &layer.kind {
            VisualLayerKind::External { .. } => {
                runs.push(VisualRun::External(idx));
            }
            VisualLayerKind::Scene(_) => match runs.last_mut() {
                Some(VisualRun::Scenes(indices)) => indices.push(idx),
                _ => runs.push(VisualRun::Scenes(vec![idx])),
            },
        }
    }
    runs
}

/// Per-window layer plan + anim host (CPU-side; textures in `picus_surface`).
#[derive(Debug)]
pub(crate) struct LayerRegistry {
    plan: CompositorPlan,
    host: AnimLayerHost,
    next_layer_id: u64,
    /// Identity → stable LayerId across rebuilds.
    identity_ids: HashMap<EntryIdentity, LayerId>,
    /// Physical pixel size of entry targets for the current metrics generation.
    texture_width: u32,
    texture_height: u32,
    /// Bumps on resize/DPI; surface must drop all layer targets for old gen (P2.6).
    metrics_generation: u64,
    /// True when plan order/identity changed this rebuild.
    plan_changed: bool,
    /// Last recorded content for each static painter-order segment. Full Masonry
    /// redraws can then retain unchanged segment textures instead of invalidating
    /// every CachedScene/Overlay entry for a local hover or focus change.
    scene_snapshots: HashMap<EntryIdentity, Vec<CachedSceneLayer>>,
}

impl Default for LayerRegistry {
    fn default() -> Self {
        Self::new(AnimTargetStrategy::FIRST_COMPOSITE)
    }
}

impl LayerRegistry {
    pub(crate) fn new(target: AnimTargetStrategy) -> Self {
        Self {
            plan: CompositorPlan::default(),
            host: AnimLayerHost::new(target),
            next_layer_id: 1,
            identity_ids: HashMap::new(),
            texture_width: 0,
            texture_height: 0,
            metrics_generation: 1,
            plan_changed: false,
            scene_snapshots: HashMap::new(),
        }
    }

    #[inline]
    pub(crate) fn plan(&self) -> &CompositorPlan {
        &self.plan
    }

    #[inline]
    pub(crate) fn plan_mut(&mut self) -> &mut CompositorPlan {
        &mut self.plan
    }

    #[inline]
    pub(crate) fn host(&self) -> &AnimLayerHost {
        &self.host
    }

    #[inline]
    pub(crate) fn host_mut(&mut self) -> &mut AnimLayerHost {
        &mut self.host
    }

    #[inline]
    pub(crate) fn metrics_generation(&self) -> u64 {
        self.metrics_generation
    }

    #[inline]
    pub(crate) fn texture_size(&self) -> (u32, u32) {
        (self.texture_width, self.texture_height)
    }

    #[inline]
    pub(crate) fn plan_changed(&self) -> bool {
        self.plan_changed
    }

    fn alloc_layer_id(&mut self) -> LayerId {
        let id = LayerId(self.next_layer_id);
        self.next_layer_id = self.next_layer_id.saturating_add(1);
        id
    }

    fn layer_id_for(&mut self, identity: EntryIdentity) -> LayerId {
        if let Some(&id) = self.identity_ids.get(&identity) {
            return id;
        }
        let id = self.alloc_layer_id();
        self.identity_ids.insert(identity, id);
        id
    }

    fn update_scene_snapshot(
        &mut self,
        identity: EntryIdentity,
        visual: &VisualLayerPlan,
        indices: &[usize],
    ) -> bool {
        let unchanged = self.scene_snapshots.get(&identity).is_some_and(|previous| {
            previous.len() == indices.len()
                && previous.iter().zip(indices).all(|(cached, &index)| {
                    let layer = &visual.layers[index];
                    matches!(
                        &layer.kind,
                        VisualLayerKind::Scene(scene)
                            if cached.scene == *scene && cached.transform == layer.transform
                    )
                })
        });
        if unchanged {
            return false;
        }

        let snapshot = indices
            .iter()
            .filter_map(|&index| {
                let layer = &visual.layers[index];
                let VisualLayerKind::Scene(scene) = &layer.kind else {
                    return None;
                };
                Some(CachedSceneLayer {
                    scene: scene.clone(),
                    transform: layer.transform,
                })
            })
            .collect();
        self.scene_snapshots.insert(identity, snapshot);
        true
    }

    /// Resize/DPI: bump metrics generation and invalidate all entries (P2.6).
    ///
    /// Callers must rebuild surface layer targets for the new generation before
    /// encoding — old-size textures must not composite with the new plan.
    pub(crate) fn notify_metrics_changed(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.texture_width == width && self.texture_height == height && self.texture_width > 0 {
            return;
        }
        self.texture_width = width;
        self.texture_height = height;
        self.metrics_generation = self.metrics_generation.saturating_add(1);
        self.plan.invalidate_all_structure();
        // Content must re-encode into new targets (FirstPaint-equivalent).
        for e in self.plan.entries_mut() {
            e.encoded_version = None;
        }
    }

    /// First paint / surface recreate: force full entry encode set.
    pub(crate) fn notify_first_paint(&mut self) {
        self.plan.invalidate_all_structure();
        for e in self.plan.entries_mut() {
            e.encoded_version = None;
        }
    }

    /// Build painter-order plan from a Masonry visual plan + host bindings (P2.1).
    ///
    /// Consecutive Scene layers coalesce into one CachedScene or Overlay segment
    /// so an External/Anim slot can sit between cached segments. Overlay vs
    /// CachedScene uses Masonry flatten helpers: first Scene run is base
    /// (CachedScene); subsequent Scene runs that `overlay_layers` would yield
    /// are Overlay when no External splits them — with External present, Scene
    /// runs are CachedScene segments except pure overlay roots after content.
    ///
    /// Practical rule used here (honest, simple):
    /// - `VisualLayerKind::External` → Anim if host-bound, else External
    /// - `VisualLayerKind::Scene` coalesced runs → CachedScene until at least
    ///   one External has been seen **and** the scene is among `overlay_layers`
    ///   widget ids after the main stack; otherwise CachedScene.
    ///
    /// For the common single-root plan (no External), result is one CachedScene.
    pub(crate) fn rebuild_from_visual_plan(
        &mut self,
        visual: &VisualLayerPlan,
        window_bounds: Rect,
    ) {
        self.plan_changed = false;
        let overlay_widget_ids: std::collections::HashSet<WidgetId> = visual
            .overlay_layers()
            .map(|layer| layer.widget_id)
            .collect();

        let runs = coalesce_visual_runs(visual);
        let mut cached_seg = 0u32;
        let mut overlay_seg = 0u32;
        let mut next_entries: Vec<CompositorEntry> = Vec::with_capacity(runs.len());
        let mut saw_external = false;

        let mut changed_static_content = std::collections::HashSet::new();
        for run in &runs {
            match run {
                VisualRun::External(idx) => {
                    saw_external = true;
                    let layer = &visual.layers[*idx];
                    let local_bounds = match &layer.kind {
                        VisualLayerKind::External { bounds } => *bounds,
                        _ => window_bounds,
                    };
                    // Masonry exposes External bounds in layer-local coordinates.
                    // Host entries use window-space geometry so widget sync can
                    // compare directly with QueryCtx::bounding_box/window_transform.
                    let bounds = layer.transform.transform_rect_bbox(local_bounds);
                    let (kind, identity, anim_id) =
                        if let Some(anim) = self.host.id_for_widget(layer.widget_id) {
                            (
                                CompositorEntryKind::Anim,
                                EntryIdentity::Anim(anim),
                                Some(anim),
                            )
                        } else {
                            (
                                CompositorEntryKind::External,
                                EntryIdentity::ExternalWidget(layer.widget_id),
                                None,
                            )
                        };
                    // Sync host geometry when bound.
                    if let Some(aid) = anim_id {
                        let _ = self.host.update_slot_geometry(aid, bounds, layer.transform);
                    }
                    // Clip/effect intentionally none/opaque until P2c isolation.
                    next_entries.push(self.make_entry(
                        identity,
                        kind,
                        bounds,
                        layer.transform,
                        AncestorClip::none(),
                        LayerEffect::OPAQUE,
                        anim_id,
                        Some(layer.widget_id),
                    ));
                }
                VisualRun::Scenes(indices) => {
                    let first = &visual.layers[indices[0]];
                    // Masonry `overlay_layers()` treats every Scene after the first as an
                    // "overlay", including content that sits after an External slot. That
                    // is a flatten-helper artifact — not a true tooltip/popup overlay.
                    // Rule:
                    // - With External in the plan: all Scene runs are CachedScene segments
                    //   (anim sits between base content; trailing content is not Overlay).
                    // - Without External: first Scene run = CachedScene; later Scene runs
                    //   that overlay_layers would yield = Overlay.
                    let all_overlay = indices
                        .iter()
                        .all(|&i| overlay_widget_ids.contains(&visual.layers[i].widget_id));
                    let (kind, identity) = if !saw_external && all_overlay && cached_seg > 0 {
                        let id = EntryIdentity::OverlaySegment(overlay_seg);
                        overlay_seg = overlay_seg.saturating_add(1);
                        (CompositorEntryKind::Overlay, id)
                    } else {
                        let id = EntryIdentity::CachedSegment(cached_seg);
                        cached_seg = cached_seg.saturating_add(1);
                        (CompositorEntryKind::CachedScene, id)
                    };
                    if self.update_scene_snapshot(identity, visual, indices) {
                        changed_static_content.insert(identity);
                    }
                    // Union bounds of scenes in the run (window-space estimate).
                    let mut bounds = layer_bounds_estimate(first, window_bounds);
                    for &i in indices.iter().skip(1) {
                        let layer = &visual.layers[i];
                        bounds = bounds.union(layer_bounds_estimate(layer, window_bounds));
                    }
                    next_entries.push(self.make_entry(
                        identity,
                        kind,
                        bounds,
                        Affine::IDENTITY,
                        AncestorClip::none(),
                        LayerEffect::OPAQUE,
                        None,
                        Some(first.widget_id),
                    ));
                }
            }
        }

        // If Masonry produced no layers, keep a single full-window cached entry
        // so FirstPaint still has a target.
        if next_entries.is_empty() {
            next_entries.push(self.make_entry(
                EntryIdentity::CachedSegment(0),
                CompositorEntryKind::CachedScene,
                window_bounds,
                Affine::IDENTITY,
                AncestorClip::none(),
                LayerEffect::OPAQUE,
                None,
                None,
            ));
        }

        // Detect order/identity change.
        let old_ids: Vec<LayerId> = self.plan.entries.iter().map(|e| e.id).collect();
        let new_ids: Vec<LayerId> = next_entries.iter().map(|e| e.id).collect();
        if old_ids != new_ids {
            self.plan_changed = true;
            self.plan.plan_version = self.plan.plan_version.saturating_add(1);
            // Order change invalidates all cached segments (P2.4).
            for e in &mut next_entries {
                e.structure_dirty = true;
            }
        } else {
            // Preserve structure_dirty / versions from previous when id matches.
            for e in &mut next_entries {
                if let Some(prev) = self.plan.get(e.id) {
                    // Geometry/clip/effect changes invalidate structure.
                    if prev.bounds != e.bounds
                        || prev.transform != e.transform
                        || prev.ancestor_clip != e.ancestor_clip
                        || prev.effect != e.effect
                        || prev.kind != e.kind
                    {
                        e.structure_dirty = true;
                        e.encoded_version = prev.encoded_version;
                        e.content_version = prev.content_version;
                    } else {
                        e.structure_dirty = prev.structure_dirty;
                        e.encoded_version = prev.encoded_version;
                        e.content_version = prev.content_version;
                    }
                }
            }
        }

        for entry in &mut next_entries {
            if changed_static_content.contains(&entry.identity) {
                entry.bump_content();
            }
        }

        // Propagate host anim dirty → content version bump on Anim entries.
        for e in &mut next_entries {
            if let Some(anim_id) = e.anim_id
                && let Some(host_e) = self.host.get(anim_id)
            {
                if host_e.dirty {
                    // Align content version with host version.
                    if e.content_version != host_e.version {
                        e.content_version = host_e.version;
                    } else if e.encoded_version == Some(e.content_version) {
                        // Host dirty but versions equal — force bump.
                        e.bump_content();
                    }
                    e.structure_dirty |= host_e.bounds != e.bounds;
                }
                e.bounds = host_e.bounds;
                e.transform = host_e.transform;
            }
        }

        self.plan.entries = next_entries;

        // Drop identity map entries that are no longer present.
        let live: std::collections::HashSet<EntryIdentity> =
            self.plan.entries.iter().map(|e| e.identity).collect();
        self.identity_ids.retain(|k, _| live.contains(k));
        self.scene_snapshots.retain(|k, _| live.contains(k));
    }

    #[allow(clippy::too_many_arguments)]
    fn make_entry(
        &mut self,
        identity: EntryIdentity,
        kind: CompositorEntryKind,
        bounds: Rect,
        transform: Affine,
        ancestor_clip: AncestorClip,
        effect: LayerEffect,
        anim_id: Option<AnimLayerId>,
        widget_id: Option<WidgetId>,
    ) -> CompositorEntry {
        let id = self.layer_id_for(identity);
        CompositorEntry {
            id,
            identity,
            kind,
            bounds,
            transform,
            ancestor_clip,
            effect,
            content_version: 0,
            encoded_version: None,
            structure_dirty: true,
            anim_id,
            widget_id,
        }
    }

    /// Bump content version on CachedScene/Overlay when Masonry content may have
    /// changed without geometry/order change (Issue 2).
    ///
    /// Call on `InputOrRebuild` / `ThemeOrFont` / `LayoutRewrite` / etc. before
    /// encode. Pure `AnimPaint` must **not** call this — anim-only ticks leave
    /// base segments clean for future G2.
    #[cfg(test)]
    pub(crate) fn mark_non_anim_content_dirty(&mut self) {
        for e in self.plan.entries_mut() {
            if matches!(
                e.kind,
                CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay
            ) {
                e.bump_content();
            }
        }
    }

    /// Live compositor layer ids (for surface `retain_layer_targets`).
    pub(crate) fn live_layer_id_raws(&self) -> Vec<u64> {
        self.plan.entries.iter().map(|e| e.id.raw()).collect()
    }

    /// Mark successful encode/present for dirty entries; clear host dirty (sticky present).
    pub(crate) fn clear_dirty_after_successful_present(&mut self) {
        for e in self.plan.entries_mut() {
            if e.needs_encode() {
                if let Some(anim_id) = e.anim_id {
                    self.host.clear_dirty_after_encode(anim_id);
                }
                e.mark_encoded();
            }
        }
        self.plan_changed = false;
    }

    /// On failed/retry present: retain dirty (do not clear encoded_version gaps).
    pub(crate) fn retain_dirty_after_failed_present(&mut self) {
        // Intentional no-op body: needs_encode stays true; host dirty stays.
        // Documented so call sites are explicit.
    }

    /// Layer ids for surface intermediate targets in painter order.
    #[allow(dead_code)] // Used by diagnostics / future selective present paths.
    pub(crate) fn ordered_layer_ids(&self) -> impl Iterator<Item = LayerId> + '_ {
        self.plan.entries.iter().map(|e| e.id)
    }

    /// Whether the ordered multi-texture path should run (vs single full-window encode).
    pub(crate) fn prefers_ordered_composite(&self) -> bool {
        self.plan.len() > 1
            || self
                .plan
                .entries()
                .iter()
                .any(|e| !matches!(e.kind, CompositorEntryKind::CachedScene))
    }

    /// Bind External painter slots whose live widgets resolve to
    /// [`PaintIsolation::AnimEntry`] (stock: Spinner; indeterminate ProgressBar).
    ///
    /// Unbound External widgets stay [`CompositorEntryKind::External`] (transparent
    /// placeholder) — they are **not** promoted to Anim with an empty host scene.
    /// No gallery/entity hardcode. **Promotion** is isolation-keyed
    /// (`promotes_to_anim_host`); **discovery** uses [`widget_paint_isolation`]
    /// (closed type allowlist). Host entries for widgets that left External /
    /// became determinate (`Inline`) are pruned. Allocates a stable
    /// [`AnimLayerId`] per widget; compositor [`LayerId`] follows plan identity.
    pub(crate) fn register_external_widgets_from_visual(
        &mut self,
        visual: &VisualLayerPlan,
        render_root: &RenderRoot,
    ) {
        let mut live_host: std::collections::HashSet<WidgetId> = std::collections::HashSet::new();
        for layer in &visual.layers {
            if !matches!(layer.kind, VisualLayerKind::External { .. }) {
                continue;
            }
            if widget_paint_isolation(render_root, layer.widget_id).promotes_to_anim_host() {
                self.host.register_external_slot(layer.widget_id);
                live_host.insert(layer.widget_id);
            }
        }
        // Prune host slots no longer External with AnimEntry isolation (e.g. ProgressBar
        // switched to determinate Inline, or widget removed from External set).
        let stale: Vec<WidgetId> = self
            .host
            .widget_ids()
            .filter(|id| !live_host.contains(id))
            .collect();
        for id in stale {
            let _ = self.host.remove_widget(id);
        }
    }

    /// Propagate host dirty/version onto Anim compositor entries.
    pub(crate) fn propagate_host_dirty_to_plan(&mut self) {
        for e in self.plan.entries_mut() {
            if let Some(anim_id) = e.anim_id
                && let Some(host_e) = self.host.get(anim_id)
            {
                if host_e.dirty {
                    if e.content_version != host_e.version {
                        e.content_version = host_e.version;
                    } else if e.encoded_version == Some(e.content_version) {
                        e.bump_content();
                    }
                    e.structure_dirty |= host_e.bounds != e.bounds;
                }
                e.bounds = host_e.bounds;
                e.transform = host_e.transform;
            }
        }
    }

    /// Rebuild host scenes from live widgets (Spinner / indeterminate ProgressBar).
    ///
    /// Phase ack is deferred until successful present (Issue 13).
    /// Prunes host entries whose widgets left the tree.
    pub(crate) fn sync_anim_entries_from_widgets(
        &mut self,
        render_root: &RenderRoot,
    ) -> AnimSyncSummary {
        let orphans: Vec<WidgetId> = self
            .host
            .widget_ids()
            .filter(|&id| !render_root.has_widget(id))
            .collect();
        for id in orphans {
            let _ = self.host.remove_widget(id);
        }

        let pairs: Vec<(AnimLayerId, WidgetId)> = self
            .plan
            .entries()
            .iter()
            .filter_map(|e| Some((e.anim_id?, e.widget_id?)))
            .collect();
        let mut summary = AnimSyncSummary::default();
        for (anim_id, widget_id) in pairs {
            let one = sync_one_anim_widget(self, render_root, anim_id, widget_id);
            summary.any_version_bumped |= one.version_bumped;
            summary.any_geometry_changed |= one.geometry_changed;
        }
        if summary.any_version_bumped {
            self.propagate_host_dirty_to_plan();
        }
        summary
    }

    /// True when the plan has at least one Anim entry (selective G2 path eligible).
    pub(crate) fn has_anim_entries(&self) -> bool {
        self.plan
            .entries()
            .iter()
            .any(|e| e.kind == CompositorEntryKind::Anim)
    }

    /// True when any non-Anim entry needs encode (blocks pure-anim selective path).
    pub(crate) fn non_anim_needs_encode(&self) -> bool {
        self.plan
            .entries()
            .iter()
            .any(|e| e.needs_encode() && !matches!(e.kind, CompositorEntryKind::Anim))
    }

    /// Encode-set for G2 assertion: only Anim kinds need encode (base clean).
    pub(crate) fn only_anim_needs_encode(&self) -> bool {
        let mut saw_anim_dirty = false;
        for e in self.plan.entries() {
            if e.needs_encode() {
                if e.kind != CompositorEntryKind::Anim {
                    return false;
                }
                saw_anim_dirty = true;
            }
        }
        saw_anim_dirty
    }

    /// After a **successful** present, ack Spinner / ProgressBar phases on live widgets.
    ///
    /// Selective G2 never runs Masonry `paint`; host scene build alone must not
    /// ack (Failed would strand dirty host with no `request_paint_only` — Issue 13).
    pub(crate) fn ack_anim_phases_after_present(&self, render_root: &RenderRoot) {
        for e in self.plan.entries() {
            let (Some(anim_id), Some(widget_id)) = (e.anim_id, e.widget_id) else {
                continue;
            };
            let Some(host_e) = self.host.get(anim_id) else {
                continue;
            };
            let Some(wref) = render_root.get_widget(widget_id) else {
                continue;
            };
            if let Some(phase) = host_e.visual_phase {
                if let Some(spinner) = wref.downcast::<Spinner>() {
                    spinner.inner().ack_visual_phase(phase);
                    continue;
                }
            }
            if let Some(phase) = host_e.continuous_phase {
                if let Some(bar) = wref.downcast::<ProgressBar>() {
                    bar.inner().ack_indeterminate_phase(phase);
                }
            }
        }
    }

    /// Backward-compatible name used by Spinner-era call sites / tests.
    #[inline]
    pub(crate) fn ack_spinner_phases_after_present(&self, render_root: &RenderRoot) {
        self.ack_anim_phases_after_present(render_root);
    }
}

/// Resolve public [`PaintIsolation`] for a live widget (host promotion gate).
///
/// **Discovery** is a closed allowlist: downcast known types and call their
/// `paint_isolation()`. Unknown widgets default to [`PaintIsolation::Inline`]
/// so bare External placeholders are not promoted to empty Anim entries.
///
/// Once resolved, the **enum value** decides promotion (`promotes_to_anim_host`).
/// Host **scene paint** is a separate type-dispatched step (Spinner arms /
/// ProgressBar segment). Custom retained widgets are not discoverable yet —
/// see module docs and `docs/guide/paint-isolation.md`.
fn widget_paint_isolation(render_root: &RenderRoot, widget_id: WidgetId) -> PaintIsolation {
    let Some(wref) = render_root.get_widget(widget_id) else {
        return PaintIsolation::Inline;
    };
    if let Some(spinner) = wref.downcast::<Spinner>() {
        return spinner.inner().paint_isolation();
    }
    if let Some(bar) = wref.downcast::<ProgressBar>() {
        return bar.inner().paint_isolation();
    }
    PaintIsolation::Inline
}

/// Downcast + host scene sync for one bound widget (Spinner / ProgressBar).
fn sync_one_anim_widget(
    registry: &mut LayerRegistry,
    render_root: &RenderRoot,
    anim_id: AnimLayerId,
    widget_id: WidgetId,
) -> AnimWidgetSyncResult {
    let Some(wref) = render_root.get_widget(widget_id) else {
        return AnimWidgetSyncResult::default();
    };

    // QueryCtx::window_transform maps content-box → window space.
    let window_transform = wref.ctx().window_transform();
    let window_bounds = wref.ctx().bounding_box();

    if let Some(spinner) = wref.downcast::<Spinner>() {
        let t = spinner.inner().t();
        let size = wref.ctx().content_box().size();
        let color = wref.get_prop::<ContentColor>().color;
        let _ = spinner;
        let _ = wref;
        // Phase ack deferred until successful present (Issue 13).
        return registry.host_mut().sync_spinner_scene(
            anim_id,
            t,
            size,
            color,
            window_transform,
            window_bounds,
        );
    }

    if let Some(bar) = wref.downcast::<ProgressBar>() {
        if !bar.inner().paint_isolation().promotes_to_anim_host() {
            // Determinate / Inline: drop host binding. Signal geometry_changed so
            // selective encode cannot keep an empty Anim entry — force full plan
            // rebuild (defense-in-depth if progress flipped without content dirt).
            let _ = bar;
            let _ = wref;
            let _ = registry.host_mut().remove_widget(widget_id);
            return AnimWidgetSyncResult {
                version_bumped: false,
                geometry_changed: true,
                visual_phase: None,
                continuous_phase: None,
            };
        }
        let phase = bar.inner().indeterminate_phase();
        // Masonry `border_box()` is content-box space (may have negative origin
        // when BorderWidth/Padding insets are non-zero). Pass through unchanged —
        // `window_transform()` expects the same space; ORIGIN+size would misalign
        // the host segment vs track chrome in pre_paint.
        let border_box = wref.ctx().border_box();
        let bar_color = wref.get_prop::<BarColor>().0;
        let border_width = *wref.get_prop::<BorderWidth>();
        let corner_radius = *wref.get_prop::<CornerRadius>();
        let _ = bar;
        let _ = wref;
        return registry.host_mut().sync_progress_indeterminate_scene(
            anim_id,
            phase,
            border_box,
            bar_color,
            border_width,
            corner_radius,
            window_transform,
            window_bounds,
        );
    }

    // Unknown External: leave empty transparent host scene.
    AnimWidgetSyncResult::default()
}

fn layer_bounds_estimate(layer: &VisualLayer, window_bounds: Rect) -> Rect {
    match &layer.kind {
        VisualLayerKind::External { bounds } => *bounds,
        VisualLayerKind::Scene(_) => window_bounds,
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
        app::{
            RenderRoot, RenderRootOptions, VisualLayer, VisualLayerKind, VisualLayerPlan,
            WindowSizePolicy,
        },
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
    use picus_widget::widgets::{Flex, ProgressBar, SizedBox, Spinner};

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

    /// Exercises [`PaintIsolation::apply`] only (not host-known type).
    struct IsolationBox {
        isolation: PaintIsolation,
    }

    impl IsolationBox {
        fn new(isolation: PaintIsolation) -> Self {
            Self { isolation }
        }
    }

    impl Widget for IsolationBox {
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
            _painter: &mut Painter<'_>,
        ) {
            self.isolation.apply(ctx);
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
            trace_span!("IsolationBox", id = id.trace())
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
            AnimTargetStrategy::WidgetBoundsTexture
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

    /// Masonry severs the current layer when External is under an ancestor clip,
    /// leaving PushClip without PopClip (`UnclosedClips`). Picus encode balances
    /// stacks in `picus_imaging` so gallery (Spinner/ProgressBar under Portal) works.
    #[test]
    fn external_under_ancestor_clip_raw_scenes_may_be_unbalanced_but_balanceable() {
        let root_widget = NewWidget::new(ClipParent::new(NewWidget::new(ModeBox::new(
            PaintLayerMode::External,
            Color::TRANSPARENT,
        ))));
        let mut root = test_root(root_widget);
        // First paint applies External (mode is not sticky across clean redraws).
        let (plan, _) = root.redraw();
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "External under clip must still reserve a placeholder; plan={plan:?}"
        );

        let mut raw_invalid = false;
        for layer in &plan.layers {
            if let VisualLayerKind::Scene(scene) = &layer.kind {
                if scene.validate().is_err() {
                    raw_invalid = true;
                    let balanced = picus_imaging::balance_scene_stacks(scene);
                    assert_eq!(
                        balanced.validate(),
                        Ok(()),
                        "picus_imaging must repair Masonry mid-clip layer splits"
                    );
                }
            }
        }
        // Document the upstream bug: at least one scene is unbalanced when
        // External finishes under ClipParent (gallery Portal + AnimEntry path).
        assert!(
            raw_invalid,
            "expected at least one unbalanced scene from External under clip \
             (if this fails, Masonry fixed layer splits — drop the balance workaround)"
        );
    }

    #[test]
    fn anim_frame_plus_paint_still_requires_full_redraw_api() {
        // Spinner-like path: AnimFrame then full redraw. Public surface is only
        // RenderRoot::redraw → full paint pass (no selective layer rebuild API).
        // Spinner uses External isolation, so the plan may be External-only
        // (root_layer skips External) — non-empty layers still prove full reassembly.
        let spinner = NewWidget::new(Spinner::new());
        let root_widget = NewWidget::new(
            SizedBox::new(spinner)
                .width(Length::px(40.0))
                .height(Length::px(40.0)),
        );
        let mut root = test_root(root_widget);

        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(16)));
        let (plan, _) = root.redraw();
        assert!(
            !plan.layers.is_empty(),
            "AnimFrame does not emit a partial plan; redraw still builds full VisualLayerPlan"
        );
        // Second full redraw also returns a complete plan (reassembly, not
        // "only changed anim entry"). If a selective API existed as the primary
        // path, product code would not need consecutive full-plan redraws.
        let (plan2, _) = root.redraw();
        assert!(
            !plan2.layers.is_empty(),
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
    fn external_plan_geometry_is_normalized_to_window_space() {
        let widget_id =
            NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let mut registry = LayerRegistry::default();
        let anim_id = registry.host_mut().register_external_slot(widget_id);
        let local_bounds = Rect::new(2.0, 3.0, 12.0, 13.0);
        let transform = Affine::translate((40.0, 50.0));
        let visual = VisualLayerPlan {
            layers: vec![VisualLayer {
                kind: VisualLayerKind::External {
                    bounds: local_bounds,
                },
                transform,
                widget_id,
            }],
        };

        registry.rebuild_from_visual_plan(&visual, Rect::new(0.0, 0.0, 200.0, 200.0));

        let host = registry.host().get(anim_id).expect("bound anim host");
        assert_eq!(host.bounds, transform.transform_rect_bbox(local_bounds));
        assert_eq!(host.transform, transform);
        assert!(
            !registry.host_mut().update_slot_geometry(
                anim_id,
                transform.transform_rect_bbox(local_bounds),
                transform,
            ),
            "re-observing window-space geometry must not dirty the anim host"
        );
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

    // --- CompositorPlan / LayerRegistry (P2.1–P2.2, P2.4, P2.6) -------------

    #[test]
    fn compositor_entry_needs_encode_respects_version_and_structure() {
        let mut entry = CompositorEntry {
            id: LayerId(1),
            identity: EntryIdentity::CachedSegment(0),
            kind: CompositorEntryKind::CachedScene,
            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
            transform: Affine::IDENTITY,
            ancestor_clip: AncestorClip::from_rects([Rect::new(0.0, 0.0, 5.0, 5.0)]),
            effect: LayerEffect { opacity: 0.5 },
            content_version: 1,
            encoded_version: None,
            structure_dirty: false,
            anim_id: None,
            widget_id: None,
        };
        // Self-contained package fields are present (P2.2).
        assert!(!entry.ancestor_clip.is_empty());
        assert!((entry.effect.opacity - 0.5).abs() < f32::EPSILON);
        assert!(entry.needs_encode(), "never encoded");

        entry.mark_encoded();
        assert!(!entry.needs_encode());

        entry.bump_content();
        assert!(entry.needs_encode(), "version change");
        entry.mark_encoded();
        entry.invalidate_structure();
        assert!(entry.needs_encode(), "structure dirty");
    }

    #[test]
    fn layer_registry_builds_painter_order_with_anim_between_cached() {
        // Cached → External(Anim) → Cached: NOT fixed Base→Overlay→Anim grouping.
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
        // Capture external widget id before move into tree — rebuild mode each paint.
        // ModeBox ids are inside Flex; re-allocate a tracking id via host after plan.
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "need External in plan for this test"
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        // Bind host to the External widget so kind becomes Anim.
        let ext_wid = plan
            .layers
            .iter()
            .find(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .map(|l| l.widget_id)
            .expect("external widget");
        let anim_id = registry.host_mut().register_external_slot(ext_wid);
        registry.host_mut().clear_dirty_after_encode(anim_id);

        let window_bounds = Rect::new(0.0, 0.0, 80.0, 40.0);
        registry.rebuild_from_visual_plan(&plan, window_bounds);

        let kinds: Vec<_> = registry.plan().entries().iter().map(|e| e.kind).collect();
        // Flex Inline|External|Inline must produce Cached|Anim|Cached (Issue 6).
        assert_eq!(
            kinds,
            vec![
                CompositorEntryKind::CachedScene,
                CompositorEntryKind::Anim,
                CompositorEntryKind::CachedScene,
            ],
            "painter-order split for Inline|External|Inline; plan layers={:?}",
            plan.layers.len()
        );
        assert!(
            registry.plan().has_anim_between_cached_segments(),
            "anim must sit between cached segments"
        );
        // Shared coalesce must match plan entry count.
        let runs = coalesce_visual_runs(&plan);
        assert_eq!(runs.len(), kinds.len());

        // Stable LayerId across rebuild.
        let ids_before: Vec<_> = registry.plan().entries().iter().map(|e| e.id).collect();
        registry.rebuild_from_visual_plan(&plan, window_bounds);
        let ids_after: Vec<_> = registry.plan().entries().iter().map(|e| e.id).collect();
        assert_eq!(
            ids_before, ids_after,
            "LayerId stable across identical rebuild"
        );
    }

    #[test]
    fn full_redraw_only_dirties_static_segment_whose_scene_changed() {
        fn solid_scene(color: Color) -> Scene {
            let mut scene = Scene::new();
            Painter::new(&mut scene).fill_rect(Rect::new(0.0, 0.0, 20.0, 20.0), color);
            scene
        }

        let before_widget =
            NewWidget::new(ModeBox::new(PaintLayerMode::Inline, Color::TRANSPARENT)).id();
        let external_widget =
            NewWidget::new(ModeBox::new(PaintLayerMode::External, Color::TRANSPARENT)).id();
        let after_widget =
            NewWidget::new(ModeBox::new(PaintLayerMode::Inline, Color::TRANSPARENT)).id();
        let make_plan = |after_color| VisualLayerPlan {
            layers: vec![
                VisualLayer {
                    kind: VisualLayerKind::Scene(solid_scene(Color::from_rgb8(1, 2, 3))),
                    transform: Affine::IDENTITY,
                    widget_id: before_widget,
                },
                VisualLayer {
                    kind: VisualLayerKind::External {
                        bounds: Rect::new(20.0, 0.0, 40.0, 20.0),
                    },
                    transform: Affine::IDENTITY,
                    widget_id: external_widget,
                },
                VisualLayer {
                    kind: VisualLayerKind::Scene(solid_scene(after_color)),
                    transform: Affine::IDENTITY,
                    widget_id: after_widget,
                },
            ],
        };

        let mut registry = LayerRegistry::default();
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        registry.rebuild_from_visual_plan(&make_plan(Color::from_rgb8(4, 5, 6)), bounds);
        registry.clear_dirty_after_successful_present();

        registry.rebuild_from_visual_plan(&make_plan(Color::from_rgb8(4, 5, 6)), bounds);
        assert_eq!(
            registry.plan().dirty_encode_ids().count(),
            0,
            "identical full redraw must reuse every static segment texture"
        );

        registry.rebuild_from_visual_plan(&make_plan(Color::from_rgb8(7, 8, 9)), bounds);
        let dirty: Vec<_> = registry
            .plan()
            .entries()
            .iter()
            .filter(|entry| entry.needs_encode())
            .map(|entry| entry.identity)
            .collect();
        assert_eq!(
            dirty,
            vec![EntryIdentity::CachedSegment(1)],
            "a local static change must not invalidate sibling cached segments"
        );
    }

    #[test]
    fn non_anim_content_dirt_bumps_cached_after_present_without_geometry_change() {
        // Issue 2 regression: after successful present, InputOrRebuild-class dirt
        // must re-encode CachedScene even when bounds/order are unchanged.
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
        let ext_wid = plan
            .layers
            .iter()
            .find(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .map(|l| l.widget_id)
            .expect("external");

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FIRST_COMPOSITE);
        let anim_id = registry.host_mut().register_external_slot(ext_wid);
        registry.host_mut().clear_dirty_after_encode(anim_id);
        let bounds = Rect::new(0.0, 0.0, 80.0, 40.0);
        registry.rebuild_from_visual_plan(&plan, bounds);
        registry.clear_dirty_after_successful_present();
        assert_eq!(
            registry.plan().dirty_encode_ids().count(),
            0,
            "clean after present"
        );

        // Rebuild with same geometry (no structure change).
        registry.rebuild_from_visual_plan(&plan, bounds);
        assert_eq!(
            registry.plan().dirty_encode_ids().count(),
            0,
            "geometry-stable rebuild alone must not dirty"
        );

        // Simulate InputOrRebuild / theme content dirt without AnimPaint.
        registry.mark_non_anim_content_dirty();
        let dirty_kinds: Vec<_> = registry
            .plan()
            .entries()
            .iter()
            .filter(|e| e.needs_encode())
            .map(|e| e.kind)
            .collect();
        assert!(
            dirty_kinds.iter().all(|k| matches!(
                k,
                CompositorEntryKind::CachedScene | CompositorEntryKind::Overlay
            )),
            "only CachedScene/Overlay dirtied, got {dirty_kinds:?}"
        );
        assert!(
            dirty_kinds
                .iter()
                .any(|k| *k == CompositorEntryKind::CachedScene),
            "CachedScene must need re-encode after content dirt"
        );
        // Anim entry stays clean when only non-anim content dirt is marked.
        let anim_dirty = registry
            .plan()
            .entries()
            .iter()
            .any(|e| e.kind == CompositorEntryKind::Anim && e.needs_encode());
        assert!(
            !anim_dirty,
            "Anim must not bump from mark_non_anim_content_dirty"
        );
    }

    #[test]
    fn dirty_version_encodes_only_changed_entries() {
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FIRST_COMPOSITE);
        let window_bounds = Rect::new(0.0, 0.0, 100.0, 100.0);

        // Synthetic single-cached plan via empty visual → one CachedScene.
        let root_widget = NewWidget::new(ModeBox::new(
            PaintLayerMode::Inline,
            Color::from_rgb8(1, 2, 3),
        ));
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        registry.rebuild_from_visual_plan(&plan, window_bounds);
        assert!(registry.plan().is_single_cached() || !registry.plan().is_empty());

        // First paint: all dirty.
        let dirty_first: Vec<_> = registry.plan().dirty_encode_ids().collect();
        assert!(!dirty_first.is_empty());
        registry.clear_dirty_after_successful_present();
        assert_eq!(registry.plan().dirty_encode_ids().count(), 0);

        // Anim host dirt only affects anim entry when present; for single cached,
        // bump content on the cached entry.
        let id = registry.plan().entries()[0].id;
        registry.plan_mut().get_mut(id).unwrap().bump_content();
        let dirty: Vec<_> = registry.plan().dirty_encode_ids().collect();
        assert_eq!(dirty, vec![id]);

        // Failed present retains dirty (P2.6 / sticky).
        registry.retain_dirty_after_failed_present();
        assert_eq!(registry.plan().dirty_encode_ids().count(), 1);
    }

    #[test]
    fn metrics_change_invalidates_all_and_bumps_generation() {
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FIRST_COMPOSITE);
        let root_widget = NewWidget::new(ModeBox::new(
            PaintLayerMode::Inline,
            Color::from_rgb8(1, 2, 3),
        ));
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        registry.clear_dirty_after_successful_present();
        let gen0 = registry.metrics_generation();

        registry.notify_metrics_changed(200, 100);
        assert_eq!(registry.metrics_generation(), gen0 + 1);
        assert_eq!(registry.texture_size(), (200, 100));
        assert_eq!(
            registry.plan().dirty_encode_ids().count(),
            registry.plan().len(),
            "all entries dirty after metrics change"
        );
        for e in registry.plan().entries() {
            assert!(
                e.encoded_version.is_none(),
                "FirstPaint-equivalent after resize"
            );
        }

        // Same size is a no-op.
        registry.clear_dirty_after_successful_present();
        registry.notify_metrics_changed(200, 100);
        assert_eq!(registry.metrics_generation(), gen0 + 1);
    }

    #[test]
    fn entry_identity_kinds_cover_compositor_entry_kind_set() {
        // Inventory: all four kinds exist and anim encodes map correctly.
        assert!(CompositorEntryKind::Anim.is_anim_encode());
        assert!(!CompositorEntryKind::CachedScene.is_anim_encode());
        assert!(!CompositorEntryKind::Overlay.is_anim_encode());
        assert!(!CompositorEntryKind::External.is_anim_encode());
        let _ = [
            CompositorEntryKind::CachedScene,
            CompositorEntryKind::Anim,
            CompositorEntryKind::Overlay,
            CompositorEntryKind::External,
        ];
    }

    #[test]
    fn prefers_ordered_composite_when_non_single_cached() {
        let mut registry = LayerRegistry::default();
        assert!(!registry.prefers_ordered_composite() || registry.plan().is_empty());

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
        let ext = plan
            .layers
            .iter()
            .find(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .unwrap()
            .widget_id;
        registry.host_mut().register_external_slot(ext);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        assert!(
            registry.prefers_ordered_composite(),
            "External/Anim plan must use ordered composite path"
        );
    }

    // --- P2c Spinner anim entry / G2 progress --------------------------------

    #[test]
    fn spinner_visual_phase_has_twelve_steps() {
        assert_eq!(Spinner::PHASE_COUNT, 12);
        assert_eq!(Spinner::visual_phase(0.0), 0);
        assert_eq!(Spinner::visual_phase(1.0 / 12.0 - 1e-9), 0);
        assert_eq!(Spinner::visual_phase(1.0 / 12.0), 1);
        assert_eq!(Spinner::visual_phase(0.5), 6);
        assert_eq!(Spinner::visual_phase(11.0 / 12.0), 11);
        // rem_euclid: t=1 maps like 0 after rem in on_anim_frame; raw 1.0 floors to 12 → 0.
        assert_eq!(Spinner::visual_phase(1.0), 0);
    }

    #[test]
    fn paint_isolation_defaults_for_spinner_and_progress_bar() {
        assert_eq!(
            Spinner::new().paint_isolation(),
            PaintIsolation::AnimEntry,
            "Spinner always AnimEntry"
        );
        assert_eq!(
            ProgressBar::new(None).paint_isolation(),
            PaintIsolation::AnimEntry,
            "indeterminate ProgressBar AnimEntry"
        );
        assert_eq!(
            ProgressBar::new(Some(0.5)).paint_isolation(),
            PaintIsolation::Inline,
            "determinate ProgressBar Inline"
        );
    }

    #[test]
    fn paint_isolation_apply_anim_entry_sets_external_slot() {
        // Harness-level: PaintIsolation::apply(AnimEntry) must reserve External
        // in VisualLayerPlan (not only enum→mode unit mapping).
        let root_widget = NewWidget::new(IsolationBox::new(PaintIsolation::AnimEntry));
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        let external_count = plan
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(
            external_count, 1,
            "AnimEntry.apply must set External; plan={plan:?}"
        );
    }

    #[test]
    fn paint_isolation_apply_inline_does_not_create_external() {
        let root_widget = NewWidget::new(IsolationBox::new(PaintIsolation::Inline));
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        let external_count = plan
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(external_count, 0, "Inline.apply is a no-op; plan={plan:?}");
    }

    #[test]
    fn undiscovered_anim_entry_apply_is_not_promoted_to_anim() {
        // Known limitation: apply alone → External placeholder, not host Anim
        // (IsolationBox is not on the discovery allowlist).
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(IsolationBox::new(PaintIsolation::AnimEntry)))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let (plan, _) = root.redraw();
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "apply must still produce External"
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        let kinds: Vec<_> = registry.plan().entries().iter().map(|e| e.kind).collect();
        assert!(
            kinds.iter().any(|k| *k == CompositorEntryKind::External),
            "undiscovered AnimEntry stays External; kinds={kinds:?}"
        );
        assert!(
            !kinds.iter().any(|k| *k == CompositorEntryKind::Anim),
            "must not invent Anim without discovery; kinds={kinds:?}"
        );
        assert_eq!(registry.host().len(), 0);
    }

    #[test]
    fn spinner_external_isolation_promotes_to_anim_entry() {
        // Plan/isolation contract (not a pixel scan of CachedScene blobs):
        // Spinner discovery + AnimEntry → External slot → host Anim entry.
        // Also covers Spinner paint_isolation().apply → External end-to-end.
        let spinner = NewWidget::new(Spinner::new());
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(spinner)
                        .width(Length::px(40.0))
                        .height(Length::px(40.0)),
                ))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        // AnimFrame then redraw so Spinner paint sets External.
        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            100,
        )));
        let (plan, _) = root.redraw();
        let external_count = plan
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(
            external_count, 1,
            "Spinner must reserve one External painter slot; plan={plan:?}"
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        let kinds: Vec<_> = registry.plan().entries().iter().map(|e| e.kind).collect();
        assert!(
            kinds.iter().any(|k| *k == CompositorEntryKind::Anim),
            "Spinner External → Anim; kinds={kinds:?}"
        );
        assert!(
            registry.has_anim_entries(),
            "anim entry present in painter-order plan"
        );
    }

    #[test]
    fn non_spinner_external_is_not_promoted_to_anim() {
        // ModeBox External has PaintIsolation::Inline (unknown) — stays External, not empty Anim.
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
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. }))
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        let kinds: Vec<_> = registry.plan().entries().iter().map(|e| e.kind).collect();
        assert!(
            kinds.iter().any(|k| *k == CompositorEntryKind::External),
            "non-Spinner External stays External; kinds={kinds:?}"
        );
        assert!(
            !kinds.iter().any(|k| *k == CompositorEntryKind::Anim),
            "must not invent Anim with empty host scene for non-Spinner External"
        );
        assert_eq!(registry.host().len(), 0);
    }

    #[test]
    fn host_spinner_phase_gate_skips_version_when_phase_unchanged() {
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FullWindowTransparent);
        let w = NewWidget::new(Spinner::new()).id();
        let id = host.register_external_slot(w);
        let color = AlphaColor::from_rgb8(255, 255, 255);
        let size = Size::new(40.0, 40.0);
        let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
        let xf = Affine::IDENTITY;

        assert!(
            host.sync_spinner_scene(id, 0.0, size, color, xf, bounds)
                .version_bumped
        );
        let v1 = host.get(id).unwrap().version;
        // Same phase (0 for t in [0, 1/12)): no version bump.
        assert!(
            !host
                .sync_spinner_scene(id, 0.01, size, color, xf, bounds)
                .version_bumped
        );
        assert_eq!(host.get(id).unwrap().version, v1);
        // Next phase.
        assert!(
            host.sync_spinner_scene(id, 1.0 / 12.0, size, color, xf, bounds)
                .version_bumped
        );
        assert_eq!(host.get(id).unwrap().version, v1 + 1);
        assert!(!host.get(id).unwrap().scene.is_empty());
    }

    #[test]
    fn pure_anim_dirt_only_anim_entry_needs_encode_g2() {
        // G2 contract at the layer level: after a clean present, only host anim
        // dirt from a real phase advance marks Anim for encode — CachedScene stays
        // clean (encode_base=0). No force-mark fallback; drive phase explicitly.
        let spinner = NewWidget::new(Spinner::new());
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(spinner)
                        .width(Length::px(40.0))
                        .height(Length::px(40.0)),
                ))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        // First phase paint + plan.
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        let first_sync = registry.sync_anim_entries_from_widgets(&root);
        assert!(
            first_sync.any_version_bumped,
            "initial host scene build must dirty anim"
        );
        let version_after_first = registry
            .plan()
            .entries()
            .iter()
            .find_map(|e| {
                e.anim_id
                    .and_then(|id| registry.host().get(id).map(|h| h.version))
            })
            .expect("host version");
        registry.clear_dirty_after_successful_present();
        assert_eq!(
            registry.plan().dirty_encode_ids().count(),
            0,
            "clean after present"
        );
        assert!(!registry.non_anim_needs_encode());

        // Advance ≥1 visual phase (1/12 s ≈ 83.3ms). Do NOT redraw — G2 path.
        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            100,
        )));
        let dirtied = registry.sync_anim_entries_from_widgets(&root);
        assert!(
            dirtied.any_version_bumped,
            "100ms anim must advance Spinner visual phase and dirt host via sync"
        );
        let version_after_phase = registry
            .plan()
            .entries()
            .iter()
            .find_map(|e| {
                e.anim_id
                    .and_then(|id| registry.host().get(id).map(|h| h.version))
            })
            .expect("host version after phase");
        assert!(
            version_after_phase > version_after_first,
            "host version must advance on phase change: {version_after_first} → {version_after_phase}"
        );

        assert!(
            registry.only_anim_needs_encode(),
            "G2: only Anim needs encode; dirty={:?}",
            registry
                .plan()
                .entries()
                .iter()
                .filter(|e| e.needs_encode())
                .map(|e| e.kind)
                .collect::<Vec<_>>()
        );
        assert!(
            !registry.non_anim_needs_encode(),
            "base CachedScene must not re-encode on pure anim dirt"
        );
    }

    #[test]
    fn host_sync_does_not_ack_phase_until_present_helper() {
        // Issues 12+13: selective never paints; ack only after successful present
        // so Failed can re-request paint while host stays dirty.
        let spinner = NewWidget::new(Spinner::new());
        let spinner_id = spinner.id();
        let root_widget = NewWidget::new(
            SizedBox::new(spinner)
                .width(Length::px(40.0))
                .height(Length::px(40.0)),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));

        // Force unacked sentinel.
        root.get_widget(spinner_id)
            .unwrap()
            .downcast::<Spinner>()
            .unwrap()
            .inner()
            .ack_visual_phase(255);

        let _ = registry.sync_anim_entries_from_widgets(&root);
        assert_eq!(
            root.get_widget(spinner_id)
                .unwrap()
                .downcast::<Spinner>()
                .unwrap()
                .inner()
                .acked_visual_phase(),
            Some(255),
            "sync alone must not ack (Failed present would strand encode retry)"
        );

        // Successful-present helper acks the host visual phase.
        registry.ack_spinner_phases_after_present(&root);
        let expected = root
            .get_widget(spinner_id)
            .unwrap()
            .downcast::<Spinner>()
            .unwrap()
            .inner()
            .phase();
        assert_eq!(
            root.get_widget(spinner_id)
                .unwrap()
                .downcast::<Spinner>()
                .unwrap()
                .inner()
                .acked_visual_phase(),
            Some(expected),
            "ack_spinner_phases_after_present must ack host phase"
        );
    }

    #[test]
    fn failed_present_retains_host_dirty_for_encode_retry() {
        // Issue 13: after Failed selective present, host dirty must remain so
        // post_dirty can re-insert AnimPaint without a new request_paint_only.
        let spinner = NewWidget::new(Spinner::new());
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(spinner)
                        .width(Length::px(40.0))
                        .height(Length::px(40.0)),
                )),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 80.0, 40.0));
        assert!(
            registry
                .sync_anim_entries_from_widgets(&root)
                .any_version_bumped
        );

        // Simulate Failed present: retain dirty, do not ack phases.
        registry.retain_dirty_after_failed_present();
        let dirty_layers: Vec<_> = registry.host().dirty_anim_paint_layers().collect();
        assert!(
            !dirty_layers.is_empty(),
            "host must stay dirty after Failed present"
        );
        assert!(
            registry.only_anim_needs_encode() || registry.plan().dirty_encode_ids().count() > 0,
            "plan Anim entries must still need encode after Failed"
        );

        // post_dirty merge contract: host dirty alone is enough for AnimPaint.
        use crate::runtime::frame_driver::{DirtyBudget, DirtyReason};
        let mut post = DirtyBudget::new();
        post.insert(DirtyReason::AnimTick);
        for layer in registry.host().dirty_anim_paint_layers() {
            post.insert(DirtyReason::AnimPaint { layer });
        }
        assert!(
            post.needs_content_present(),
            "host dirty → AnimPaint must request encode after Failed (not pure AnimTick)"
        );
        assert!(
            post.is_selective_anim_encode()
                || post.has(DirtyReason::AnimPaint {
                    layer: dirty_layers[0]
                })
        );

        // Successful present clears host dirty; only then would ack run.
        registry.clear_dirty_after_successful_present();
        assert_eq!(registry.host().dirty_ids().count(), 0);
        assert_eq!(registry.plan().dirty_encode_ids().count(), 0);
    }

    #[test]
    fn geometry_change_reported_from_spinner_sync() {
        // Issue 11 partial: bounds move reports geometry_changed for full-path fallthrough.
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FullWindowTransparent);
        let w = NewWidget::new(Spinner::new()).id();
        let id = host.register_external_slot(w);
        let color = AlphaColor::from_rgb8(255, 255, 255);
        let size = Size::new(40.0, 40.0);
        let r1 = host.sync_spinner_scene(
            id,
            0.0,
            size,
            color,
            Affine::IDENTITY,
            Rect::new(0.0, 0.0, 40.0, 40.0),
        );
        assert!(r1.version_bumped);
        // First build: bounds from ZERO → rect counts as geometry_changed.
        assert!(r1.geometry_changed);
        // Same geom / phase: no bump, no geom change.
        let r2 = host.sync_spinner_scene(
            id,
            0.0,
            size,
            color,
            Affine::IDENTITY,
            Rect::new(0.0, 0.0, 40.0, 40.0),
        );
        assert!(!r2.version_bumped);
        assert!(!r2.geometry_changed);
        let r3 = host.sync_spinner_scene(
            id,
            0.0,
            size,
            color,
            Affine::IDENTITY,
            Rect::new(10.0, 10.0, 50.0, 50.0),
        );
        assert!(r3.version_bumped);
        assert!(r3.geometry_changed);
    }

    // --- P2d indeterminate ProgressBar ---------------------------------------

    #[test]
    fn progress_phase_at_zero_half_and_period() {
        assert_eq!(ProgressBar::phase_from_elapsed(0.0), 0.0);
        assert!((ProgressBar::phase_from_elapsed(0.6) - 0.5).abs() < 1e-12);
        assert!((ProgressBar::phase_from_elapsed(1.2)).abs() < 1e-12);
        // Cross 1.2s period.
        assert!((ProgressBar::phase_from_elapsed(1.8) - 0.5).abs() < 1e-12);
        // Large delta skip wraps.
        let phase = ProgressBar::phase_from_elapsed(5.0);
        assert!((phase - (5.0_f64 / 1.2).rem_euclid(1.0)).abs() < 1e-12);
        assert!((ProgressBar::segment_left_frac(0.0) - (-0.3)).abs() < 1e-12);
        assert!((ProgressBar::segment_width_frac() - 0.3).abs() < 1e-12);
    }

    #[test]
    fn indeterminate_progress_external_promotes_to_anim() {
        let bar = NewWidget::new(ProgressBar::new(None));
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(bar)
                        .width(Length::px(120.0))
                        .height(Length::px(20.0)),
                ))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        let external_count = plan
            .layers
            .iter()
            .filter(|l| matches!(l.kind, VisualLayerKind::External { .. }))
            .count();
        assert_eq!(
            external_count, 1,
            "indeterminate ProgressBar must reserve External; plan={plan:?}"
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 200.0, 40.0));
        let kinds: Vec<_> = registry.plan().entries().iter().map(|e| e.kind).collect();
        assert!(
            kinds.iter().any(|k| *k == CompositorEntryKind::Anim),
            "indeterminate ProgressBar External → Anim; kinds={kinds:?}"
        );
    }

    #[test]
    fn determinate_progress_is_not_external_or_anim() {
        let bar = NewWidget::new(ProgressBar::new(Some(0.5)));
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(bar)
                        .width(Length::px(120.0))
                        .height(Length::px(20.0)),
                )),
        );
        let mut root = test_root(root_widget);
        // Even after AnimFrame, determinate must not schedule isolation.
        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            100,
        )));
        let (plan, _) = root.redraw();
        assert!(
            !plan
                .layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "determinate ProgressBar must stay Inline; plan={plan:?}"
        );
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(registry.host().len(), 0);
        assert!(!registry.has_anim_entries());
    }

    #[test]
    fn host_progress_phase_gate_and_large_delta() {
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FullWindowTransparent);
        let w = NewWidget::new(ProgressBar::new(None)).id();
        let id = host.register_external_slot(w);
        let color = AlphaColor::from_rgb8(0, 120, 212);
        let border = BorderWidth::default();
        let radius = CornerRadius::default();
        let local = Rect::new(0.0, 0.0, 100.0, 16.0);
        let bounds = Rect::new(0.0, 0.0, 100.0, 16.0);
        let xf = Affine::IDENTITY;

        // t=0 / phase 0
        assert!(
            host.sync_progress_indeterminate_scene(
                id, 0.0, local, color, border, radius, xf, bounds
            )
            .version_bumped
        );
        let v0 = host.get(id).unwrap().version;
        // Same phase: no bump.
        assert!(
            !host
                .sync_progress_indeterminate_scene(
                    id, 0.0, local, color, border, radius, xf, bounds
                )
                .version_bumped
        );
        assert_eq!(host.get(id).unwrap().version, v0);

        // t=600ms → phase 0.5
        assert!(
            host.sync_progress_indeterminate_scene(
                id, 0.5, local, color, border, radius, xf, bounds
            )
            .version_bumped
        );
        assert_eq!(host.get(id).unwrap().version, v0 + 1);
        assert!(!host.get(id).unwrap().scene.is_empty());

        // Cross period: phase wraps to 0 again → bump (continuous phase key).
        assert!(
            host.sync_progress_indeterminate_scene(
                id, 0.0, local, color, border, radius, xf, bounds
            )
            .version_bumped
        );

        // Large delta target phase still rebuilds when different.
        let jump = ProgressBar::phase_from_elapsed(5.0);
        assert!(
            host.sync_progress_indeterminate_scene(
                id, jump, local, color, border, radius, xf, bounds
            )
            .version_bumped
        );
    }

    #[test]
    fn host_progress_segment_uses_content_space_border_origin() {
        // Non-zero BorderWidth: content-space border_box has negative origin.
        // Host scene build must receive that rect, not ORIGIN+size.
        use crate::masonry_core::layout::Length;
        let mut host = AnimLayerHost::new(AnimTargetStrategy::FullWindowTransparent);
        let w = NewWidget::new(ProgressBar::new(None)).id();
        let id = host.register_external_slot(w);
        let color = AlphaColor::from_rgb8(0, 120, 212);
        let border = BorderWidth::all(Length::px(10.0));
        let radius = CornerRadius::default();
        let content_space = Rect::new(-10.0, -10.0, 110.0, 30.0);
        let wrong_origin = Rect::from_origin_size(Point::ORIGIN, content_space.size());

        let correct_seg =
            ProgressBar::indeterminate_segment_rect(content_space, 0.5, &border, &radius)
                .expect("segment with content-space origin");
        let wrong_seg =
            ProgressBar::indeterminate_segment_rect(wrong_origin, 0.5, &border, &radius)
                .expect("segment with ORIGIN rewrite");
        assert!(
            (correct_seg.x0 - wrong_seg.x0).abs() > 1.0,
            "content-space origin shifts segment vs ORIGIN rewrite: {} vs {}",
            correct_seg.x0,
            wrong_seg.x0
        );
        assert!(
            correct_seg.x0 < 0.0 || correct_seg.x0 < wrong_seg.x0,
            "insets pull segment toward negative content origin"
        );

        assert!(
            host.sync_progress_indeterminate_scene(
                id,
                0.5,
                content_space,
                color,
                border,
                radius,
                Affine::IDENTITY,
                Rect::new(0.0, 0.0, 120.0, 40.0),
            )
            .version_bumped
        );
        assert!(!host.get(id).unwrap().scene.is_empty());
    }

    #[test]
    fn pure_anim_progress_dirt_only_anim_needs_encode_g2() {
        // G2: after clean present, only host anim dirt from phase advance marks
        // Anim for encode — CachedScene stays clean.
        let bar = NewWidget::new(ProgressBar::new(None));
        let root_widget = NewWidget::new(
            Flex::row()
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(255, 0, 0),
                )))
                .with_fixed(NewWidget::new(
                    SizedBox::new(bar)
                        .width(Length::px(120.0))
                        .height(Length::px(20.0)),
                ))
                .with_fixed(NewWidget::new(ModeBox::new(
                    PaintLayerMode::Inline,
                    Color::from_rgb8(0, 0, 255),
                ))),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 200.0, 40.0));
        let first = registry.sync_anim_entries_from_widgets(&root);
        assert!(
            first.any_version_bumped,
            "initial ProgressBar host scene must dirty anim"
        );
        let version_after_first = registry
            .plan()
            .entries()
            .iter()
            .find_map(|e| {
                e.anim_id
                    .and_then(|id| registry.host().get(id).map(|h| h.version))
            })
            .expect("host version");
        registry.clear_dirty_after_successful_present();
        assert_eq!(registry.plan().dirty_encode_ids().count(), 0);
        assert!(!registry.non_anim_needs_encode());

        // Advance 600ms (half period) without full redraw — G2 path.
        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            600,
        )));
        let dirtied = registry.sync_anim_entries_from_widgets(&root);
        assert!(
            dirtied.any_version_bumped,
            "600ms must advance continuous phase and dirt host"
        );
        let version_after = registry
            .plan()
            .entries()
            .iter()
            .find_map(|e| {
                e.anim_id
                    .and_then(|id| registry.host().get(id).map(|h| h.version))
            })
            .expect("host version after phase");
        assert!(
            version_after > version_after_first,
            "host version must advance: {version_after_first} → {version_after}"
        );
        assert!(
            registry.only_anim_needs_encode(),
            "G2: only Anim needs encode on pure ProgressBar anim dirt"
        );
        assert!(
            !registry.non_anim_needs_encode(),
            "base CachedScene must not re-encode on pure anim dirt"
        );
    }

    #[test]
    fn progress_some_none_lifecycle_toggles_isolation() {
        let bar = NewWidget::new(ProgressBar::new(Some(0.25)));
        let bar_id = bar.id();
        let root_widget = NewWidget::new(
            SizedBox::new(bar)
                .width(Length::px(120.0))
                .height(Length::px(20.0)),
        );
        let mut root = test_root(root_widget);

        // Determinate: no External.
        let (plan, _) = root.redraw();
        assert!(
            !plan
                .layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. }))
        );

        // Some → None: reset phase, start anim isolation.
        root.edit_widget(bar_id, |mut w| {
            let mut bar = w.downcast::<ProgressBar>();
            ProgressBar::set_progress(&mut bar, None);
        });
        {
            let phase = root
                .get_widget(bar_id)
                .unwrap()
                .downcast::<ProgressBar>()
                .unwrap()
                .inner()
                .indeterminate_phase();
            assert_eq!(phase, 0.0, "Some→None resets phase to 0 before first tick");
            let elapsed = root
                .get_widget(bar_id)
                .unwrap()
                .downcast::<ProgressBar>()
                .unwrap()
                .inner()
                .indeterminate_elapsed();
            assert_eq!(elapsed, 0.0, "Some→None resets elapsed");
        }
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        assert!(
            plan.layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "None must enable External isolation"
        );

        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 120.0, 20.0));
        assert!(registry.has_anim_entries());

        // None → Some: drop isolation and host binding; no permanent anim tick.
        root.edit_widget(bar_id, |mut w| {
            let mut bar = w.downcast::<ProgressBar>();
            ProgressBar::set_progress(&mut bar, Some(0.75));
        });
        // Drain one already-scheduled AnimFrame from the prior indeterminate mode.
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(16)));
        assert!(
            !root.needs_anim(),
            "after None→Some + one drain frame, determinate must not keep needs_anim"
        );
        let (plan, _) = root.redraw();
        assert!(
            !plan
                .layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "Some must leave External isolation"
        );
        // Further anim frames must not re-arm External.
        let _ = root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(
            100,
        )));
        assert!(!root.needs_anim(), "determinate stays off the anim clock");
        let (plan2, _) = root.redraw();
        assert!(
            !plan2
                .layers
                .iter()
                .any(|l| matches!(l.kind, VisualLayerKind::External { .. })),
            "further ticks must not re-arm External"
        );
        registry.register_external_widgets_from_visual(&plan2, &root);
        registry.rebuild_from_visual_plan(&plan2, Rect::new(0.0, 0.0, 120.0, 20.0));
        assert_eq!(
            registry.host().len(),
            0,
            "determinate must prune host anim slot"
        );
        assert!(!registry.has_anim_entries());
    }

    #[test]
    fn determinate_during_sync_reports_geometry_changed() {
        // Defense-in-depth: if a plan still has an Anim entry but the widget is
        // already determinate, sync prunes host and forces full-path fallthrough.
        let bar = NewWidget::new(ProgressBar::new(None));
        let bar_id = bar.id();
        let root_widget = NewWidget::new(
            SizedBox::new(bar)
                .width(Length::px(120.0))
                .height(Length::px(20.0)),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 120.0, 20.0));
        assert!(registry.has_anim_entries());

        // Flip to determinate without going through register prune first.
        root.edit_widget(bar_id, |mut w| {
            let mut bar = w.downcast::<ProgressBar>();
            ProgressBar::set_progress(&mut bar, Some(0.5));
        });
        let summary = registry.sync_anim_entries_from_widgets(&root);
        assert!(
            summary.any_geometry_changed,
            "determinate during selective sync must force full-path (geometry_changed)"
        );
        assert_eq!(
            registry.host().len(),
            0,
            "host slot removed when widget is determinate"
        );
    }

    #[test]
    fn progress_ack_only_after_present_helper() {
        let bar = NewWidget::new(ProgressBar::new(None));
        let bar_id = bar.id();
        let root_widget = NewWidget::new(
            SizedBox::new(bar)
                .width(Length::px(120.0))
                .height(Length::px(20.0)),
        );
        let mut root = test_root(root_widget);
        let _ =
            root.handle_window_event(WindowEvent::AnimFrame(std::time::Duration::from_millis(1)));
        let (plan, _) = root.redraw();
        let mut registry = LayerRegistry::new(AnimTargetStrategy::FullWindowTransparent);
        registry.register_external_widgets_from_visual(&plan, &root);
        registry.rebuild_from_visual_plan(&plan, Rect::new(0.0, 0.0, 120.0, 20.0));

        // Force unacked sentinel distinct from live phase.
        root.get_widget(bar_id)
            .unwrap()
            .downcast::<ProgressBar>()
            .unwrap()
            .inner()
            .ack_indeterminate_phase(-1.0);

        let _ = registry.sync_anim_entries_from_widgets(&root);
        assert_eq!(
            root.get_widget(bar_id)
                .unwrap()
                .downcast::<ProgressBar>()
                .unwrap()
                .inner()
                .acked_indeterminate_phase(),
            Some(-1.0),
            "sync alone must not ack ProgressBar phase"
        );

        registry.ack_anim_phases_after_present(&root);
        let expected = root
            .get_widget(bar_id)
            .unwrap()
            .downcast::<ProgressBar>()
            .unwrap()
            .inner()
            .indeterminate_phase();
        assert_eq!(
            root.get_widget(bar_id)
                .unwrap()
                .downcast::<ProgressBar>()
                .unwrap()
                .inner()
                .acked_indeterminate_phase(),
            Some(expected),
            "ack after present must commit host continuous phase"
        );
    }
}

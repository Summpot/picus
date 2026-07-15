//! Per-window frame scheduling: dirty reasons → decision table.
//!
//! [`FrameDriver`] is **internal** (not on the app facade). Applications still
//! only call `run_picus`. This module replaces the old boolean-OR paint path
//! with an explicit decision table so animation clock, rewrite/encode, and
//! present can be scheduled independently.
//!
//! Full-window encode remains acceptable in Phase 1; layered anim textures are
//! Phase 2. See `docs/plans/frame-pipeline.md` Phase 1.

use std::time::{Duration, Instant};

/// Why a window may need work this frame.
///
/// Populated from Masonry redraw/anim signals, resize metrics, surface retry,
/// first paint, and rewrite flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DirtyReason {
    FirstPaint,
    /// ECS rebuild / explicit `needs_redraw` (input-driven content change).
    InputOrRebuild,
    LayoutRewrite,
    ResizeMetrics,
    /// Pixel change for an anim layer (`layer == 0` means full-window today).
    AnimPaint {
        layer: u32,
    },
    /// Advance the anim clock only; pixels may be unchanged.
    AnimTick,
    /// Painter-order / clip / transform plan change (reserved; Phase 2).
    CompositorPlan,
    ThemeOrFont,
    RetrySurface,
}

impl DirtyReason {
    /// Hard rule (G5 / P1.4): these must **never** be skipped by any
    /// anim-only present throttle (including diagnostic `PICUS_ANIM_PRESENT_HZ`).
    #[inline]
    pub(crate) const fn is_unthrottled_present(self) -> bool {
        matches!(
            self,
            Self::FirstPaint | Self::InputOrRebuild | Self::ResizeMetrics | Self::RetrySurface
        )
    }
}

/// Aggregated dirty reasons for one frame decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DirtyBudget {
    reasons: Vec<DirtyReason>,
}

impl DirtyBudget {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(&mut self, reason: DirtyReason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.reasons.is_empty()
    }

    #[inline]
    pub(crate) fn has(&self, reason: DirtyReason) -> bool {
        self.reasons.contains(&reason)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = DirtyReason> + '_ {
        self.reasons.iter().copied()
    }

    /// True when any reason forces unthrottled present (G5).
    pub(crate) fn requires_unthrottled_present(&self) -> bool {
        self.reasons.iter().any(|r| r.is_unthrottled_present())
    }

    /// Anim clock should advance this frame.
    pub(crate) fn needs_anim_tick(&self) -> bool {
        self.reasons
            .iter()
            .any(|r| matches!(r, DirtyReason::AnimTick | DirtyReason::AnimPaint { .. }))
    }

    /// Decision-table: encode/present may be required this frame (anything
    /// except pure `AnimTick`, including `AnimPaint`).
    ///
    /// **Not** the same as Bevy wake flag [`RedrawDemand::need_content_present`]:
    /// that host flag is sticky/rewrite-based and stays false after a throttled
    /// AnimPaint encode-skip. This method feeds `FrameDriver::decide_present`
    /// only.
    pub(crate) fn needs_content_present(&self) -> bool {
        self.reasons
            .iter()
            .any(|r| !matches!(r, DirtyReason::AnimTick))
    }

    /// Pure anim present: only [`DirtyReason::AnimTick`] / [`DirtyReason::AnimPaint`].
    ///
    /// Eligible for selective anim encode (skip base rewrite/reassembly) when the
    /// window already has a stable ordered plan with anim entries (P2c / G2).
    pub(crate) fn is_selective_anim_encode(&self) -> bool {
        !self.is_empty()
            && self.reasons.iter().all(|r| {
                matches!(
                    r,
                    DirtyReason::AnimTick | DirtyReason::AnimPaint { .. }
                )
            })
            && self
                .reasons
                .iter()
                .any(|r| matches!(r, DirtyReason::AnimPaint { .. }))
    }
}

/// Separated work flags for one frame (P1.3 decision table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct FrameDecision {
    pub do_anim_tick: bool,
    pub do_rewrite: bool,
    pub do_encode: bool,
    pub do_present: bool,
    /// Entered scheduling but present deferred (idle anim clock or throttle).
    pub anim_tick_only: bool,
    /// Any work was considered (not a pure idle skip).
    pub enter_work: bool,
    /// Present was skipped solely by the optional anim present throttle.
    pub throttled_anim_present: bool,
}

/// Bevy wake demand split by timeline (Phase 1b redraw semantics).
///
/// Picus still wakes the Bevy event loop with a single
/// [`bevy_window::RequestRedraw`] when [`Self::any`] is true — Bevy reactive
/// mode has no public "run only Last/paint" path. These flags make **why** we
/// wake explicit and testable:
///
/// - [`Self::need_anim_tick`]: timeline B (advance anim clock; may skip encode)
/// - [`Self::need_content_present`]: Bevy content-wake demand from host stickies
///   / rewrite (resize/retry/input/theme). **Decision-table content ≠ this flag**:
///   [`DirtyBudget::needs_content_present`] is true for `AnimPaint` and drives
///   encode/present decisions; after a throttled AnimPaint skip the host wake
///   flag stays false so the next frame is not G5-promoted to `InputOrRebuild`.
///
/// # Tradeoff (P1b.2)
///
/// Anim-only demand still runs a full Bevy schedule (`PreUpdate`…`Last`). Avoiding
/// that empty system-table spin would require a custom winit integration or a
/// paint-only runner; out of scope for Phase 1b. Measurable win today is correct
/// **classification** (Failed does not force content loops; throttle stays anim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RedrawDemand {
    /// Need another Bevy frame for the anim clock (Masonry `needs_anim` / sticky).
    pub need_anim_tick: bool,
    /// Need another Bevy frame for content encode/present or unfulfilled content stickies.
    ///
    /// Host sticky/rewrite wake only — not [`DirtyBudget::needs_content_present`].
    pub need_content_present: bool,
}

impl RedrawDemand {
    #[inline]
    pub(crate) const fn none() -> Self {
        Self {
            need_anim_tick: false,
            need_content_present: false,
        }
    }

    #[inline]
    pub(crate) const fn anim_tick_only() -> Self {
        Self {
            need_anim_tick: true,
            need_content_present: false,
        }
    }

    #[inline]
    pub(crate) const fn content_present_only() -> Self {
        Self {
            need_anim_tick: false,
            need_content_present: true,
        }
    }

    #[inline]
    pub(crate) const fn both() -> Self {
        Self {
            need_anim_tick: true,
            need_content_present: true,
        }
    }

    /// True when Bevy should receive `RequestRedraw`.
    #[inline]
    pub(crate) const fn any(self) -> bool {
        self.need_anim_tick || self.need_content_present
    }

    /// Pure anim scheduling — no content present demand.
    #[inline]
    pub(crate) const fn is_anim_only(self) -> bool {
        self.need_anim_tick && !self.need_content_present
    }

    #[inline]
    pub(crate) fn merge(&mut self, other: Self) {
        self.need_anim_tick |= other.need_anim_tick;
        self.need_content_present |= other.need_content_present;
    }
}

/// Outcome of host frame execution after [`FrameDriver`] decisions.
///
/// Mirrors the former `PaintFrameResult` so `paint_masonry_ui` / perf wiring
/// stay stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameStepResult {
    pub painted: bool,
    /// Structured Bevy wake demand (Phase 1b); replaces a bare `wants_redraw` bool.
    pub redraw_demand: RedrawDemand,
    pub anim_tick_only: bool,
    pub paint_reasons: u32,
    pub phases: crate::perf::PaintPhaseTimings,
    pub decision: FrameDecision,
}

impl FrameStepResult {
    pub(crate) fn skipped() -> Self {
        Self {
            painted: false,
            redraw_demand: RedrawDemand::none(),
            anim_tick_only: false,
            paint_reasons: crate::perf::PaintReason::Skipped as u32,
            phases: crate::perf::PaintPhaseTimings::default(),
            decision: FrameDecision::default(),
        }
    }

    /// Convenience: any structured demand (writes `RequestRedraw` when true).
    #[inline]
    pub(crate) const fn wants_redraw(&self) -> bool {
        self.redraw_demand.any()
    }
}

/// ~30 Hz min interval (`from_millis(33)`) when a diagnostic cap is requested
/// via `PICUS_ANIM_PRESENT_HZ=30`. **Not** the product default — unset env means
/// no anim present throttle (see [`parse_anim_present_min_interval`]).
pub(crate) const DEFAULT_ANIM_PRESENT_MIN_INTERVAL: Duration = Duration::from_millis(33);

/// Parse `PICUS_ANIM_PRESENT_HZ` (or absence) into a min present interval.
///
/// # Product path (G10 / P2e)
///
/// Unset or empty → **no throttle** (`None`). Content / input / resize /
/// first-paint / retry redraws are **never** throttled regardless
/// ([`DirtyReason::is_unthrottled_present`]).
///
/// # Explicit diagnostic override
///
/// - unset / empty → unlimited product path (no anim present throttle)
/// - `0` / `off` / `none` / `false` → no throttle (same as unset)
/// - positive number → present at most that many **anim-driven** frames per second
/// - invalid value → warn and treat as no throttle (product default)
///
/// When a positive Hz is set, the throttle applies only to anim-driven presents
/// (existing G5 matrix: Resize / Input / FirstPaint / Retry never blocked).
pub(crate) fn parse_anim_present_min_interval(raw: Option<&str>) -> Option<Duration> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return None;
    };
    if raw == "0"
        || raw.eq_ignore_ascii_case("off")
        || raw.eq_ignore_ascii_case("none")
        || raw.eq_ignore_ascii_case("false")
    {
        return None;
    }
    match raw.parse::<f64>() {
        Ok(hz) if hz.is_finite() && hz > 0.0 => Some(Duration::from_secs_f64(1.0 / hz)),
        _ => {
            tracing::warn!(
                value = %raw,
                "invalid PICUS_ANIM_PRESENT_HZ; leaving anim present unthrottled"
            );
            None
        }
    }
}

/// Resolve the animation-only present throttle interval.
///
/// Returns `None` on the product path (env unset) and when the override disables
/// throttling (`PICUS_ANIM_PRESENT_HZ=0` / `off` / `none` / `false`).
pub(crate) fn anim_present_min_interval() -> Option<Duration> {
    use std::sync::OnceLock;
    static INTERVAL: OnceLock<Option<Duration>> = OnceLock::new();
    *INTERVAL.get_or_init(|| {
        parse_anim_present_min_interval(std::env::var("PICUS_ANIM_PRESENT_HZ").ok().as_deref())
    })
}

/// Per-window frame scheduler (decision + optional anim present throttle).
///
/// Execution (Masonry anim tick, Vello encode, surface present) stays on
/// [`super::WindowRuntime`]; this type owns **what** to do and throttle state.
#[derive(Debug, Default)]
pub(crate) struct FrameDriver {
    /// Last time an animation-driven frame was presented (spinner, etc.).
    last_anim_present: Option<Instant>,
}

impl FrameDriver {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Entry decision from pre-tick dirty signals.
    ///
    /// Separates `do_anim_tick` from encode/present (filled after tick via
    /// [`Self::decide_present`]). Host entry is [`super::WindowRuntime::step_frame`],
    /// not a `FrameDriver::step` method.
    ///
    /// `do_rewrite` here is advisory only: Phase 1 keeps rewrite+encode+present
    /// coupled on the content path. Pre-tick budgets never include `AnimPaint`
    /// (that reason is inserted post-tick).
    pub(crate) fn decide_entry(dirty: &DirtyBudget) -> FrameDecision {
        if dirty.is_empty() {
            return FrameDecision::default();
        }
        FrameDecision {
            do_anim_tick: dirty.needs_anim_tick(),
            // Advisory: host still couples rewrite to encode in Phase 1.
            do_rewrite: dirty.has(DirtyReason::LayoutRewrite)
                || dirty.requires_unthrottled_present()
                || dirty.has(DirtyReason::ThemeOrFont)
                || dirty.has(DirtyReason::CompositorPlan)
                || dirty.has(DirtyReason::InputOrRebuild),
            do_encode: false,
            do_present: false,
            anim_tick_only: false,
            enter_work: true,
            throttled_anim_present: false,
        }
    }

    /// Present/encode decision after anim tick and post-tick dirty refresh.
    ///
    /// # Hard rule (P1.4 / G5)
    ///
    /// `ResizeMetrics`, `InputOrRebuild`, `FirstPaint`, and `RetrySurface` are
    /// never blocked by the anim present throttle.
    ///
    /// When `min_interval` is `Some`, the optional throttle only applies to
    /// **anim-driven** content ([`DirtyReason::AnimPaint`] / anim clock +
    /// rewrite without unthrottled reasons). Pure layout/theme/compositor dirt
    /// is not delayed by the anim interval. Product path passes `None`.
    pub(crate) fn decide_present(
        &self,
        dirty: &DirtyBudget,
        min_interval: Option<Duration>,
        now: Instant,
    ) -> FrameDecision {
        let mut decision = FrameDecision {
            enter_work: true,
            do_anim_tick: dirty.needs_anim_tick(),
            ..FrameDecision::default()
        };

        if !dirty.needs_content_present() {
            // Pure AnimTick — advance clock only; no encode/present.
            decision.anim_tick_only = true;
            return decision;
        }

        decision.do_rewrite = true;
        decision.do_encode = true;
        decision.do_present = true;

        if Self::should_apply_anim_present_throttle(dirty)
            && let Some(interval) = min_interval
        {
            let within = self
                .last_anim_present
                .is_some_and(|last| now.duration_since(last) < interval);
            if within {
                decision.do_rewrite = false;
                decision.do_encode = false;
                decision.do_present = false;
                decision.anim_tick_only = true;
                decision.throttled_anim_present = true;
                return decision;
            }
        }

        decision
    }

    /// Whether the optional pure-anim present throttle may skip this frame.
    ///
    /// Matches Phase 0 `anim_driven_present = should_tick && !incoming_redraw &&
    /// !first_paint`: any non-G5 content present that co-occurs with the anim
    /// clock (including **LayoutRewrite + AnimTick** without Input/Resize/etc.)
    /// may be delayed when a diagnostic interval is set. Pure `LayoutRewrite`
    /// without anim is never throttled. G5 reasons and ThemeOrFont /
    /// CompositorPlan always present immediately.
    fn should_apply_anim_present_throttle(dirty: &DirtyBudget) -> bool {
        if dirty.requires_unthrottled_present() {
            return false;
        }
        // Structural / theme changes always present even if the anim clock runs.
        if dirty.has(DirtyReason::ThemeOrFont) || dirty.has(DirtyReason::CompositorPlan) {
            return false;
        }
        // AnimPaint alone, or any content present while the anim clock is active
        // (e.g. LayoutRewrite + AnimTick after spinner rewrite).
        dirty
            .iter()
            .any(|r| matches!(r, DirtyReason::AnimPaint { .. }))
            || (dirty.needs_anim_tick() && dirty.needs_content_present())
    }

    /// Record that an animation-driven present was submitted (for throttle).
    pub(crate) fn note_anim_present(&mut self, now: Instant) {
        self.last_anim_present = Some(now);
    }

    /// Map dirty reasons into legacy [`crate::perf::PaintReason`] bits.
    pub(crate) fn paint_reasons_mask(dirty: &DirtyBudget) -> u32 {
        let mut mask = 0u32;
        for reason in dirty.iter() {
            mask |= match reason {
                DirtyReason::FirstPaint => crate::perf::PaintReason::FirstPaint as u32,
                DirtyReason::InputOrRebuild
                | DirtyReason::ResizeMetrics
                | DirtyReason::RetrySurface
                | DirtyReason::ThemeOrFont
                | DirtyReason::CompositorPlan
                | DirtyReason::AnimPaint { .. } => crate::perf::PaintReason::NeedsRedraw as u32,
                DirtyReason::LayoutRewrite => crate::perf::PaintReason::NeedsRewritePasses as u32,
                DirtyReason::AnimTick => crate::perf::PaintReason::NeedsAnimFrame as u32,
            };
        }
        mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval_33ms() -> Duration {
        Duration::from_millis(33)
    }

    #[test]
    fn anim_tick_only_may_skip_present() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        let driver = FrameDriver::new();
        let now = Instant::now();
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), now);
        assert!(decision.anim_tick_only);
        assert!(!decision.do_encode);
        assert!(!decision.do_present);
        assert!(!decision.throttled_anim_present);
    }

    #[test]
    fn resize_must_attempt_present_not_blocked_by_throttle() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        dirty.insert(DirtyReason::AnimPaint { layer: 0 });
        dirty.insert(DirtyReason::ResizeMetrics);
        let mut driver = FrameDriver::new();
        // Simulate a very recent anim present — throttle would fire without resize.
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
        assert!(
            decision.do_present && decision.do_encode,
            "ResizeMetrics must not be blocked by anim throttle: {decision:?}"
        );
        assert!(!decision.throttled_anim_present);
        assert!(!decision.anim_tick_only);
    }

    #[test]
    fn first_paint_input_retry_also_unthrottled() {
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        for reason in [
            DirtyReason::FirstPaint,
            DirtyReason::InputOrRebuild,
            DirtyReason::RetrySurface,
        ] {
            let mut dirty = DirtyBudget::new();
            dirty.insert(DirtyReason::AnimTick);
            dirty.insert(reason);
            let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
            assert!(
                decision.do_present,
                "{reason:?} must attempt present under throttle pressure"
            );
        }
    }

    #[test]
    fn anim_paint_alone_may_be_throttled() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        dirty.insert(DirtyReason::AnimPaint { layer: 0 });
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
        assert!(decision.throttled_anim_present);
        assert!(!decision.do_present);
        assert!(decision.anim_tick_only);
    }

    #[test]
    fn selective_anim_encode_budget_detects_pure_anim_paint() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        dirty.insert(DirtyReason::AnimPaint { layer: 1 });
        assert!(dirty.is_selective_anim_encode());

        dirty.insert(DirtyReason::InputOrRebuild);
        assert!(!dirty.is_selective_anim_encode());

        let mut tick_only = DirtyBudget::new();
        tick_only.insert(DirtyReason::AnimTick);
        assert!(
            !tick_only.is_selective_anim_encode(),
            "AnimTick alone is not content present"
        );
    }

    #[test]
    fn base_invalidation_via_input_or_rebuild_is_unthrottled() {
        // Issue 10: rewrite-completed sticky is promoted to InputOrRebuild so
        // anim throttle cannot drop the frame that must reassemble base.
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        dirty.insert(DirtyReason::AnimPaint { layer: 0 });
        dirty.insert(DirtyReason::LayoutRewrite);
        dirty.insert(DirtyReason::InputOrRebuild);
        assert!(dirty.requires_unthrottled_present());
        assert!(!dirty.is_selective_anim_encode());

        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
        assert!(
            decision.do_present,
            "base invalidation must present under anim throttle pressure"
        );
        assert!(!decision.throttled_anim_present);
    }

    #[test]
    fn anim_paint_allowed_after_interval() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimPaint { layer: 0 });
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let later = t0 + interval_33ms() + Duration::from_millis(1);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), later);
        assert!(decision.do_present);
        assert!(!decision.throttled_anim_present);
    }

    #[test]
    fn throttle_disabled_never_skips_anim_paint() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimPaint { layer: 0 });
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, None, t0);
        assert!(decision.do_present);
    }

    #[test]
    fn empty_dirty_does_not_enter() {
        let dirty = DirtyBudget::new();
        let entry = FrameDriver::decide_entry(&dirty);
        assert!(!entry.enter_work);
        assert!(!entry.do_anim_tick);
    }

    #[test]
    fn decide_entry_requests_anim_tick() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        let entry = FrameDriver::decide_entry(&dirty);
        assert!(entry.enter_work);
        assert!(entry.do_anim_tick);
    }

    #[test]
    fn anim_present_hz_unset_is_unlimited() {
        // G10: product path has no transitional throttle when env is unset.
        assert_eq!(parse_anim_present_min_interval(None), None);
        assert_eq!(parse_anim_present_min_interval(Some("")), None);
        assert_eq!(parse_anim_present_min_interval(Some("   ")), None);
    }

    #[test]
    fn anim_present_hz_zero_disables_throttle() {
        assert_eq!(parse_anim_present_min_interval(Some("0")), None);
        assert_eq!(parse_anim_present_min_interval(Some("off")), None);
        assert_eq!(parse_anim_present_min_interval(Some("NONE")), None);
        assert_eq!(parse_anim_present_min_interval(Some("false")), None);
    }

    #[test]
    fn anim_present_hz_positive_sets_interval() {
        let interval = parse_anim_present_min_interval(Some("60")).expect("60 Hz enabled");
        assert!((interval.as_secs_f64() - (1.0 / 60.0)).abs() < 1e-9);
        let interval = parse_anim_present_min_interval(Some("10")).expect("10 Hz enabled");
        assert!((interval.as_secs_f64() - 0.1).abs() < 1e-9);
        // Diagnostic ~30 Hz opt-in (named constant is the historical 33 ms approx).
        let interval = parse_anim_present_min_interval(Some("30")).expect("30 Hz enabled");
        assert!((interval.as_secs_f64() - (1.0 / 30.0)).abs() < 1e-9);
        assert!((interval.as_secs_f64() - DEFAULT_ANIM_PRESENT_MIN_INTERVAL.as_secs_f64()).abs() < 0.002);
    }

    #[test]
    fn anim_present_hz_invalid_falls_back_to_unlimited() {
        assert_eq!(parse_anim_present_min_interval(Some("not-a-number")), None);
        assert_eq!(parse_anim_present_min_interval(Some("-5")), None);
    }

    #[test]
    fn retry_reason_is_unthrottled() {
        assert!(DirtyReason::RetrySurface.is_unthrottled_present());
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::RetrySurface);
        assert!(dirty.requires_unthrottled_present());
        assert!(dirty.needs_content_present());
    }

    #[test]
    fn pure_layout_rewrite_not_anim_throttled() {
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::LayoutRewrite);
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
        assert!(
            decision.do_present,
            "LayoutRewrite without anim path must present: {decision:?}"
        );
    }

    #[test]
    fn layout_rewrite_plus_anim_tick_may_be_throttled() {
        // When a diagnostic interval is set: non-G5 content co-occurring with
        // the anim clock is treated as anim-driven present and may skip under
        // the interval (not a G5 violation). Product path passes None.
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::LayoutRewrite);
        dirty.insert(DirtyReason::AnimTick);
        let mut driver = FrameDriver::new();
        let t0 = Instant::now();
        driver.note_anim_present(t0);
        let decision = driver.decide_present(&dirty, Some(interval_33ms()), t0);
        assert!(
            decision.throttled_anim_present && !decision.do_present,
            "LayoutRewrite+AnimTick under throttle pressure may skip: {decision:?}"
        );
        // Unset / unlimited product path never throttles the same dirty set.
        let unlimited = driver.decide_present(&dirty, None, t0);
        assert!(
            unlimited.do_present && !unlimited.throttled_anim_present,
            "product path (None interval) must present: {unlimited:?}"
        );
    }

    #[test]
    fn redraw_demand_any_and_merge() {
        assert!(!RedrawDemand::none().any());
        assert!(RedrawDemand::anim_tick_only().any());
        assert!(RedrawDemand::anim_tick_only().is_anim_only());
        assert!(!RedrawDemand::content_present_only().is_anim_only());
        assert!(!RedrawDemand::both().is_anim_only());

        let mut d = RedrawDemand::anim_tick_only();
        d.merge(RedrawDemand::content_present_only());
        assert_eq!(d, RedrawDemand::both());
    }

    #[test]
    fn multi_window_redraw_demand_merge_matrix() {
        // Mirrors `paint_masonry_ui` OR-merge of per-window FrameStepResult demands
        // before a single process-wide RequestRedraw.
        fn merge_windows(a: RedrawDemand, b: RedrawDemand) -> RedrawDemand {
            let mut d = RedrawDemand::none();
            d.merge(a);
            d.merge(b);
            d
        }

        // Anim-only ∨ content → both flags; still one RequestRedraw via any().
        let merged = merge_windows(
            RedrawDemand::anim_tick_only(),
            RedrawDemand::content_present_only(),
        );
        assert_eq!(merged, RedrawDemand::both());
        assert!(merged.any());

        // Failed-none ∨ content → content only (Failed must not clear content wake).
        let merged = merge_windows(RedrawDemand::none(), RedrawDemand::content_present_only());
        assert_eq!(merged, RedrawDemand::content_present_only());
        assert!(merged.need_content_present && !merged.need_anim_tick);

        // Anim-only ∨ Failed-none → anim-only (idle window does not clear demand).
        let merged = merge_windows(RedrawDemand::anim_tick_only(), RedrawDemand::none());
        assert_eq!(merged, RedrawDemand::anim_tick_only());
        assert!(merged.is_anim_only());

        // none ∨ none → no wake.
        let merged = merge_windows(RedrawDemand::none(), RedrawDemand::none());
        assert_eq!(merged, RedrawDemand::none());
        assert!(!merged.any());

        // Failed anim-only ∨ content (typical multi-window matrix from host paths).
        let failed_anim = RedrawDemand::anim_tick_only(); // finish_present Failed + live clock
        let content = RedrawDemand::content_present_only();
        let merged = merge_windows(failed_anim, content);
        assert_eq!(merged, RedrawDemand::both());
    }

    #[test]
    fn pure_anim_tick_dirty_needs_anim_not_content() {
        // DirtyBudget classification that feeds decide_present — decision-table
        // content (includes AnimPaint) is not the Bevy wake content flag.
        let mut dirty = DirtyBudget::new();
        dirty.insert(DirtyReason::AnimTick);
        assert!(dirty.needs_anim_tick());
        assert!(!dirty.needs_content_present());
    }
}

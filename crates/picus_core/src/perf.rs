//! Lightweight frame-phase timing for diagnosing CPU-bound UI frames.
//!
//! Enable with environment variable `PICUS_FRAME_TIMING=1` (or `true` / `yes`).
//! When enabled, Picus records durations for synthesis, retained rebuild, and
//! paint (split into rewrite/redraw vs imaging present) and logs a summary
//! about once per second at `info` level:
//!
//! ```text
//! picus frame timing: frames=60 synth=0.12ms rebuild=0.40ms paint=14.80ms \
//!   (redraw=6.10 present=8.70) painted=60/60 synth_nodes=avg 820 reasons=...
//! ```
//!
//! Spans are always available via `tracing` (`picus_core::perf` target) so
//! `RUST_LOG=picus_core::perf=trace` works without the env flag.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use bevy_ecs::prelude::Resource;

/// Whether process-level frame timing aggregation is enabled.
pub fn frame_timing_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("PICUS_FRAME_TIMING")
            .map(|value| {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("yes")
                    || value.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
    })
}

/// Aggregated phase timings for recent frames.
#[derive(Resource, Debug, Default)]
pub struct FrameTiming {
    window_started: Option<Instant>,
    frames: u32,
    painted_frames: u32,
    synth_dirty_frames: u32,
    synth_ns: u128,
    rebuild_ns: u128,
    paint_ns: u128,
    paint_redraw_ns: u128,
    paint_present_ns: u128,
    synth_nodes_sum: u64,
    /// Bitmask of paint reasons observed this window (see [`PaintReason`]).
    paint_reasons: u32,
    /// Compact labels of dirty synthesis reasons seen this window.
    synth_reason_labels: Vec<&'static str>,
}

/// Why a paint pass ran (for idle continuous-redraw diagnosis).
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum PaintReason {
    FirstPaint = 1 << 0,
    NeedsRedraw = 1 << 1,
    NeedsAnimFrame = 1 << 2,
    RenderRootNeedsAnim = 1 << 3,
    NeedsRewritePasses = 1 << 4,
    Skipped = 1 << 5,
    /// Animation ticked but no widget requested a pixel update (no present).
    AnimTickNoPresent = 1 << 6,
}

impl FrameTiming {
    pub fn begin_frame(&mut self) {
        if !frame_timing_enabled() {
            return;
        }
        if self.window_started.is_none() {
            self.window_started = Some(Instant::now());
        }
    }

    pub fn record_synthesis(
        &mut self,
        duration: Duration,
        dirty: bool,
        node_count: usize,
        reason_labels: &[&'static str],
    ) {
        if !frame_timing_enabled() {
            return;
        }
        self.synth_ns += duration.as_nanos();
        if dirty {
            self.synth_dirty_frames += 1;
            self.synth_nodes_sum += node_count as u64;
            for label in reason_labels {
                if !self.synth_reason_labels.contains(label) {
                    self.synth_reason_labels.push(*label);
                }
            }
        }
    }

    pub fn record_rebuild(&mut self, duration: Duration) {
        if !frame_timing_enabled() {
            return;
        }
        self.rebuild_ns += duration.as_nanos();
    }

    pub fn record_paint(
        &mut self,
        total: Duration,
        redraw: Duration,
        present: Duration,
        painted: bool,
        reasons: u32,
    ) {
        if !frame_timing_enabled() {
            return;
        }
        self.frames += 1;
        self.paint_ns += total.as_nanos();
        self.paint_redraw_ns += redraw.as_nanos();
        self.paint_present_ns += present.as_nanos();
        if painted {
            self.painted_frames += 1;
        }
        self.paint_reasons |= reasons;

        let Some(started) = self.window_started else {
            return;
        };
        if started.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.flush_summary();
    }

    fn flush_summary(&mut self) {
        let frames = self.frames.max(1) as f64;
        let synth_ms = (self.synth_ns as f64 / frames) / 1_000_000.0;
        let rebuild_ms = (self.rebuild_ns as f64 / frames) / 1_000_000.0;
        let paint_ms = (self.paint_ns as f64 / frames) / 1_000_000.0;
        let redraw_ms = (self.paint_redraw_ns as f64 / frames) / 1_000_000.0;
        let present_ms = (self.paint_present_ns as f64 / frames) / 1_000_000.0;
        let avg_nodes = if self.synth_dirty_frames == 0 {
            0.0
        } else {
            self.synth_nodes_sum as f64 / f64::from(self.synth_dirty_frames)
        };
        let reasons = format_paint_reasons(self.paint_reasons);
        let synth_reasons = if self.synth_reason_labels.is_empty() {
            "none".to_string()
        } else {
            self.synth_reason_labels.join("|")
        };

        tracing::info!(
            target: "picus_core::perf",
            frames = self.frames,
            painted = self.painted_frames,
            synth_dirty = self.synth_dirty_frames,
            synth_ms = format_args!("{synth_ms:.3}"),
            rebuild_ms = format_args!("{rebuild_ms:.3}"),
            paint_ms = format_args!("{paint_ms:.3}"),
            redraw_ms = format_args!("{redraw_ms:.3}"),
            present_ms = format_args!("{present_ms:.3}"),
            avg_synth_nodes = format_args!("{avg_nodes:.0}"),
            paint_reasons = %reasons,
            synth_reasons = %synth_reasons,
            "picus frame timing"
        );

        *self = Self {
            window_started: Some(Instant::now()),
            ..Self::default()
        };
    }
}

fn format_paint_reasons(mask: u32) -> String {
    let mut parts = Vec::new();
    if mask & PaintReason::FirstPaint as u32 != 0 {
        parts.push("first");
    }
    if mask & PaintReason::NeedsRedraw as u32 != 0 {
        parts.push("redraw");
    }
    if mask & PaintReason::NeedsAnimFrame as u32 != 0 {
        parts.push("anim_frame");
    }
    if mask & PaintReason::RenderRootNeedsAnim as u32 != 0 {
        parts.push("needs_anim");
    }
    if mask & PaintReason::NeedsRewritePasses as u32 != 0 {
        parts.push("rewrite");
    }
    if mask & PaintReason::Skipped as u32 != 0 {
        parts.push("skipped");
    }
    if mask & PaintReason::AnimTickNoPresent as u32 != 0 {
        parts.push("anim_no_present");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join("|")
    }
}

/// RAII timer that records elapsed nanos into a callback.
pub struct PhaseTimer {
    start: Instant,
}

impl PhaseTimer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

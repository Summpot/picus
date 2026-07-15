# Frame pipeline baseline record

> **Status**: protocol + template (Phase 0)  
> **Related plan**: [plans/frame-pipeline.md](../plans/frame-pipeline.md)  
> **Runtime narrative**: [architecture/runtime.md](../architecture/runtime.md)

This file is the **versioned record** for frame-pipeline performance baselines.
PR descriptions may link here; they must not be the only place numbers live.

CPU-side `PICUS_FRAME_TIMING` phases measure submit-path wall time only. They are
**not** displayed-frame latency. On Windows, **PresentMon/ETW is required** for
any acceptance claim that involves drag ghosting or displayed-frame latency
(G3/G4 and later).

---

## 1. Environment template

Fill one block per machine / OS build used for a baseline run.

| Field | Value |
|-------|--------|
| Date (UTC) | _TBD_ |
| Operator | _TBD_ |
| Host OS | Windows _version_ / build |
| CPU | _model_ |
| GPU | _model_ + driver version |
| Display | _Hz_, HDR on/off, G-Sync/FreeSync |
| Scale factor | e.g. 100% / 150% |
| Picus commit | _sha_ |
| Profile | `debug` / `release` |
| Present mode observed | Mailbox / FifoRelaxed / Fifo / other (from runtime logs) |
| `PICUS_FRAME_TIMING` | `1` |
| `PICUS_ANIM_PRESENT_HZ` | unset (default ~30) / `0` · `off` · `none` · `false` (disable) / positive Hz |
| PresentMon version | _required_ |
| Notes | power plan, background load, multi-monitor, etc. |

---

## 2. Repeatable protocol

### 2.1 Resolutions (fixed)

Run the full scenario matrix at **both**:

1. **1920×1080** client area (logical; match scale factor so physical is known)
2. **3840×2160** client area

Record both logical size and physical size (scale factor).

### 2.2 Scenarios

| ID | Scenario | Interaction |
|----|----------|-------------|
| S1 | Gallery **Button** idle | No pointer motion after warm-up; no OS/surface invalidation expected |
| S2 | Gallery **Spinner** page, window still | Spinner (or indeterminate ProgressBar) animating; window not moved |
| S3 | Gallery **Spinner** page, **window drag** | Fixed drag trajectory (below) while spinner runs |

Use the same gallery navigation path each run (document the clicks).

### 2.3 Fixed window-drag trajectory (S3)

Use one of:

- **Manual scripted**: start at a known screen position; drag roughly along a
  horizontal figure-eight or left→right→left path spanning ≥ half the work area
  for the full sample window; same path every run; or
- **Automation** (preferred when available): same pixel path replayed by a tool.

Document which method was used. Do **not** change path mid-matrix.

### 2.4 Timing

| Phase | Duration | Action |
|-------|----------|--------|
| Warm-up | **10 s** | Reach steady state; do **not** include in stats |
| Sample | **30 s** | Capture PresentMon/ETW + optional `PICUS_FRAME_TIMING` log |
| Repeats | **×3 debug** and **×3 release** per resolution × scenario | Report median of run medians where noted |

### 2.5 PresentMon / ETW (required on Windows)

PresentMon (or equivalent ETW capture that yields per-presented-frame times) is
**mandatory** for Windows baselines. Optional tools elsewhere do not replace it
for G3/G4 claims.

Suggested capture:

1. Start PresentMon (or GPUView/WPA ETL) against the Picus example process.
2. Begin warm-up, then mark sample start/stop (or trim CSV to the 30 s window).
3. Archive raw CSV/ETL under a local or CI artifact path; paste **summaries** into
   §3 below (do not rely on PR body alone).

Minimum fields to extract:

- Presented frame count / rate
- Display latency or MsBetweenDisplayChange (tool-dependent name)
- Dropped / late frames if available
- Group results by actual present mode when known

### 2.6 CPU timing companion (optional but recommended)

```text
set PICUS_FRAME_TIMING=1
set RUST_LOG=picus_core::perf=info
# Default product path (transitional ~30 Hz anim-only present):
#   leave PICUS_ANIM_PRESENT_HZ unset
# Unthrottled anim present (baseline / debug only):
#   set PICUS_ANIM_PRESENT_HZ=0
#   (also accepted: off / none / false)
cargo run -p gallery --release
```

Record 1 Hz `picus frame timing` averages for the sample window. Remember:
`present_submit_ms` ≠ display time.

**How to read CPU averages:**

| Field group | Denominator |
|-------------|-------------|
| `anim_tick_ms` | All entered-work paint attempts for that window (`frames`) |
| `scene_build_*`, `surface_acquire`, `encode_*`, `composite`, `present_submit` | **Content paint attempts only** (`frames − anim_tick_only`, logged as `content_paint_frames`) so throttled anim-only zeros do not dilute encode/present |
| Process `input_dispatch_ms` / `synth_ms` / `rebuild_ms` | `bevy_frames` (not multi-window paint attempts) |
| Process `paint_ms` / `redraw_ms` / `present_ms` | **Content paint attempts** (`frames − anim_tick_only`, logged as process `content_paint_frames`); if that is 0, `present_ms=0` and paint/redraw fall back to all entered-work attempts |
| Process `frames` | Sum of per-window paint attempts (can be ≈ windows × Bevy paints) |

Also: `anim_tick_ms` currently includes rewrite inside `AnimFrame`; `scene_build_base_ms` is root `redraw()` only (see `runtime.md` Phase 0 honesty notes).

---

## 3. Result tables (fill per campaign)

### 3.1 CSV / ETL summary placeholders

| Run | Profile | Res | Scenario | PresentMon CSV / ETL path | Notes |
|-----|---------|-----|----------|---------------------------|-------|
| 1 | debug | 1080p | S1 | _path_ | |
| 2 | debug | 1080p | S2 | _path_ | |
| 3 | debug | 1080p | S3 | _path_ | |
| … | release | 4K | S3 | _path_ | |

### 3.2 Display-path latency (PresentMon/ETW)

Report **median / p95 / p99** of the chosen latency metric (name the column).

| Profile | Res | Scenario | Run medians (3) | Median-of-medians | p95 | p99 | Unit |
|---------|-----|----------|-----------------|-------------------|-----|-----|------|
| debug | 1080p | S3 | _ · _ · _ | _ | _ | _ | ms |
| release | 1080p | S3 | _ · _ · _ | _ | _ | _ | ms |
| debug | 4K | S3 | _ · _ · _ | _ | _ | _ | ms |
| release | 4K | S3 | _ · _ · _ | _ | _ | _ | ms |

### 3.3 Present counts

| Profile | Res | Scenario | presents / 30 s (3 runs) | Notes |
|---------|-----|----------|--------------------------|-------|
| release | 1080p | S1 Button idle | _ | Expect ~0 without OS invalidation (G6 target) |
| release | 1080p | S2 Spinner still | _ | With default throttle ≈ 30 Hz class |
| release | 1080p | S2 + `PICUS_ANIM_PRESENT_HZ=0` | _ | Unthrottled comparison only |

### 3.4 CPU phase averages (`PICUS_FRAME_TIMING`, ms)

Copy values from the per-window `picus frame timing` line. Present-path columns
are **content-paint means** (see §2.6), not diluted by `anim_tick_only` samples.
Record both `presented` and `anim_tick_only` counters for the sample window.

| Profile | Res | Scenario | anim_tick | scene_build_base | encode_base | composite | present_submit | presented | anim_tick_only | content_paint_frames |
|---------|-----|----------|-----------|------------------|-------------|---------|----------------|-----------|----------------|----------------------|
| release | 1080p | S2 | _ | _ | _ | _ | _ | _ | _ | _ |
| release | 1080p | S3 | _ | _ | _ | _ | _ | _ | _ | _ |

---

## 4. Acceptance thresholds (campaign targets)

Phase 0 only requires the **skeleton and protocol** (G1). Numeric gates for later
phases (from the plan; refine when first real numbers exist):

| Gate | Intent | Threshold (plan) | Status |
|------|--------|------------------|--------|
| G1 | Named phases + per-window `frame_id` | Metrics log + this protocol | Skeleton in tree |
| G3 | Spinner still: design phases visible; indeterminate bar ≈ `0.9 × min(60, refresh_hz)` without permanent global throttle | PresentMon + content version | Pending architecture |
| G4 | Spinner drag: displayed-frame latency p95 ≤ 2 refresh periods; ≥30% better than P0 baseline; default path not permanent fps cut | PresentMon ×3 debug/release | Needs filled §3 as P0 baseline |
| G6 | Button idle present count = 0 in 30 s sample | Counter | Pending |
| G10 | Remove default 30 Hz anim throttle | Code review after G2–G4 | Blocked on P2e |

**P0 baseline freeze**: once §3 is first filled for a named commit, later PRs
compare against that row set (or a clearly marked newer baseline revision).

---

## 5. Revision log

| Date | Commit | Change |
|------|--------|--------|
| 2026-07-16 | Phase 0 PR | Created protocol + empty result tables |
| 2026-07-16 | Phase 0 review fixes | Document present-path vs anim_tick denominators; full `PICUS_ANIM_PRESENT_HZ` disable set |
| 2026-07-16 | Phase 2a layer gate | §6 anim target strategy + size-gate assumptions (no new PresentMon numbers yet) |
| 2026-07-16 | Phase 2a review fixes | §6.3 explicit: assumptions ≠ G2 acceptance; G2 is P2b |

---

## 6. Anim target strategy (Phase 2a gate → P2b)

Gate implementation: `picus_core::runtime::layers` (`AnimTargetStrategy`). Narrative:
[architecture/runtime.md](../architecture/runtime.md) (Masonry layer contract).

### 6.1 Choice

| Field | Value |
|-------|--------|
| **Selected for first composite (P2b)** | **`FullWindowTransparent`** — full-window transparent anim render target; only anim widgets paint into it |
| Deferred | `WidgetBoundsAtlas` — tight bounds / atlas (Phase 4 or if §6.2 fails) |
| Boundary path | Picus `AnimLayerHost` + Masonry `PaintLayerMode::External` slots (not upstream-only isolation) |

Plan §2.0 already recommended full-window transparent + “only draw anim widgets” for
the first vertical slice. The Phase 2a inventory confirmed Masonry cannot yet supply
self-contained selective layer redraw, so Picus owns anim dirty state regardless of
target geometry.

### 6.2 Size / budget criteria (plan gates)

| Criterion | Intent | How we will judge |
|-----------|--------|-------------------|
| **encode_anim + composite p95** | ≤ **25%** of refresh period (e.g. ≤ 4.17 ms @ 60 Hz) | `PICUS_FRAME_TIMING` `encode_anim_ms` + `composite_ms` content-paint means once P2b wires counters; PresentMon for display path |
| **4K G3** | Spinner design phases still visible at 3840×2160 | §2 protocol S2 @ 4K after P2b |
| **Drag G4** | Displayed-frame latency p95 ≤ 2 refresh periods; ≥30% better than P0 | §3 PresentMon S3 |
| **Multi-entry** | Multiple anim entries must **not** force full-window **base** clear each tick | Host dirty set encodes only dirty anim entries; base stays cached |

### 6.3 Assumptions (until measured)

These are **planning assumptions**, **not** acceptance claims and **not** “G2
satisfied” today. G2 (`encode_base` → 0 on pure anim; host-only anim encode) is a
**P2b** acceptance target. Fill §3 when P2b exists and re-state claims only with
measured counters / PresentMon.

1. **Full-window transparent anim RT cost** is dominated by Vello encode of a *sparse*
   anim scene plus a full-size clear/composite, not by re-recording the entire base
   UI tree (**target:** base encode_count → 0 on pure anim ticks — G2 after P2b).
2. At **1080p**, full-window anim encode is expected to clear the 25% refresh budget
   for a single Spinner-class scene; at **4K** the clear+composite term grows with
   pixel count and is the first risk for switching to atlas.
3. **WidgetBoundsAtlas** reduces pixel work but requires reliable bounds under
   scroll/clip/transform and more composite rect bookkeeping; deferred until
   FullWindowTransparent fails §6.2 on real hardware.
4. **Mailbox** present remains required for G4; target strategy does not replace
   present-mode policy.
5. Transitional **~30 Hz anim present throttle** stays default until G2–G4 pass
   after host+composite (G10).

### 6.4 Comparison snapshot (qualitative)

| Dimension | FullWindowTransparent | WidgetBoundsAtlas |
|-----------|----------------------|-------------------|
| Implementation risk | Lower (matches current full-window paths) | Higher (scissor/atlas, dirty union) |
| encode_anim vs resolution | Scales with window pixels | Scales with widget dirty union |
| Clip/scroll correctness | Composite in window space; External slot bounds from layout | Must clip atlas samples to ancestor clip |
| Multi Spinner | One shared anim RT or N full-window RTs (product choice in P2b) | Natural atlas packing |
| When to prefer | Default first ship | 4K / many entries if p95 over budget |

**Decision:** ship P2b with **FullWindowTransparent**; re-evaluate atlas only if
encode_anim+composite p95 exceeds 25% refresh on the §2 matrix (especially 4K S2/S3).

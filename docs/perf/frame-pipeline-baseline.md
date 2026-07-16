# Frame pipeline baseline record

> **Status**: protocol + result template (architecture P0–P3 done; PresentMon tables may still be empty placeholders)  
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
| `PICUS_ANIM_PRESENT_HZ` | unset = unlimited product path / `0` · `off` · `none` · `false` (no throttle) / positive Hz (diagnostic cap) |
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
# Default product path (G10): no anim present throttle:
#   leave PICUS_ANIM_PRESENT_HZ unset
# Explicit no-throttle tokens (same as unset):
#   set PICUS_ANIM_PRESENT_HZ=0
#   (also accepted: off / none / false)
# Diagnostic cap only (opt-in; anim-driven presents only, G5 still unthrottled):
#   set PICUS_ANIM_PRESENT_HZ=30
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
| release | 1080p | S2 Spinner still | _ | Product path (unset) — unlimited; fill PresentMon when run |
| release | 1080p | S2 + `PICUS_ANIM_PRESENT_HZ=30` | _ | Diagnostic cap comparison only |

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

Protocol and G1 metrics skeleton are in tree. Spinner product-path G2, unit G2,
and G10 are architecture-done.
Numeric display-path gates (G3/G4) still need PresentMon fills — refine thresholds
when first real numbers exist:

| Gate | Intent | Threshold (plan) | Status |
|------|--------|------------------|--------|
| G1 | Named phases + per-window `frame_id` | Metrics log + this protocol | **Done** (skeleton + protocol in tree) |
| G2 | Pure Spinner / indeterminate bar: `encode_base` ≈ 0; anim host only | Unit contracts + timing | **Done** (Spinner gallery product timing + unit G2; not PresentMon) |
| G3 | Spinner still: design phases visible; indeterminate bar ≈ `0.9 × min(60, refresh_hz)` without permanent global throttle | PresentMon + content version | Architecture done; **numbers placeholder** until measured |
| G4 | Spinner drag: displayed-frame latency p95 ≤ 2 refresh periods; ≥30% better than P0 baseline; default path not permanent fps cut | PresentMon ×3 debug/release | Architecture done; **§3 tables still empty** — do not invent numbers |
| G6 | Button idle present count = 0 in 30 s sample | Counter | Pending measurement |
| G10 | Remove default 30 Hz anim throttle | Code review | **Done (P2e):** unset = unlimited; override opt-in |

**P0 baseline freeze**: once §3 is first filled for a named commit, later PRs
compare against that row set (or a clearly marked newer baseline revision).

---

## 5. Revision log

| Date | Commit | Change |
|------|--------|--------|
| 2026-07-16 | Phase 0 PR | Created protocol + empty result tables |
| 2026-07-16 | Phase 0 review fixes | Document present-path vs anim_tick denominators; full `PICUS_ANIM_PRESENT_HZ` disable set |
| 2026-07-16 | Phase 2a layer gate | §6 anim target strategy + size-gate assumptions (no new PresentMon numbers yet) |
| 2026-07-16 | Phase 2a review fixes | §6.3 explicit: size-budget assumptions ≠ display-path acceptance |
| 2026-07-16 | Phase 2e / G10 | Default anim present throttle removed; `PICUS_ANIM_PRESENT_HZ` diagnostic opt-in only. Spinner + ProgressBar G2 unit contracts + PresentPolicy FIFO/Mailbox tests exist; PresentMon G3/G4 numbers remain placeholders |
| 2026-07-16 | Phase 6 docs | Status + gate table honesty (G2 unit done; G3/G4 placeholders); plan marked complete for P0–P3+P6 |
| 2026-07-16 | Corrective product-path pass | Drain in-frame redraw signals before present settlement; normalize External geometry to window space; compare static Scene segments; switch default Anim targets to tight widget bounds with region composite. Gallery Spinner steady-state measured `scene_build_base=0`, `encode_base=0`; G3/G4 PresentMon still pending. |

---

## 6. Anim target strategy (Phase 2a gate → P2b)

Gate implementation: `picus_core::runtime::layers` (`AnimTargetStrategy`). Narrative:
[architecture/runtime.md](../architecture/runtime.md) (Masonry layer contract).

### 6.1 Choice

| Field | Value |
|-------|--------|
| **Selected product target** | **`WidgetBoundsTexture`** — one tight texture per Anim entry plus painter-order dirty-region composite |
| Fallback | `FullWindowTransparent` — compatibility/debug path |
| Deferred | Packed atlas allocation for many simultaneous Anim entries |
| Boundary path | Picus `AnimLayerHost` + Masonry `PaintLayerMode::External` slots (not upstream-only isolation) |

The first vertical slice used full-window transparent targets. Product-path review
showed that this still multiplied pixel work at large window sizes, so the selected
path now derives a physical target from each External slot's window-space bounds and
composites it back into the ordered intermediate through a region viewport/scissor.
Once a complete intermediate exists, Anim-only frames replay all intersecting
ordered layers only inside the changed Anim target union.

### 6.2 Size / budget criteria (plan gates)

| Criterion | Intent | How we will judge |
|-----------|--------|-------------------|
| **encode_anim + composite p95** | ≤ **25%** of refresh period (e.g. ≤ 4.17 ms @ 60 Hz) | `PICUS_FRAME_TIMING` `encode_anim_ms` + `composite_ms` content-paint means once P2b wires counters; PresentMon for display path |
| **4K G3** | Spinner design phases still visible at 3840×2160 | §2 protocol S2 @ 4K after P2b |
| **Drag G4** | Displayed-frame latency p95 ≤ 2 refresh periods; ≥30% better than P0 | §3 PresentMon S3 |
| **Multi-entry** | Multiple anim entries must **not** force full-window **base** clear each tick | Host dirty set encodes only dirty anim entries; base stays cached |

### 6.3 Assumptions (until measured)

**G2 unit contracts** and Spinner gallery product timing (pure-anim
`scene_build_base` / `encode_base` → 0) are delivered. The bullets below remain
planning assumptions for display-path cost; they are not PresentMon G3/G4
acceptance. Fill §3 only with measured counters / PresentMon; do not invent numbers.

1. Tight Anim targets remove full-window clear/encode work and Anim-only intermediate
   recomposition is scissored to the changed target union; Vello fixed overhead and
   the final full-window intermediate-to-swapchain blit remain measurable costs.
2. External bounds are normalized from layer-local into window space before target
   allocation; a 2 px physical pad preserves edge antialiasing.
3. Static full redraws compare retained Scene+transform snapshots per painter-order
   run, so local hover changes do not bump every cached segment.
4. **Mailbox** present remains preferred for G4; target strategy does not replace
   present-mode policy.
5. **G10 done:** product path has **no** default anim present throttle. Optional
   `PICUS_ANIM_PRESENT_HZ` positive-Hz cap is diagnostic only (G5 still never
   blocked). Full PresentMon G3/G4 remain separate measurement work.

### 6.4 Comparison snapshot (qualitative)

| Dimension | FullWindowTransparent | WidgetBoundsTexture |
|-----------|----------------------|-------------------|
| Implementation risk | Lower (compatibility fallback) | Higher (window/physical bounds + region composite) |
| encode_anim vs resolution | Scales with window pixels | Scales with widget dirty union |
| Clip/scroll correctness | Composite in window space; External slot bounds from layout | Must package ancestor clip with host scene |
| Multi Spinner | One shared anim RT or N full-window RTs (compatibility path) | N tight textures; recomposite their changed target union |
| When to prefer | Debug/fallback | Product default; packed atlas remains optional |

**Decision:** ship tight per-entry **WidgetBoundsTexture** targets with region
composite. Re-evaluate packed atlas allocation only if many-entry texture count or
encode_anim+composite p95 exceeds the §2 matrix budget.

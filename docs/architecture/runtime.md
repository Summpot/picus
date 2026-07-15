# Runtime architecture

Picus keeps Bevy as the application scheduler and owns one retained runtime per
window. `MasonryRuntime` is a non-send resource containing the window runtimes;
each `WindowRuntime` owns the retained view state, action sink, hit-test state,
and paint bookkeeping for one Bevy window. The primary window is attached
automatically. Additional windows attach when their `Window` entity is seen.

The normal frame flow is:

```text
PreUpdate   input, retained routing, action dispatch
Update      application systems and state changes
PostUpdate  projection invalidation, synthesis, retained rebuild, IME sync
Last        paint and present for each attached window
```

Paint errors are captured as diagnostics. A frame is marked painted only after
`present()` succeeds, so a failed presentation cannot make later lifecycle
logic assume that pixels reached the window. Window size sent to the retained
runtime is logical size; pointer hit testing uses the event window's physical
cursor position.

Fonts registered through `AppPicusExt` are queued in `XilemFontBridge` and
broadcast to every attached window. A window that attaches later replays all
already registered font bytes. Theme backdrops are explicit: the application
can select one with `theme_backdrop`, while an explicit window setting takes
precedence over a stylesheet backdrop.

See [multi-window](../guide/multi-window.md), [i18n and fonts](../guide/i18n-fonts-icons.md),
and [styling and themes](../guide/styling-themes.md) for application-facing
configuration.

### 四条时间线 (four timelines)

The long-term frame architecture separates four independent timelines so that
animation clock, scene build, and present freshness are no longer one OR-coupled
path. Full plan: [plans/frame-pipeline.md](../plans/frame-pipeline.md).

| Timeline | Role | Trigger | Drop policy |
|----------|------|---------|-------------|
| **A Input/Shell** | Pointer, keyboard, move/resize message pump | Events | Do not drop messages |
| **B Anim clock** | Advance `t`, opacity, cursor blink timers | Logical clock (may be 60–120 Hz) | State may jump |
| **C Scene build** | Rewrite + build/encode scene (today: full window; target: painter-order entries) | Only when corresponding content changes | Uncommitted work may merge |
| **D Present** | Submit the latest ready composite | Display path | Mailbox drops stale; FIFO may only backpressure |

**Today (Phase 1 / 1b)** each window’s paint path runs through an internal
`FrameDriver` (`picus_core::runtime::frame_driver`, not on the app facade).
`paint_masonry_ui` → `WindowRuntime::step_frame`, which uses
`FrameDriver::decide_entry` / `decide_present` (there is no `FrameDriver::step`).
`DirtyBudget` aggregates `FirstPaint`, `InputOrRebuild`, `LayoutRewrite`,
`ResizeMetrics`, `AnimPaint { layer }`, `AnimTick`, `CompositorPlan`,
`ThemeOrFont`, `RetrySurface`. Decision flags record intent; **Phase 1
execution only splits anim-tick vs full-window encode/present** —
`do_rewrite` / `do_encode` / `do_present` stay coupled on the content path until
layered textures (Phase 2). Sticky content dirt (`resize_dirty`, `retry_dirty`,
`theme_or_font_dirty`, …) is cleared **only after successful present**.

**Hard rule (G5):** `ResizeMetrics`, `InputOrRebuild`, `FirstPaint`, and
`RetrySurface` are **never** skipped by the anim present throttle. Continuous
widgets (e.g. Spinner) may still full-window encode this phase; scheduling
semantics are correct so interaction/resize redraws are not blocked.

A **transitional** pure-animation present throttle (~30 Hz default; override
`PICUS_ANIM_PRESENT_HZ` with a positive Hz, or `0` / `off` / `none` / `false` to
disable) reduces DWM drag ghosting. That throttle is **not** the end state; it is
removed only after layered anim encode gates pass (G10). Anim tick and present
are **not** inseparably tied: pure `AnimTick` may skip encode/present while
keeping the event loop awake. Non-G5 content co-occurring with the anim clock
(e.g. `LayoutRewrite` + `AnimTick`) may also be delayed by the interval.

**PresentPolicy (G7):** surface creation negotiates an explicit capability —
`MailboxLatest` (GPU/compositor may replace queued frames) or
`FifoBackpressure` (name covers FIFO / FifoRelaxed / AutoVsync / Immediate
fallbacks that are *not* Mailbox; does not imply every non-mailbox mode is true
FIFO queueing). `LatestReadyQueue` is a **helper** for CPU-side latest-only
coalescing of *unsubmitted* frames (unit-tested; **not yet on the hot present
path** — present remains single in-flight submit). Submitted FIFO frames are
**not** claimed withdrawable. There is no fake unified `drop_stale` boolean
across modes. Runtime logs mode + strategy at surface init. Shared helper:
`picus_surface::select_present_mode` / `PresentPolicy::negotiate`.

**Observability:** set `PICUS_FRAME_TIMING=1` for per-window phase averages and a
monotonic `frame_id` (`input_dispatch_ms`, `anim_tick_ms`,
`scene_build_base_ms` / `scene_build_anim_ms`, `surface_acquire_ms`,
`encode_*_ms`, `composite_ms`, `present_submit_ms`, `presented` /
`anim_tick_only`). These are **CPU submit-path** times — not displayed-frame
latency.

**Phase instrumentation honesty:** `anim_tick_ms` includes rewrite that Masonry
performs inside `AnimFrame`. `scene_build_base_ms` is only the subsequent root
`redraw()` call. Present-path averages (`encode_*`, `composite`,
`present_submit`, …) and process `paint_ms` / `present_ms` are over **content
paint attempts** (`frames − anim_tick_only`), not diluted by throttled
anim-only zeros. Process log `frames` counts **per-window paint attempts**; ECS
averages use `bevy_frames`. Idle pure-`Skipped` paint does not assign `frame_id`s
but still flushes process summaries on a ~1s wall clock.

Windows baselines require PresentMon/ETW; protocol and result template:
[perf/frame-pipeline-baseline.md](../perf/frame-pipeline-baseline.md).

<<<<<<< HEAD
### Bevy redraw semantics (Phase 1b)

After each window `step_frame`, the host returns an internal **`RedrawDemand`**
(not on the app facade):

| Flag | Meaning | Typical sources |
|------|---------|-----------------|
| `need_anim_tick` | Timeline B — schedule another Bevy frame so the anim clock can advance | `needs_anim_frame`, `render_root.needs_anim()`, throttled pure-anim skip |
| `need_content_present` | Timeline C/D — content encode/present still owed | `needs_redraw`, `resize_dirty`, `retry_dirty`, `theme_or_font_dirty`, rewrite passes |

`paint_masonry_ui` OR-merges demands across windows and writes a single Bevy
`RequestRedraw` **only when either flag is set** (ContentPresent **or** AnimTick
scheduling). Classification is unit-tested (Failed present never sets
`need_content_present`; throttled AnimPaint stays anim-only and does not
escalate to `InputOrRebuild`).

#### Relationship to `WinitSettings` reactive mode

`run_picus` installs latency-bounded reactive updates when the app has not
already inserted `WinitSettings` (`bevy_winit`):

- focused: `UpdateMode::reactive(~1/120 s)` — wake on window/device/user events,
  `RequestRedraw`, or the wait timeout
- unfocused: `UpdateMode::reactive_low_power(~1/30 s)` — ignores pure device
  motion; still wakes on window/user events and `RequestRedraw`

Implications:

1. **Idle UI sleeps** until input, resize, proxy wake, timeout, or Picus
   `RequestRedraw` — we do **not** use continuous/game mode by default.
2. **Any** `RequestRedraw` (anim-only or content) runs a **full Bevy schedule**
   (`PreUpdate` → `Update` → `PostUpdate` → `Last` paint). Bevy has no public
   “paint-only / Last-only” update path; Phase 1b therefore **classifies**
   demand but does **not** skip the system table for pure `AnimTick`.
3. Tradeoff (P1b.2): avoiding full empty spins on anim-only wakes would need a
   custom winit integration or a dedicated anim timer outside the full schedule
   — deferred; measurable today is correct wake **reason** and no Failed/content
   redraw loops.
4. Two layers stay separate: (a) content stickies set `need_content_present` so
   Bevy **wakes** (a `RequestRedraw` is written and not dropped); (b) G5 dirty
   reasons (`FirstPaint` / `InputOrRebuild` / `ResizeMetrics` / `RetrySurface`)
   still force **unthrottled encode/present** on the FrameDriver path. Wake
   demand alone does not imply G5: e.g. rewrite-only content demand can still
   hit the transitional LayoutRewrite+AnimTick present throttle while Bevy is
   awake.

App public API remains `run_picus`; `FrameDriver` / `RedrawDemand` stay internal.
=======
### Masonry layer contract (Phase 2a hard gate)

Before multi-texture composite (P2b), Picus freezes the Masonry boundary. Source of
truth for the inventory and selected interface:
`picus_core::runtime::layers` (crate-private; not on the app facade).

#### Gate questions and results (xilem rev `4b1922c`)

| # | Question | Result |
|---|----------|--------|
| 1 | Can `PaintLayerMode` / `VisualLayerPlan` yield **self-contained, independently renderable** painter-order entries under ancestor clip/scroll, transform, ZStack, overlay? | **No for product isolation.** Empirical FAIL on: (a) **sticky isolation** — mode resets to Inline each pass; clean widgets drop IsolatedScene/**External** on next redraw; (b) **missing clip package** — `VisualLayer` is only `kind` / `transform` / `widget_id` (External adds `bounds`); no ancestor clip-chain field; (c) flatten helpers **skip** External. **Not separately spiked:** scroll portals, ZStack front/back, Masonry overlay `layer_root_ids` stack — FAIL still stands without those spikes because isolation is non-sticky and layers lack clip metadata. Transforms bake into scene/local space when a split *does* occur. No persistent compositor `LayerId` (checklist; upstream FIXME). |
| 2 | Can an anim tick emit **only the changed anim entry** without full-tree `RenderRoot::redraw()` and without reassembling base scene? | **No.** Public path is only full `redraw()` → paint pass. Consecutive redraws always reassemble a full plan. Per-widget `scene_cache` may skip re-recording clean widgets; that is not selective layer rebuild. |

**Evidence classes** (see `MasonryLayerCapabilities` / `CapabilityEvidence`): sticky
isolation, External slot/skip/collapse, clip type-shape, and full-redraw reassembly are
**empirical spikes**. `persistent_layer_id: false` is an **inventory checklist** bit
(re-audit on pin bump).

**Forbidden reading:** classifying a post-hoc `VisualLayerPlan` as “per-layer scene
build” is incorrect. The plan is a full-pass painter-order snapshot; selective work
must be owned by Picus dirty sets, not by slicing the plan after the fact.

#### Selected path

**Picus `AnimLayerHost`** (not “wait for upstream only”):

- **Masonry:** layout, hit-test, painter-order **`PaintLayerMode::External`** placeholders
  (widget must call `set_paint_layer_mode(External)` **every paint** — mode is not sticky;
  host registration alone does not set mode).
- **Picus host:** independent anim entry state (`AnimLayerId`, bounds, transform, version, dirty); P2b builds anim scenes/textures only for dirty entries.
- **Composite (P2b, not this PR):** exact painter-order composite of base segment(s) + host anim textures + overlays.

**Upstream revision strategy (parallel, non-blocking):** track/contribute persistent
`LayerId`, sticky isolation, self-contained clip/effect on isolated layers, and selective
layer redraw. If a future pin gains
`MasonryLayerCapabilities::supports_upstream_only_anim_isolation()`, Picus may
narrow the host; P2b does not wait on that.

**Failure fallback:** keep Phase 1 full-window encode + transitional anim present
throttle until host+composite land; never claim VisualLayerPlan classification as
isolation.

#### Ownership / lifecycle

**P2a status:** `AnimLayerHost` is a free-standing scaffold in
`picus_core::runtime::layers`. It is **not** a `WindowRuntime` field and is **not**
used by `step_frame` / paint. Current product path remains full-window encode with
`AnimPaint { layer: 0 }`.

**Planned ownership (P2b target)** — diagram below is aspirational:

```mermaid
flowchart TB
  subgraph window["WindowRuntime (planned P2b)"]
    RR["RenderRoot / Masonry<br/>layout · hit-test · External slots"]
    Host["AnimLayerHost (Picus)<br/>AnimLayerId · dirty · version"]
    Surf["LayerSurfaces<br/>base + anim textures"]
  end
  RR -->|"painter-order External placeholder"| Host
  Host -->|"dirty anim entries only"| Surf
  RR -->|"base plan when base_dirty"| Surf
  Surf -->|"exact-order composite"| Present["swapchain present"]
```

```text
# P2b target contract (not current behavior)

register_external_slot(widget_id)  → AnimLayerId in host maps only
  (widget path must set_paint_layer_mode(External) every paint — not sticky)
layout/compose                     → update bounds/transform; CompositorPlan if plan changes
AnimFrame tick                     → host.mark_anim_paint(id)
                                     [target] skip base rewrite when only anim dirty
encode                             → [target] dirty host entries only; base if base_dirty
unmount                            → host.remove_widget; External slot drops on next plan
```

#### Anim target choice (size gate input)

| Strategy | Encode shape | First P2b? |
|----------|--------------|------------|
| **Full-window transparent** anim target; only anim widgets paint | Full-window clear of anim RT; sparse scene | **Yes** (plan §2.0) |
| Widget-bounds / atlas | Smaller encode; more bookkeeping | Deferred (P4 / if G3·G4 fail) |

Rationale and budget assumptions:
[perf/frame-pipeline-baseline.md](../perf/frame-pipeline-baseline.md) §6.
>>>>>>> 35bce89 (feat: close P2a Masonry layer contract gate + AnimLayerHost spike)

### Frame pipeline evolution

Implementation plan and success metrics G1–G10:
[plans/frame-pipeline.md](../plans/frame-pipeline.md).

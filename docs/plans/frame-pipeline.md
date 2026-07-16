# Picus 帧管线解耦完整计划

> **状态**：**代码修复完成，显示链路验收中**（P0–P3 + P6；P4 紧致 target/region composite 已交付，P5 未开工）
> **范围**：动画时钟 / 内容脏区与层 encode / present 新鲜度 / 与 Bevy·DWM 边界  
> **动机**：消除「动画帧率 vs 拖窗流畅度」假权衡；根因是架构耦合，不是单点旋钮。  
> **诚实边界**：**G2 已在 gallery Spinner 产品路径实测**（稳态 `scene_build_base=0`、`encode_base=0`）；PresentMon **G3/G4** 数字仍为占位（不编造）。

---

## 0. 背景

### 0.1 帧路径（历史 → 当前端态）

**当前端态（P0–P3）**：

```text
PreUpdate   input, retained routing, action dispatch
Update      app systems, style, overlays, …
PostUpdate  projection invalidation, synthesis, retained rebuild, IME
Last        paint_masonry_ui
            → WindowRuntime::step_frame
              → FrameDriver::decide_entry
              → (optional) AnimFrame tick  # then post_dirty
              → FrameDriver::decide_present
              → if encode:
                   · pure AnimPaint (G2): sync Anim hosts → encode dirty Anim
                     (usually no CompositorPlan rebuild / no base reassembly)
                   · content/resize/first-paint: redraw → rebuild CompositorPlan
                     → encode dirty (rewrite+encode+present coupled)
              → ordered composite → present
```

决策表分离 `do_anim_tick` 与 encode/present；**`decide_present` 在 anim tick 之后**（对 post_dirty）。内容路径上 rewrite/encode/present 仍耦合；纯动画稳态走选择性 Anim 层 encode。Sticky 脏标志仅在 **present 成功** 后清除。产品路径 **无** 默认 anim present 节流（G10）；`PICUS_ANIM_PRESENT_HZ` 仅为 diagnostic。权威叙述：[architecture/runtime.md](../architecture/runtime.md)。

**历史（Phase 0 之前）**：`WindowRuntime::paint_frame` 把下列布尔 **OR** 成一条路径：

- `needs_redraw` / `needs_anim_frame` / `render_root.needs_anim()` / `needs_rewrite_passes()`

Spinner 等控件每 tick `request_anim_frame` + `request_paint_only` → **整窗** rewrite + encode + present。

### 0.2 已落地的相关优化（勿回退语义）

| 提交/能力 | 作用 | 与本计划关系 |
|-----------|------|----------------|
| Dev profile 依赖 `opt-level=2` | debug 帧成本下降 | 保留；不替代架构 |
| 增量 synthesis 缓存 | hover/scroll 不全窗投影 | **正交**：ECS 轴；本计划是**像素/present 轴** |
| Nav 未选中页剪枝 / 侧栏默认折叠 | 减投影与 retained 体量 | 保留 |
| Style 无操作不写 | 减假 dirty | 保留 |
| Mailbox 优先 present | 欠载丢中间帧 | **本计划正式化为 PresentPolicy** |
| 纯动画 present ~30Hz 节流 | 拖影缓解 | **已降级为可选 diagnostic**（`PICUS_ANIM_PRESENT_HZ` 显式正 Hz；unset = 不节流） |

### 0.3 问题对照

| 现象 | 结构原因 |
|------|----------|
| Spinner 页拖窗拖影 | 壳由 DWM 动，内容由全窗同步 present 更新；Fifo/排队或算不过来 → 内容滞后 |
| 动画想 60Hz 像在抢拖窗预算 | 动画推进被实现成「全窗 present 泵」 |
| debug 比 release 拖影重 | 同架构下单帧更贵，积压更易出现 |
| hover/scroll 曾卡顿 | 全窗 synthesis（已用增量缓存缓解，属另一轴） |

### 0.4 目标一句话

> **把「动画时钟」「内容脏区/层 encode」「以最新帧为目标的 present」拆成可独立调度的管线；高频动画离开 base 全窗路径；present 只服务合成器新鲜度，不为每个 anim tick 担保一次全窗提交。**

---

## 1. 成功度量

| ID | 目标 | 验收方式 |
|----|------|----------|
| G1 | 四条时间线在代码与文档中可命名、可按窗口/帧 ID 度量 | `PICUS_FRAME_TIMING` per-window + `frame_id` 分 phase（FrameDriver 在 P1）；显示链路用 PresentMon/ETW |
| G2 | 纯 Spinner 页：`encode_base` 均摊接近 0；动画只触达 anim 层 | **单元合同已交付**；display 路径仍待 PresentMon / timing 实测 |
| G3 | Spinner 页静止窗口：动画视觉流畅（目标显示刷新贴近显示链路，不靠永久 30Hz 全局 throttle） | 人工 + 可选 PresentMon |
| G4 | Spinner 页拖窗：拖影明显轻于架构改造前；**默认产品路径不依赖永久砍动画 fps** | 人工对比 debug/release |
| G5 | Resize / 交互 redraw **永不**被「动画节流」挡住 | 单测 + 手工 |
| G6 | Idle（无 anim dirty）：几乎不 present | timing `painted` 比例 |
| G7 | Present 语义 = 最新帧；能力协商 Mailbox 优先 | surface 单测 + 运行时 debug 日志 |
| G8 | 应用公共 API 不变：`run_picus` / facade / 无 `__macro_support` | examples 编译与既有测试 |
| G9 | `AGENTS.md` + `docs/architecture/runtime.md` 与实现同步 | 文档 PR 与代码同栈 |
| G10 | 过渡 30Hz anim throttle 从默认路径移除或降为显式 opt-in | 代码审查 — **P2e 已交付** |

---

## 2. 非目标

- 重写 Bevy 或废弃 retained Masonry  
- 要求立即 GPU 粒子/游戏级特效架构  
- 把「永久 30Hz 动画」写成产品默认  
- 应用层每个 demo 自己管 present  
- 破坏「无主题 ≈ 无可见样式」、无自动 dark/light  
- 改变 `UiAction` / 投影注册 / BSN 作者路径  
- 本计划**不**重做 ECS 增量 synthesis（已有缓存；只保证不冲突）

---

## 3. 目标架构

### 3.1 四条时间线

| 线 | 职责 | 触发 | 丢帧策略 |
|----|------|------|----------|
| **A Input/Shell** | 指针、键盘、move/resize 消息泵 | 事件 | 不丢消息 |
| **B Anim clock** | 推进 `t`、opacity、光标计时 | 逻辑时钟（可 60–120Hz） | 状态可跳 |
| **C Scene build** | rewrite + **按层** encode | 仅显示内容变 | 可合并 |
| **D Present** | 提交**最新**缓冲 | 显示链路 | **丢旧保新** |

### 3.2 窗口合成图

```text
Window (swapchain)
├── Base layer       # chrome + 当前页；低频；脏才 encode
├── Overlay layers   # 对齐现有 overlay（z 序）
└── Anim layer(s)    # Spinner / indeterminate / 可选光标；高频小代价
```

每层：`LayerId`、bounds、dirty 标志、scene 或 texture、与 Masonry visual layer 的映射策略。

### 3.3 脏因（FrameDriver 输入）

```text
DirtyReason =
  | FirstPaint
  | InputOrRebuild      # ECS rebuild / 明确 needs_redraw
  | LayoutRewrite
  | ResizeMetrics
  | AnimPaint { layer } # 仅某层像素变
  | AnimTick            # 只要时钟，不要像素
  | ThemeOrFont
  | RetrySurface
```

### 3.4 PresentPolicy

```text
PresentPolicy {
  mode_preference: Mailbox > FifoRelaxed > AutoVsync > Fifo
  desired_maximum_frame_latency: 1 // backend hint，不是硬保证
  ready_queue: LatestOnly           // 只丢尚未提交的旧合成结果
  // 协商结果（勿伪造统一 drop_stale）:
  //   MailboxLatest     — GPU/compositor 可替换已排队帧
  //   FifoBackpressure  — CPU 侧合并未提交帧 + 背压；已 submit 的 FIFO 帧不可撤回
}
```

`drop_stale` 不作为跨 present mode 的布尔承诺：已经提交给 FIFO/FifoRelaxed
swapchain 的帧无法由 Picus 撤回。运行时必须记录实际模式和生效的 fallback
策略，验收报告按模式分组。

### 3.5 与 Bevy 的边界

- Bevy 仍是应用调度器；**不**把「每次 Update 醒来」等同「全窗 UI 帧」。  
- `RequestRedraw` 语义收敛为：有 **ContentPresent** 或 **AnimTick 需要调度** 时再写。  
- 可选后续：`NeedAnimTick` / `NeedContentPresent` 分消息（Phase 1b）。

---

## 4. 模块落点

| 模块 | 路径（建议） | 职责 |
|------|----------------|------|
| FrameDriver | `picus_core::runtime::frame_driver` | 决策表 `decide_entry` / `decide_present`；宿主 `WindowRuntime::step_frame` 执行 |
| DirtyBudget / reasons | 同上或 `frame_dirty.rs` | 聚合本帧脏因 |
| PresentPolicy | `picus_surface` + core 薄封装 | 模式协商、latency |
| LayerRegistry | `picus_core::runtime::layers` | base/anim/overlay 层表与 texture |
| Anim isolation API | `picus_widget` + projection | 控件声明 `PaintIsolation` / anim layer |
| Timing | `picus_core::perf` | phase 扩展 |
| 文档 | `docs/architecture/runtime.md` + 本计划 | 权威叙述 |

**不**把 FrameDriver 放进应用 facade 公共面；应用仍只 `run_picus`。

---

## 5. 分阶段实施计划

### Phase 0 — 语义、度量、去过渡债

**目标**：可观测、可对比；避免 30Hz 节流变成「正式架构」。

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P0.1 | `PICUS_FRAME_TIMING`：per-window + 单调 `frame_id`；`input_dispatch_ms`、`anim_tick_ms`、`scene_build_base_ms`、`scene_build_anim_ms`（分层前为 0）、`surface_acquire_ms`、`encode_*_ms`、`composite_ms`、`present_submit_ms`、`presented`/`anim_tick_only`；文档标明 CPU submit ≠ 显示时间 | **已交付** |
| P0.2 | `runtime.md`「四条时间线」小节链到本计划 | **已交付** |
| P0.3 | 曾保留 ~30Hz animation-only throttle 为 transitional 默认 + `PICUS_ANIM_PRESENT_HZ` override；**P2e 后默认改为不节流**（override 仍为 diagnostic） | **已交付**（默认值见 P2e） |
| P0.4 | 可重复基线协议：1920×1080 与 3840×2160、固定拖窗轨迹、10s warm-up + 30s 采样、debug/release 各 ×3；Windows 上 PresentMon/ETW **必跑** | **已交付**（协议文档） |
| P0.5 | 版本化 `docs/perf/frame-pipeline-baseline.md`（环境、CSV/ETL 摘要占位、median/p95/p99、验收阈值） | **已交付**（模板；实测数字待首轮填写） |

**验收**：G1 度量骨架；协议可由另一台 Windows 开发机复跑；**P0 合并后**产品行为曾与当时版本一致（仍默认 ~30Hz 纯动画 present 节流；**P0 当时；P2e 已改为 unset=不节流**）。  
**风险**：过早关掉默认 30Hz → 拖影回潮；P2e 已在分层 G2 合同后去掉默认节流，Mailbox 仍为 present 策略。

**建议 PR**：`PR0-metrics-docs-throttle-policy`

---

### Phase 1 — FrameDriver（单缓冲，全窗 encode 仍可保留）

**目标**：用显式调度替换 `needs_*` 大 OR；**不**要求立刻分层纹理。

| 工作项 | 细节 |
|--------|------|
| P1.1 | 引入 `FrameDriver` / `FrameStepResult`；`paint_masonry_ui` → `WindowRuntime::step_frame` + `decide_*` | **已交付** |
| P1.2 | 脏因枚举 + 从 Masonry 信号 / `incoming_redraw` / resize 填充 | **已交付** |
| P1.3 | 决策表标志分离；Phase 1 执行仅分 anim-tick vs encode/present（rewrite 仍耦合） | **已交付** |
| P1.4 | **硬规则**：`ResizeMetrics`、`InputOrRebuild`、`FirstPaint`、`RetrySurface` → 不得被 anim 节流跳过 present | **已交付** |
| P1.5 | `PresentPolicy` 从 surface 创建路径抽出；与 core 共享 `select_present_mode` | **已交付** |
| P1.6 | 单测：AnimTick skip / Resize 必 present / sticky restore / ready queue | **已交付** |
| P1.7 | 删除或旁路「anim 与 present 绑死」的隐式假设注释 | **已交付** |

**验收**：G5、G7；代码路径可读；Spinner 仍可能全窗 encode，但调度语义正确。  
**风险**：回归漏画 → 保留 `has_painted_once` 与 Retry 路径测试；sticky 仅在 present 成功后清除。

**建议 PR**：`PR1-frame-driver`（可拆 1a driver、1b policy 若 diff 过大） — **已合并到本分支**

---

### Phase 1b — Bevy 唤醒语义（可选但推荐紧随 P1）

| 工作项 | 细节 |
|--------|------|
| P1b.1 | 区分「需要 anim 调度」与「需要内容 present」的 redraw 请求 |
| P1b.2 | 无 ContentPresent 且仅 AnimTick 时，避免无意义的整表 Bevy 系统空转放大（在可测前提下） |
| P1b.3 | 文档：与 `WinitSettings` reactive 模式的关系 |

**建议 PR**：`PR1b-redraw-semantics`（可与 P1 合并若小）

---

### Phase 2 — 合成层与 Anim layer 纹理（解拖影的主路径）

**目标**：高频动画不脏 base 全窗 encode。

#### 2.0 设计冻结（P2 开工前写进 PR 描述）

- Base target：现有全窗 offscreen（或 swapchain 兼容路径）  
- Anim layer：默认使用 widget bounds 紧致 texture；region viewport/scissor 按 painter order 合成；全窗透明仅作内部 fallback
- Present：`composite(base, anim_layers…) → swapchain`（可用现有 blitter 扩展）  
- Masonry：评估 `VisualLayerPlan` / overlay_layers 是否可映射；不够则 Picus 侧维护 `AnimLayerHost`  

##### 2.0a Phase 2a hard gate（**关闭后方可 P2b**）— **已完成**

权威叙述：[`docs/architecture/runtime.md`](../architecture/runtime.md)「Masonry layer
contract」；代码：`picus_core::runtime::layers`；目标尺寸决策：
[`docs/perf/frame-pipeline-baseline.md`](../perf/frame-pipeline-baseline.md) §6。

| 门禁项 | 结论 |
|--------|------|
| 自包含可独立 encode 的 painter-order entry（clip/scroll/transform/ZStack/overlay） | **上游不足（实证）**：sticky isolation 失败（mode 每 pass 重置；External/Isolated 二次 redraw 塌缩）；`VisualLayer` 无 clip 字段；flatten 跳过 External。**未单独 spike**：scroll / ZStack / Masonry overlay stack（决策仍 FAIL）。`LayerId` 为清单位。 |
| anim tick 只发变更 entry、免全树 `redraw`/base 重装 | **上游不足（实证）**：仅有全量 `RenderRoot::redraw` 重装 plan；`scene_cache` ≠ 按层 rebuild |
| 选型 | **`AnimLayerHost` scaffold**（P2a 未挂到 `WindowRuntime`）；P2b 再接 External 槽 + 脏集 |
| Anim target | **`WidgetBoundsTexture`**（每 widget 紧致 texture + region composite；packed atlas 后续可加）；`FullWindowTransparent` fallback |
| 上游策略 | 并行跟踪 LayerId / sticky isolation / self-contained clip / selective redraw；**不阻塞** P2b |
| 失败回退 | P1 全窗 encode；可选 `PICUS_ANIM_PRESENT_HZ` diagnostic cap（产品路径默认不节流） |
| 禁止 | 把 post-hoc `VisualLayerPlan` 分类说成 “per-layer scene build” |

**未做（故意）**：多 texture composite、Spinner 真拆层 encode、`step_frame` 挂 host、纯 anim 免 base rewrite——属 P2b。Host `register_*` **不**设置 paint mode；控件须每 paint `set_paint_layer_mode(External)`。

#### 2.1 基础设施

| 工作项 | 细节 |
|--------|------|
| P2.1 | `LayerKind::{Base, Anim, Overlay}` + 每窗 `LayerSurfaces` |
| P2.2 | `picus_surface`：多 texture blit / 有序 composite present API |
| P2.3 | Dirty：`base_dirty` / `anim_dirty`；仅 dirty 层 encode |
| P2.4 | Timing：`encode_base_ms` / `encode_anim_ms` / `composite_ms` |
| P2.5 | Resize：所有层随 metrics 重建；FirstPaint 全层 |

#### 2.2 垂直切片：UiSpinner — **P2c 已交付（G2 产品路径 + 单测）**

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P2.6 / P2.7 | `Spinner` 每 paint `PaintLayerMode::External`（局部实现，无 gallery/entity 特判）；External 自动 `register_external_slot` → Anim entry | **已交付** |
| P2.8 | Spinner 像素只在 host window-space scene；cached segments 不含 spinner；painter order 前后景不变 | **已交付** |
| P2.9 | 12-step visual phase 门控 `request_paint_only` / host version；相位未变 tick 不 encode/present；稳态 selective path 免全树 `redraw`、免 base reassemble/encode | **已交付** |
| P2.10 | 层合同与 gallery timing 证明 pure anim → 仅 Anim `needs_encode`、`scene_build_base=0`、`encode_base=0`；G4 PresentMon 协议见 baseline 文档 | **部分**（G2 产品路径通过；G3/G4 数据待） |

#### 2.3 扩展 — **P2d indeterminate ProgressBar 已交付（G2 层合同）**

| 工作项 | 细节 |
|--------|------|
| P2.10+ / P2d | Indeterminate `UiProgressBar` 迁 anim 层（G2 单测） | **已交付** |
| P2.11 | （可选）光标闪烁：评估 TextArea 是否适合 anim 层或保持 paint_only 低频 |

#### 2.4 去默认 anim present throttle — **P2e / G10 已交付（代码路径）**

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P2e.1 | 产品路径 unset `PICUS_ANIM_PRESENT_HZ` → **不节流**（不再默认 ~33ms） | **已交付** |
| P2e.2 | `PICUS_ANIM_PRESENT_HZ` 正 Hz = diagnostic opt-in（仅 anim-driven；G5 永不挡） | **已交付** |
| P2e.3 | `0` / `off` / `none` / `false` = 不节流；非法值警告后亦不节流 | **已交付** |
| P2e.4 | 文档：`runtime.md`、`frame-pipeline-baseline.md`、本计划进度 | **已交付** |
| P2e.5 | PresentMon G3/G4 实测数字 | **未强制**（占位表保留；不编造数字） |

**验收说明**：G10 代码审查项完成。Spinner G2 gallery 产品路径、Spinner/ProgressBar 层合同与 PresentPolicy FIFO/Mailbox 单测已在树内；**不**将 G3/G4 PresentMon 数字写成已验收。

**建议 PR 栈**（历史命名；工作项 ID 以 §12 进度表为准）：

1. `PR2a-layer-surfaces-composite`  
2. `PR2b` / Spinner anim entry（计划工作项 **P2c**）  
3. `PR2c` / indeterminate ProgressBar（计划工作项 **P2d**）  
4. `PR2d-remove-default-anim-throttle`（分支历史名）→ 计划工作项 **P2e / G10**（本 PR）

---

### Phase 3 — 控件声明 API（PaintIsolation）— **已交付（本 PR）**

**目标**：高频动画**必须**声明隔离；默认控件走 base。

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P3.1 | `PaintIsolation::{Inline, AnimEntry}` 在 `picus_widget`（painter slot，非全局顶层；facade 不进 prelude） | **已交付** |
| P3.2 | `Spinner` / indeterminate `ProgressBar` 默认 `AnimEntry`；determinate `Inline` | **已交付** |
| P3.3 | Host：discovery 仍为已知类型 allowlist；**promotion** 按 isolation 提升 External→Anim；稳定 layer id | **已交付** |
| P3.4 | 文档：[`docs/guide/paint-isolation.md`](../guide/paint-isolation.md)（含 discovery 限制 + path forward）+ runtime / public-modules | **已交付** |
| P3.5 | AGENTS 硬规则：持续 ~60Hz 视觉动画不得默认脏整窗 base present 路径 | **已交付** |

**验收**：新控件有明确约定；gallery 无 hardcode 特例；Spinner/ProgressBar 走公共 isolation（discovery + host 场景绘制仍 type-dispatch；自定义仅 `apply` 不提升）。  
**建议 PR**：`PR3-paint-isolation-api`

---

### Phase 4 — 紧致 Anim target / atlas（部分完成）

**目标**：进一步降 anim encode 成本（全窗透明 anim 层仍可能偏贵时）。

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P4.1 | External layer-local bounds 经 transform 规范为 window-space，再转 physical target | **已交付** |
| P4.2 | 每 Anim entry 使用紧致 texture（packed atlas 尚未做） | **已交付** |
| P4.3 | region viewport/scissor 按 painter order 合成；纯 Anim 帧仅在变更 target 并集内重放相交层 | **已交付**（含紧致 target 与 full-source scissor 的 headless GPU 像素测试） |
| P4.4 | 多 Spinner / 大窗口 `encode_anim_ms` 与 PresentMon 矩阵 | **部分**（本机 debug smoke；正式矩阵待） |

**验收**：大窗口单 Spinner 的 encode 成本显著低于全窗。  
**建议 PR**：`PR4-anim-dirty-rect`（可延后）

---

### Phase 5 — 异步 encode（可选）

**目标**：UI 线程在拖窗时不被 Vello 堵住。

| 工作项 | 细节 |
|--------|------|
| P5.1 | Encode 任务队列 + 「最新任务 ID」；完成时若已过期则丢弃 |
| P5.2 | UI 线程：input + FrameDriver 决策 + present 最新就绪缓冲 |
| P5.3 | 线程安全：scene 快照或双缓冲 command  
| P5.4 | 取消/窗口销毁生命周期  
| P5.5 | 压力测试：Spinner + 拖窗 + 快速 resize |

**验收**：拖窗时消息泵与 present 新鲜度优于同步 encode。  
**风险**：竞态、内存、调试难度——**仅在 P2 收益确认后启动**。  
**建议 PR**：`PR5a-async-encode-prototype` → `PR5b-stabilize`

---

### Phase 6 — 文档收尾与清理 — **已交付**

| 工作项 | 细节 | 状态 |
|--------|------|------|
| P6.1 | 重写 `docs/architecture/runtime.md` 帧阶段与层模型（端态叙述） | **已交付** |
| P6.2 | `docs/README.md` 链计划 + paint-isolation + baseline；examples 索引 Spinner 说明 | **已交付** |
| P6.3 | 删除遗留「默认 30Hz 仍为产品路径」类注释 / 过时脚手架措辞 | **已交付** |
| P6.4 | 本计划状态改为「完成」；进度摘要与前后对比（诚实：G2 合同 vs G3/G4 占位） | **已交付** |

**建议 PR**：`PR6-docs-cleanup`

---

## 6. PR 依赖图（拓扑序）

```text
PR0-metrics-docs-throttle-policy
    │
    ▼
PR1-frame-driver ──► PR1b-redraw-semantics（可选）
    │
    ▼
PR2a-layer-surfaces-composite
    │
    ▼
PR2b-spinner-anim-layer          (plan work item P2c)
    │
    ├──────────────► PR2c-progress-indeterminate  (plan work item P2d)
    │
    ▼
PR2d-remove-default-anim-throttle  (branch/historical name; plan work item P2e / G10)
    │
    ▼
PR3-paint-isolation-api
    │
    ├──────────────► PR4-anim-dirty-rect（可选）
    │
    └──────────────► PR5a/b-async-encode（可选，建议 PR4 后或并行评估）
    │
    ▼
PR6-docs-cleanup
```

**并行机会**：

- PR1b 可与 PR2a 早期调研并行  
- PR2c 与 PR3 部分文档可并行  
- PR4 / PR5 互不阻塞，但都依赖 PR2b  

---

## 7. 测试策略

| 层级 | 内容 |
|------|------|
| 单元 | `select_present_mode`；FrameDriver 决策表（resize 不节流、仅 AnimTick 可 skip present）；layer dirty 标志 |
| 集成 | headless 或 test harness：rebuild 后 present 成功才 `has_painted_once`；surface Retry |
| 回归 | 现有 `picus_core` / gallery 测试全绿 |
| 性能 | `PICUS_FRAME_TIMING=1` 场景矩阵（下表） |
| 人工 | gallery Spinner 拖窗、侧栏 hover、滚动、主题切换 |

### 7.1 性能场景矩阵（每阶段 PR 至少跑 debug）

| 场景 | 关注指标 |
|------|----------|
| Button idle | `painted`≈0，`synth_dirty`≈0 |
| Hover sidebar | `avg_cache_hits` 高，synth 低 |
| Scroll content | 无整窗 projection 尖刺 |
| Spinner idle | `encode_base`→0（P2 后），anim 成本有界 |
| Spinner drag window | 拖影主观分；present 无长排队 |
| Window resize | 跟手；无卡死 |

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 分层 composite 与 Mica alpha 不一致 | 跟现有 blitter/预乘路径；透明窗专项测 |
| Masonry 层模型不够用 | Picus 侧 LayerRegistry 不阻塞；逐步对齐 VisualLayer |
| 双层导致 hit/视觉错位 | Anim 仅绘制隔离；布局仍单树 |
| 去掉 30Hz 后拖影回潮 | 严格 P2 完成后再默认去掉；Mailbox 保持 |
| 异步 encode 复杂度爆炸 | P5 可选；需 P2 数据支撑 |
| 范围蔓延到 ECS 投影大重构 | 明确非目标；synthesis 缓存只维护兼容 |

---

## 9. 工作量粗估（工程人日，单人参考）

| 阶段 | 粗估 |
|------|------|
| P0 | 1–2 |
| P1 (+1b) | 3–5 |
| P2a–d | 8–14 |
| P3 | 2–3 |
| P4 可选 | 3–6 |
| P5 可选 | 8–15 |
| P6 | 1–2 |
| **必做合计 (P0–P3+P6)** | **约 15–26** |
| **含 P4+P5** | **约 26–47** |

---

## 10. 里程碑

| 里程碑 | 包含 | 对外可感知结果 |
|--------|------|----------------|
| M1 语义 | P0+P1 | **完成** — 帧调度可读；度量齐全 |
| M2 分层 | P2 | **完成** — Spinner G2 产品路径通过；默认无需动画 throttle；G3/G4 实测待填 |
| M3 API | P3 | **完成** — `PaintIsolation` 约定稳定 |
| M4 增强 | P4/P5 | **部分** — 紧致 target/region composite 已交付；packed atlas/P5 待 |
| M5 收尾 | P6 | **完成** — 文档与清理 |

---

## 11. 实施约定

1. **每 PR 可独立合并、可回滚**；禁止「大爆炸」单 PR 含 P0–P5。  
2. **先度量后删 throttle**：P2d 依赖 Spinner 层验收。  
3. **契约双更**：行为变则 `docs/architecture/runtime.md` + 必要时 `AGENTS.md` 一条硬规则。  
4. **应用无感**：examples 除 gallery 观感外不应改业务 API。  
5. **与增量 synthesis**：FrameDriver 在 `Last`；synthesis 仍在 `PostUpdate`；仅消费 rebuild 结果，不重入投影。  

---

## 12. 进度跟踪

| 阶段 | 状态 |
|------|------|
| P0 | **完成**（度量骨架 + 文档 + 节流策略与 override；基线表待实测填写） |
| P1 | **完成**（FrameDriver 调度 + PresentPolicy；内容路径 encode 仍耦合） |
| P1b | 部分/可后续（redraw 语义与 FrameDriver 粘性修复已叠在 P1 分支） |
| P2a | **完成**（Masonry 层契约硬门禁 + `AnimLayerHost` + 目标策略文档） |
| P2b | **完成**（`CompositorEntryKind` + 稳定 `LayerId`、painter-order plan、dirty/version、`render_ordered_frame`、resize metrics generation、timing 分桶） |
| P2c | **完成**（Spinner External isolation、host scene、12-step phase gate、selective anim encode / **G2 产品路径实测**） |
| P2d | **完成**（indeterminate ProgressBar anim 层 + **G2 层合同单测**） |
| P2e | **完成 / G10**（unset = 不节流；`PICUS_ANIM_PRESENT_HZ` diagnostic opt-in；G5 永不被挡） |
| P3 | **完成**（`PaintIsolation::{Inline, AnimEntry}`；guide + AGENTS 硬规则） |
| P4 | **部分完成**（widget-bounds target + region composite；packed atlas/正式矩阵待） |
| P5 | 可选·未开始 |
| P6 | **完成**（runtime 端态叙述、README/examples 链接、过时注释清理、本计划收尾） |

### 12.1 前后对比（诚实摘要）

| 维度 | 改造前 | 改造后（P0–P3 端态） | 验收诚实度 |
|------|--------|----------------------|------------|
| 调度 | `needs_*` 大 OR → 整窗 paint | `FrameDriver` + `DirtyBudget` 分 entry/present | **G1** 代码+文档 |
| 动画 present | 与全窗 encode/present 绑死；后有 ~30Hz 默认节流 | 默认不节流（G10）；可选 `PICUS_ANIM_PRESENT_HZ` | **G10** 代码审查 |
| 层 | 无；每 tick 全窗 | painter-order `CompositorPlan`；Spinner/indeterminate bar → Anim entry | **G2** 单元合同 |
| 纯 anim 成本 | `encode_base` 每 tick | 稳态 selective：免 base reassembly/encode；Anim 用紧致 target | **G2** gallery timing + 单测；非 PresentMon |
| 交互/resize | 可被 anim 路径挤占语义 | G5 永不被 anim throttle 挡 | **G5** 单测 |
| Present 策略 | 隐式 | `PresentPolicy` Mailbox 优先 + FIFO fallback 显式 | **G7** 单测 |
| 控件 API | 无公共 isolation | `PaintIsolation::{Inline, AnimEntry}` | **P3** |
| 静止窗流畅（G3） | 靠砍 fps 或拖影 | 架构上不依赖永久 30Hz | **未填 PresentMon 数字**（表可占位） |
| 拖窗拖影（G4） | 全窗同步 present 易滞后 | Anim 离 base；Mailbox 保新鲜度 | **未填 PresentMon 数字**（表可占位） |

**结论**：必做架构、G2 产品路径与 P4 紧致 target slice 已完成；**G3/G4 仍以 PresentMon 实测为准**，[`docs/perf/frame-pipeline-baseline.md`](../perf/frame-pipeline-baseline.md) 中显示链路数字在首轮正式采集前保持占位，**禁止编造**。

---

## 13. 参考

- 本会话架构讨论：四条时间线、假权衡、DWM 拖影  
- [architecture/runtime.md](../architecture/runtime.md)  
- [architecture/projection.md](../architecture/projection.md)（ECS 轴，正交）  
- [perf/frame-pipeline-baseline.md](../perf/frame-pipeline-baseline.md)（基线协议与结果模板）  
- 已合并 perf 相关提交：`19cb1a9`、`33164d2`、`54f0f91`  

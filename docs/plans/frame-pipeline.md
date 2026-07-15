# Picus 帧管线解耦完整计划

> **状态**：计划中（2026-07-16）  
> **范围**：动画时钟 / 内容脏区与层 encode / present 新鲜度 / 与 Bevy·DWM 边界  
> **动机**：消除「动画帧率 vs 拖窗流畅度」假权衡；根因是架构耦合，不是单点旋钮。

---

## 0. 背景

### 0.1 现状帧路径

```text
PreUpdate   input, retained routing, action dispatch
Update      app systems, style, overlays, …
PostUpdate  projection invalidation, synthesis, retained rebuild, IME
Last        paint_masonry_ui → AnimFrame? → redraw? → Vello encode → present
```

`WindowRuntime::paint_frame` 大致把下列布尔 **OR** 成一条路径：

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
| 纯动画 present ~30Hz 节流 | 拖影缓解 | **过渡手段**；正式架构落地后降级为可选/debug 或删除 |

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
| G1 | 四条时间线在代码与文档中可命名、可度量 | `FrameDriver` + `PICUS_FRAME_TIMING` 分 phase |
| G2 | 纯 Spinner 页：`encode_base` 均摊接近 0；动画只触达 anim 层 | timing + 可选 counter |
| G3 | Spinner 页静止窗口：动画视觉流畅（目标显示刷新贴近显示链路，不靠永久 30Hz 全局 throttle） | 人工 + 可选 PresentMon |
| G4 | Spinner 页拖窗：拖影明显轻于架构改造前；**默认产品路径不依赖永久砍动画 fps** | 人工对比 debug/release |
| G5 | Resize / 交互 redraw **永不**被「动画节流」挡住 | 单测 + 手工 |
| G6 | Idle（无 anim dirty）：几乎不 present | timing `painted` 比例 |
| G7 | Present 语义 = 最新帧；能力协商 Mailbox 优先 | surface 单测 + 运行时 debug 日志 |
| G8 | 应用公共 API 不变：`run_picus` / facade / 无 `__macro_support` | examples 编译与既有测试 |
| G9 | `AGENTS.md` + `docs/architecture/runtime.md` 与实现同步 | 文档 PR 与代码同栈 |
| G10 | 过渡 30Hz anim throttle 从默认路径移除或降为显式 opt-in | 代码审查 |

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
  max_frame_latency: 1
  drop_stale: true
  // 可选：EnterSizeMove 时 PreferFreshness 更激进
}
```

### 3.5 与 Bevy 的边界

- Bevy 仍是应用调度器；**不**把「每次 Update 醒来」等同「全窗 UI 帧」。  
- `RequestRedraw` 语义收敛为：有 **ContentPresent** 或 **AnimTick 需要调度** 时再写。  
- 可选后续：`NeedAnimTick` / `NeedContentPresent` 分消息（Phase 1b）。

---

## 4. 模块落点

| 模块 | 路径（建议） | 职责 |
|------|----------------|------|
| FrameDriver | `picus_core::runtime::frame_driver` | 每窗 `step(delta)`：tick → 决策 → encode → present |
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

| 工作项 | 细节 |
|--------|------|
| P0.1 | `PICUS_FRAME_TIMING` 扩展：`anim_tick_ms`、`encode_ms`（暂可整窗）、`present_ms`、`presented`/`anim_tick_only` 计数 |
| P0.2 | 文档：本计划 + `runtime.md` 增加「四条时间线」小节（链到本计划） |
| P0.3 | 将 `ANIM_PRESENT_MIN_INTERVAL` 标为 **transitional**：`// TODO(frame-pipeline): remove after anim layer`；或改为 `PICUS_ANIM_PRESENT_HZ` 环境变量 opt-in，默认关闭节流（**二选一在 P0 PR 定稿**，推荐 opt-in 以免默认假权衡） |
| P0.4 | 基线录制脚本/说明：gallery Button idle、Spinner 静止、Spinner 拖窗、debug/release PresentMon 可选 |

**验收**：G1 度量骨架；基线数字记入 PR 描述。  
**风险**：关掉默认 30Hz 后拖影回潮 → 用 P1/P2 顶上，不把节流当终局。

**建议 PR**：`PR0-metrics-docs-throttle-policy`

---

### Phase 1 — FrameDriver（单缓冲，全窗 encode 仍可保留）

**目标**：用显式调度替换 `needs_*` 大 OR；**不**要求立刻分层纹理。

| 工作项 | 细节 |
|--------|------|
| P1.1 | 引入 `FrameDriver` / `FrameStepResult`；`paint_masonry_ui` 只调 `driver.step` |
| P1.2 | 脏因枚举 + 从 Masonry 信号 / `incoming_redraw` / resize 填充 |
| P1.3 | 决策表：`do_anim_tick` / `do_rewrite` / `do_encode` / `do_present` 分离 |
| P1.4 | **硬规则**：`ResizeMetrics`、`InputOrRebuild`、`FirstPaint`、`RetrySurface` → 不得被 anim 节流跳过 present |
| P1.5 | `PresentPolicy` 从 surface 创建路径抽出；与 core 共享选择逻辑（已有 `select_present_mode`） |
| P1.6 | 单测：mock/轻量路径验证「仅 AnimTick 可不 present」「Resize 必 present」 |
| P1.7 | 删除或旁路「anim 与 present 绑死」的隐式假设注释 |

**验收**：G5、G7；代码路径可读；Spinner 仍可能全窗 encode，但调度语义正确。  
**风险**：回归漏画 → 保留 `has_painted_once` 与 Retry 路径测试。

**建议 PR**：`PR1-frame-driver`（可拆 1a driver、1b policy 若 diff 过大）

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
- Anim layer：独立 texture（尺寸策略：全窗透明 **或** spinner 包围盒 atlas——**首版推荐全窗透明 + 只画 anim widgets**，实现简单；二期再 atlas）  
- Present：`composite(base, anim_layers…) → swapchain`（可用现有 blitter 扩展）  
- Masonry：评估 `VisualLayerPlan` / overlay_layers 是否可映射；不够则 Picus 侧维护 `AnimLayerHost`  

#### 2.1 基础设施

| 工作项 | 细节 |
|--------|------|
| P2.1 | `LayerKind::{Base, Anim, Overlay}` + 每窗 `LayerSurfaces` |
| P2.2 | `picus_surface`：多 texture blit / 有序 composite present API |
| P2.3 | Dirty：`base_dirty` / `anim_dirty`；仅 dirty 层 encode |
| P2.4 | Timing：`encode_base_ms` / `encode_anim_ms` / `composite_ms` |
| P2.5 | Resize：所有层随 metrics 重建；FirstPaint 全层 |

#### 2.2 垂直切片：UiSpinner

| 工作项 | 细节 |
|--------|------|
| P2.6 | 投影/widget 路径标记 Spinner 为 Anim isolation（见 Phase 3 API，可先 hardcode） |
| P2.7 | Spinner 绘制只进入 anim 层 scene；base 不含 spinner 像素（或 base 留空位） |
| P2.8 | Spinner tick → 只 `anim_dirty`；base 不 rewrite encode（layout 未变时） |
| P2.9 | gallery Spinner 页 + 拖窗人工验收（G2–G4） |

#### 2.3 扩展

| 工作项 | 细节 |
|--------|------|
| P2.10 | Indeterminate `UiProgressBar` 迁 anim 层 |
| P2.11 | （可选）光标闪烁：评估 TextArea 是否适合 anim 层或保持 paint_only 低频 |

**验收**：G2、G3、G4、G10（默认不再需要 30Hz 全局 throttle）。  
**风险**：

- 命中测试 / 布局仍在 Masonry 树：anim 层仅**绘制隔离**，hit 仍走 retained 树  
- 透明度与 Mica：composite 预乘 alpha 与现有 blitter 一致性  
- 双层 encode 错误导致闪烁：对比测试截图（若已有 golden 则扩）

**建议 PR 栈**：

1. `PR2a-layer-surfaces-composite`  
2. `PR2b-spinner-anim-layer`  
3. `PR2c-progress-indeterminate`  
4. `PR2d-remove-default-anim-throttle`

---

### Phase 3 — 控件声明 API（PaintIsolation）

**目标**：高频动画**必须**声明隔离；默认控件走 base。

| 工作项 | 细节 |
|--------|------|
| P3.1 | `PaintIsolation::{Base, AnimLayer}`（名称可定）放在 widget 或 core 公共合适位置（facade 谨慎 re-export） |
| P3.2 | `UiSpinner` / indeterminate bar 默认 `AnimLayer` |
| P3.3 | 投影侧读取 isolation，分配 layer id |
| P3.4 | 文档：`docs/guide` 或 architecture：何时必须用 AnimLayer  
| P3.5 | AGENTS：一条硬规则——「持续 60Hz 视觉动画不得默认脏整窗 base present 路径」（措辞精炼） |

**验收**：新控件有明确约定；gallery 无 hardcode 特例（hardcode 从 P2 删掉）。  
**建议 PR**：`PR3-paint-isolation-api`

---

### Phase 4 — 脏矩形 / Anim atlas（可选增强）

**目标**：进一步降 anim encode 成本（全窗透明 anim 层仍可能偏贵时）。

| 工作项 | 细节 |
|--------|------|
| P4.1 | 收集 anim widget 的 layout bounds 并集为 dirty rect |
| P4.2 | 仅 dirty rect encode 或 atlas 子纹理  
| P4.3 | 与 scissor/blit 路径集成  
| P4.4 | 基准：多 Spinner / 大窗口下 `encode_anim_ms` |

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

### Phase 6 — 文档收尾与清理

| 工作项 | 细节 |
|--------|------|
| P6.1 | 重写 `docs/architecture/runtime.md` 帧阶段与层模型 |
| P6.2 | `docs/README.md` 已链本计划；examples 索引如有 Spinner 说明则更新 |
| P6.3 | 删除遗留 throttle / 过时注释 / 临时代码  
| P6.4 | 本计划状态改为「完成」并写进度摘要与前后对比数据 |

**建议 PR**：`PR6-docs-cleanup`（可与最后功能 PR 合并）

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
PR2b-spinner-anim-layer
    │
    ├──────────────► PR2c-progress-indeterminate
    │
    ▼
PR2d-remove-default-anim-throttle
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
| M1 语义 | P0+P1 | 帧调度可读；度量齐全 |
| M2 分层 | P2 | Spinner 拖窗明显改善；默认无需动画 throttle |
| M3 API | P3 | 控件约定稳定 |
| M4 增强 | P4/P5 | 大窗/重载更稳 |
| M5 收尾 | P6 | 文档与清理完成 |

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
| P0 | 未开始 |
| P1 | 未开始 |
| P1b | 未开始 |
| P2 | 未开始 |
| P3 | 未开始 |
| P4 | 可选·未开始 |
| P5 | 可选·未开始 |
| P6 | 未开始 |

（落地时在本表与文首状态更新。）

---

## 13. 参考

- 本会话架构讨论：四条时间线、假权衡、DWM 拖影  
- [architecture/runtime.md](../architecture/runtime.md)  
- [architecture/projection.md](../architecture/projection.md)（ECS 轴，正交）  
- 已合并 perf 相关提交：`19cb1a9`、`33164d2`、`54f0f91`  

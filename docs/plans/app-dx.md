# Picus 应用层 DX 完整计划

> **状态**：核心 DX 已落地（2026-07-14）
>
> **进度摘要**：§5.0 主路径 1–6 基本完成。公共面清理、`ctx.button` 迁移、sink 注入、`styled`/InlineStyle builder、主题契约与 FIFO/sink 隔离测试、trybuild 已落地。仍 open：widget/overlay 全部并入 dispatcher、资源 dirty 集成测试、嵌套 AGENTS、组合控件与部分 5.10 项。

---

## 0. 背景

Picus 内核（Bevy ECS + 投影 + 保留式 Masonry + Fluent 样式）自洽，但**应用作者路径偏重**：自定义 `UiComponentTemplate`、逐个 `register_*`、手写 `UiEventQueue::drain_actions`、主题与窗口配置入口分散。

目标：

- 用 **宏 + `AppPicusExt` + Bevy 原生消息** 压低样板  
- **所有 examples 全量迁移应用入口、注册和动作路径**；复杂例子只保留确有必要的低层投影/bridge  
- **不改变「缺失样式则不显示」的视觉契约**；主题允许只覆盖部分组件，主题配置入口与应用入口对齐  
- 复杂能力（自定义投影、overlay、流式 Markdown）仍完整保留  
- Picus 尚未公开发布，本计划完成时**删除过渡公共 API**，不为仓库内旧调用保留永久双轨  
- **文档分层**：`AGENTS.md` 只保留可执行契约与指针；架构/教程/参考进 `docs/`；README 做入口  

```text
┌──────────────────────────────────────────────────┐
│  应用层 DX                                         │
│  macros · AppPicusExt · theme · Message · 文档分层  │
│  样式糖 · 投影 helper · 组合控件 · 全 examples 迁移  │
└────────────────────────┬─────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────┐
│  内核                                              │
│  PicusPlugin · internal action queue · component template │
│  BSN · 样式/主题管线 · MasonryRuntime · 投影失效    │
└──────────────────────────────────────────────────┘

文档角色（目标态）：

  README.md     → 定位、安装、Quick start、链到 docs/
  AGENTS.md     → 强制规则、禁止项、作用域契约、解释文档指针
  docs/**       → 完整：架构、应用指南、样式、运行时、示例索引、宏参考
  rustdoc       → API 级细节（与 docs 概念文互补，不重复长教程）
```

---

## 1. 问题对照

| 摩擦 | 说明 |
|------|------|
| 双层描述 | BSN 实体树 + `project` 再拼 view |
| 显式注册 | 漏 `register_ui_component` / `register_projection_resource` 即不刷新 |
| 事件管道 | 每 app 手写 drain + match；与 Bevy `Message` 生态脱节 |
| 主题入口分散 | `load_style_sheet_ron` / `set_active_style_variant_by_name` 与窗口 runner 未统一 |
| 自定义区块膨胀 | 每区块 Component + template + 注册 + class |
| 示例口径不一 | 新读者不知「推荐路径」；迁移未覆盖全库 |
| 文档职责错位 | `AGENTS.md` 塞满业务/运行时叙述（对 agent 过重）；人类可读文档又不足；README Quick start 仍是旧 drain 模式 |

### 成功度量

| ID | 目标 |
|----|------|
| G1 | 业务动作只通过 `MessageReader<UiAction<T>>` 消费；应用不访问或 drain 内部队列 |
| G2 | 自定义组件用 derive 声明注册元数据，并通过一个显式批量清单注册；资源依赖不再另写注册链 |
| G3 | 主题**不会**在无配置时偷偷启用；`AppPicusExt` 提供清晰的主题加载/选 variant API |
| G4 | 全部 examples 的注册、业务动作和启动入口迁到唯一推荐 API；复杂投影本身可保留 |
| G5 | **文档分层完成**：`AGENTS.md` 瘦身；`docs/` 成为架构与教程权威源；与 DX 契约一致 |
| G6 | 减少无谓双层写作（helper、组合控件、文档指引） |
| G7 | 新人/agent 能从 README → docs → example 走通，而不必通读现行超长 `AGENTS.md` |
| G8 | facade 不再根级重导出整个 `picus_core`；旧 queue/drain/runner 入口从公共面移除 |

---

## 2. 非目标

- 改成 immediate-mode  
- 去掉 BSN / ECS / 投影失效契约  
- 重写 `picus_widget` 或废弃 Fluent RON 体系  
- **框架在无主题时自动选 dark/light**（保持「无主题 ≈ 无可见样式」）  
- 要求一个主题实现所有 Picus 组件；应用主题可以只实现实际使用的组件  
- 因为组件或属性缺少匹配样式而报错；只有 RON 语法、无效 token 类型等结构错误才报错  
- 新增独立 `examples/minimal` 作为「唯一正确示例」  
- 任意闭包挂在 Component 上（Send/生命周期问题）  
- 与 React/SwiftUI 语法级兼容  

---

## 3. 默认决策

| # | 议题 | 决定 |
|---|------|------|
| D1 | 无主题时的外观 | **什么都不显示**（透明/无填充）且不报错；想获得可见外观的应用显式加载 sheet 和/或选 variant。框架**不**默认 dark。 |
| D2 | 主题入口 | 只扩展 `AppPicusExt`：加载 RON、选 variant、可选 backdrop、窗口配置与 `run_picus` 使用同一条链；不新增同名 `picus::App` wrapper。 |
| D3 | 示例策略 | **不**单独做 minimal；先用 timer 验证完整新路径，再迁移全部 examples。复杂例可保留低层投影，但不得保留旧注册/动作/runner 路径。 |
| D4 | 业务动作 | retained 侧队列改为 crate-private 单消费者边界；应用只见 `UiAction<T>: Message`。不公开 drain，也不保留 queue/Message 双轨。 |
| D5 | 按钮负载 | ECS 侧挂非泛型 `UiEmit::new(T)`，内部以 `TypeId` 注册表分发为 `UiAction<T> { source, action }`；无 `UiEmit` 时才发 `BuiltinUiAction::Clicked`。 |
| D6 | 宏 | **必做但只保留显式批量注册策略**：derive 生成独立注册元数据 trait，`register_ui_components!` 是唯一常规收集入口；不使用 inventory/linkme。 |
| D7 | 闭包实体 | 不做。 |
| D8 | 文档 | **DX 必做部分**（见 §5.9）。`AGENTS.md` 不承载长篇业务说明；权威叙述在 `docs/`。 |
| D9 | 破坏性清理 | 发布前直接删除根级 `picus_core::*` 过渡重导出、公开 queue/drain、重复 runner 和旧注册教学；不走 deprecation 周期。 |

### 主题优先级（仅在有配置时）

1. 应用显式 `set_active_style_variant_by_name` / `AppPicusExt` 已选 variant  
2. 已加载 stylesheet 的 `default_variant`  
3. **无回退**——不自动 dark；未选 variant 则不应用 variant 规则（与现内核一致）

缺少主题、variant、组件规则或单个属性规则都不是配置错误。Picus 允许主题只实现应用实际使用的组件；未解析出的可见属性保持透明/空值。错误只用于无法解析的 RON、错误的值类型、无效 token 引用等结构问题，不能用“主题完整性”校验阻止部分主题运行。

### 样式使用分层（文档）

| 层级 | 用途 |
|------|------|
| 0 | 无主题 = 无可见默认装饰（契约） |
| 1 | 应用通过 `AppPicusExt` 加载 Fluent bundle 或 RON 并选 variant |
| 2 | Inline / builder 局部样式 |
| 3 | class + 应用 RON override |
| 4 | 完整多品牌 stylesheet |

---

## 4. 目标作者路径（相关能力落地后）

```rust
#[derive(Clone, Debug)]
enum AppAction {
    Inc,
    Dec,
}

#[derive(Resource, Default)]
struct Count(i32);

#[derive(Component, Clone, Default, UiComponent)]
#[ui_component(resources(Count))]
struct CountLabel;

impl UiComponentTemplate for CountLabel {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let n = ctx.world.resource::<Count>().0;
        // styled / classes 等糖
        Arc::new(label(format!("{n}")))
    }
}

fn on_app_action(
    mut reader: MessageReader<UiAction<AppAction>>,
    mut count: ResMut<Count>,
) {
    for UiAction { action, .. } in reader.read() {
        match action {
            AppAction::Inc => count.0 += 1,
            AppAction::Dec => count.0 -= 1,
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        UiRoot
        UiFlexColumn
        Children [
            CountLabel,
            (UiButton { label: { "+".into() } } template_value(UiEmit::new(AppAction::Inc))),
            (UiButton { label: { "-".into() } } template_value(UiEmit::new(AppAction::Dec))),
        ]
    });
}

fn main() -> Result<(), EventLoopError> {
    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .load_style_sheet_ron(include_str!("../assets/themes/app.ron"))
        .add_ui_action::<AppAction>()
        .add_systems(Startup, setup)
        .add_systems(Update, on_app_action);

    register_ui_components!(app, CountLabel);
    app.run_picus("Counter", BevyWindowOptions::default())
}
```

示意：业务侧用 **Bevy system + `MessageReader`**；资源依赖随组件元数据注册；应用没有 queue/drain 入口。`UiEmit` 使用 `template_value` 是因为它保存类型擦除 payload，不提供无意义的 `Default`。

---

## 5. 工作项清单

### 5.0 实施顺序与决策门

以下阶段有依赖关系；阶段验收未通过时，不迁移依赖其 API 的全部 examples 或用户文档：

1. **公共契约**：冻结 `UiAction<T>`、`UiEmit`、内部 dispatcher、`PicusUiSet` 和 facade 删除清单。
2. **组件注册**：实现独立注册元数据 trait、derive 与唯一的显式批量注册宏。
3. **应用入口**：完成 `AppPicusExt` 主题/窗口/`run_picus` API，并删除重复 runner。
4. **纵向验证**：只迁移 `timer`，完成 headless 点击 → Message → Resource 测试和一个真实窗口 smoke test。
5. **全量迁移与清理**：迁移其余 examples，删除旧公开 API、根级过渡重导出和旧示例代码。
6. **文档定稿**：README、guide、rustdoc 与分层后的 AGENTS 只描述已通过前述验收的最终 API。

每阶段可以拆 PR，但同一个 PR 不得同时引入仍未定型的 API 和将其写成稳定用户指南。

---

### 5.1 主题与应用入口设施

**契约（不可破）**

- [x] 文档与测试固定：**未加载主题 / 未选 variant 时，控件不出现框架默认可见填充与文字色**（「什么都不显示」）  
- [x] **删除或永不引入**「框架默认 dark」类行为  
- [x] 固定“部分主题”契约：缺少某组件或属性的规则不是错误，只有格式、类型、token 引用等结构问题报错  

**`AppPicusExt` / 入口（必做方向）**

- [x] 在 `AppPicusExt` 上统一并定名主题 API：`load_style_sheet`、`load_style_sheet_ron`、`style_variant`、`theme_backdrop`、`clear_theme_backdrop_override`  
- [x] `run_picus(title, BevyWindowOptions)` 是唯一推荐 runner；移除 `run_app` / `run_app_with_window_options`，不新增 `picus::App` wrapper  
- [x] 文档说明无主题或部分主题产生透明/空属性是正常结果，而不是加载失败；诊断日志只能提示当前没有匹配规则，不能升级为错误  
- [x] 示例一律**显式**加载主题（现状多数已有 RON；迁移时保持显式，不依赖隐式默认）

**非目标**

- 无配置时自动启用 Fluent dark/light  

---

### 5.2 控件消息与 Bevy `Message`（设计结论 + 工作项）

仓库已在 Bevy **0.19**（`MessageReader` / `MessageWriter`，见 accelerator 等）。应用侧应与之对齐。

#### 5.2.1 现状

| 层 | 机制 | 特点 |
|----|------|------|
| 控件 / 保留式回调 | `UiEventQueue`（`SegQueue` 类型擦除） | 可在非 Bevy system 上下文 lock-free 入队；typed drain 会消费匹配项 |
| 应用 | 手写 PreUpdate drain | 单消费者语义泄漏到应用，与 Bevy 调度、多监听者生态脱节 |
| 引擎输入 | Bevy `Message` | 标准 system 参数 |

#### 5.2.2 最终架构

1. **retained 边界仍使用队列，但队列完全内部化**  
   - 重命名为 `InternalUiEventQueue`（或等价），类型、条目和 drain API 均为 `pub(crate)`  
   - 队列/sink 归属单个 Bevy `App`；删除进程级全局 queue slot，`WindowRuntime` / `ViewCtx` 持有该 app sink 的 clone  
   - retained widget、Xilem task 和跨线程回调只负责写入；只有一个 Picus dispatcher 可以 drain  
   - 自定义 retained 回调通过只写的 `UiActionSender<T>` 发射，不获得队列句柄或消费能力；`ProjectionCtx::action_sender::<T>()` 返回可捕获的 clone  

2. **应用只见 `UiAction<T>`**  

   ```rust
   pub struct UiAction<T> {
       pub source: Entity,
       pub action: T,
   }

   impl<T: Send + Sync + 'static> Message for UiAction<T> {}
   ```

   payload `T` 不需要 derive `Message`。`add_ui_action::<T>()` 注册 `Messages<UiAction<T>>`、typed `UiActionSender<T>` resource 和一个 `TypeId` dispatcher；需要挂进 `UiEmit` 或 retained view 时再要求 `T: Clone + Send + Sync + 'static`。`ProjectionCtx::action_sender::<T>()` 会在未注册时给出明确诊断。

3. **单次 drain + `TypeId` 分发**  
   - `InternalUiEventQueue` 条目携带 `source` 与擦除 payload  
   - `UiActionRegistry` 保存 `TypeId -> fn(&mut World, Entity, &dyn Any)`；应用 payload handler downcast、clone 并写 `UiAction<T>`，内置 handler 可直接更新 ECS 状态并继续发 typed action  
   - dispatcher 是唯一消费者，并循环 drain/dispatch 直到本帧队列稳定；设置每帧动作上限以检测自触发循环  
   - 保持队列 FIFO：同一批按入队顺序处理，handler 新发动作追加到已有动作之后  
   - 未注册 payload 在 debug/test 明确失败、release 记录一次 error 后丢弃，绝不重新塞回队列造成永久积压  
   - Picus 内置动作及其 ECS mutation handler 由 `PicusPlugin` 自己注册，不要求应用注册，也不再由多个 system 分别 typed-drain  

4. **`UiEmit` 是非泛型 ECS 组件**  
   - `UiEmit::new(T)` 将 `T` 保存为 `Arc<dyn Any + Send + Sync>` 及其 `TypeId`，组件本身可 `Clone`，但不伪造 `Default`  
   - `UiButton` 投影时直接读取同实体的 `UiEmit`：存在则把擦除 payload 放进 retained button；不存在才使用 `BuiltinUiAction::Clicked`  
   - 因为投影阶段已完成选择，不需要枚举未知的 `UiEmit<T>` component，也不会同时发业务动作和 fallback Clicked  
   - BSN 使用 `template_value(UiEmit::new(...))`，符合 runtime-only、无默认模板值的既有契约  

5. **低层自定义投影**  
   - 用 `ctx.button(T, label)` / `ctx.button_with_child(T, child)` 取代 `button(entity, T, ...)`；helper 从 `ProjectionCtx` 获取 source 与已注册 sender  
   - 回调需要延迟发射时捕获 `UiActionSender<T>`；删除依赖进程级全局 queue 的公开 `emit_ui_action`  
   - 应用没有“绕过 Message 直接消费”的逃逸路径  

#### 5.2.3 调度契约

公开 `PicusUiSet`，并在 `PreUpdate` 中固定链：

```text
Input → RetainedRouting → DispatchActions
```

- Bevy 输入触发的按钮、文本、选择和 overlay 动作必须在同一帧 `Update` 开始前写入 `UiAction<T>`  
- 应用普通 `Update` system 无需额外 `.after(...)` 即可读取本帧输入动作  
- 在 `Update` 或更晚阶段由应用主动调用 sender 的动作定义为下一帧可见；文档和测试固定该边界  
- 将当前分散在 `PreUpdate` / `Update` 的 action-consuming 逻辑注册为 dispatcher handler，并在 `DispatchActions` 内运行；纯生命周期、样式和动画系统仍留在原阶段  

#### 5.2.4 工作项

- [x] 实现 `UiAction<T>`、`UiActionRegistry`、`add_ui_action::<T>()` 与单消费者 dispatcher  
- [x] 将现有队列、条目、typed drain 收为 `pub(crate)`，删除全局 queue slot，以 app-owned sink + `UiActionSender<T>` 替代公开全局 emitter  
- [x] 将 app-owned sink 注入每个 `WindowRuntime`（多窗口共享同一 app 队列；处理窗口前 reinstall；多个 App 互不串流）  
  - ViewCtx 仍经 thread-local 写入（picus_view 不依赖 action 类型）；语义上与 WindowRuntime 同 sink
- [x] 实现非泛型 `UiEmit::new(T)`，并让 `UiButton` 在投影阶段选择业务 payload 或 Builtin fallback  
- [x] 定义并配置 `PicusUiSet`，将 widget/overlay/action-consuming 逻辑迁到固定 PreUpdate dispatcher；增加每帧动作上限保护  
- [x] 指针命中、交互状态等高频内部事件继续使用内部专用处理，不自动提升为应用 Message  
- [x] 将所有应用代码从 drain 迁到 `MessageReader<UiAction<T>>`，随后删除 facade 中旧事件队列导出  

#### 5.2.5 验收

- 应用 system 仅依赖 `MessageReader<UiAction<T>>` 即可处理业务动作  
- 同一个 `UiAction<T>` 可被多个 MessageReader 读取，且每个 reader 恰好读一次  
- 有 `UiEmit` 时只发业务动作；无 `UiEmit` 时只发 `BuiltinUiAction::Clicked`；disabled 时二者都不发  
- 未注册 payload 不会留在内部队列，也不会静默污染后续帧  
- `picus` facade 无 `UiEventQueue`、`UiEvent`、`TypedUiEvent`、`drain_actions` 或全局 `emit_ui_action` 公共入口  

---

### 5.3 控件绑定与内置动作

- [x] `UiEmit::new(T)` 可通过 `template_value(...)` 挂入 BSN  
- [x] 语义：有 `UiEmit` → `UiAction<T>`；否则 `UiAction<BuiltinUiAction>`，其 payload 为 `Clicked`  
- [x] advanced 投影统一使用 `ctx.button(T, label)` / `ctx.button_with_child(...)`；删除旧的显式 entity helper 公开签名（`pub(crate)` 仅供内核）  
- [x] disabled 按钮不发射（与现行为一致）  

---

### 5.4 宏改造（**必做**，详细设计）

宏用于声明组件注册元数据和生成一个显式注册清单。建议新建 workspace crate：

```text
crates/picus_macros/   # proc-macro crate
picus / picus_core     # 重导出宏与配套 trait/注册表
```

#### 5.4.1 目标能力一览

| 宏 | 性质 | 解决的问题 |
|----|------|------------|
| `#[derive(UiComponent)]` | proc-macro derive | 生成独立的 Picus 注册元数据，不触碰 `UiComponentTemplate` impl |
| `register_ui_components!(app, A, B, C)` | macro_rules | 执行 derive 生成的注册元数据，作为唯一常规组件清单 |
| `classes!("a", "b")` | macro_rules | `StyleClass` 构造 |

不引入 `ProjectionResource`、`PicusMessage`、`picus_app!`、inventory 或 linkme。资源依赖属于读取它的 UI component；业务 payload 通过普通 `add_ui_action::<T>()` 注册；Bevy `App` 已经是 builder。

#### 5.4.2 `#[derive(UiComponent)]`

**输入约束**

- 目标类型已是 `Component`（可要求用户同时 `#[derive(Component)]`，或宏再展开 Component——**推荐用户显式 `Component`**，宏只做 Picus 侧）  
- 已实现 `UiComponentTemplate`（derive **不**生成 `project` 函数体；投影逻辑仍手写）  
- 默认要求 `Default + Clone`，与 BSN authoring 契约一致；明确的 runtime-only 类型可用 `#[ui_component(runtime_only)]` 跳过该断言  

**生成内容**

1. **独立注册 trait**  
   - derive 实现隐藏的 `picus::__macro_support::UiComponentRegistration`，不实现或扩展 `UiComponentTemplate`  
   - trait 的 `register(&mut App)` 调用内部注册入口；重复类型注册保持幂等  
   - `register_ui_components!(app, A, B)` 只展开为对该 trait 的两次调用  

2. **投影资源依赖**  
   - 属性可选：`#[ui_component(resources(Count, Draft))]`  
   - 依赖注册写在 `UiComponentRegistration::register` 内，不尝试生成 `UiComponentTemplate::register_projection_dependencies`  
   - 删除应用层 `register_projection_resources!`；确有动态/框架级依赖时使用 `picus::runtime::advanced` 的低层 API  

3. **类型名样式别名（若 StyleTypeRegistry 需要）**  
   - 可选 `#[ui_component(style_name = "todo.item")]` 注册选择器用名  

**示例展开（示意）**

```rust
#[derive(Component, Clone, Default, UiComponent)]
#[ui_component(resources(Count))]
struct CountLabel;

// 宏生成大致：
impl picus::__macro_support::UiComponentRegistration for CountLabel {
    fn register(app: &mut App) {
        picus::__macro_support::register_ui_component::<Self>(app);
        picus::__macro_support::register_projection_resource::<Count>(app);
    }
}
```

**测试**

- [x] trybuild：缺少 `UiComponentTemplate` 的清晰报错（与 Default/Clone 一并出现）  
- [x] trybuild：普通 authoring component 缺少 `Default` / `Clone` 的清晰报错；`runtime_only` 例外可通过  
- [x] 集成：只调用一次 `register_ui_components!(app, CountLabel)` 后投影、expand、selector alias 均可用  
- [ ] resources 属性确实触发 Resource 变更后的合成 dirty  

#### 5.4.3 `classes!` 与轻量 macro_rules

- [x] `classes!("todo.item", "selected")` → `StyleClass(vec![…])`  
- [x] 可放在 `picus` 的 `macro_rules`，不必进 proc-macro crate  
- [x] 与 BSN 字段 patch 兼容  

#### 5.4.4 批量注册宏

- [x] `register_ui_components!(app, A, B, C)`，`app` 必须是可变 `bevy_app::App` 绑定  
- [x] 展开只依赖 `UiComponentRegistration`，重复列出同一类型不重复安装 systems  
- [x] 不提供自动收集或第二套批量资源宏  

#### 5.4.5 工程与导出

- [x] 根 `Cargo.toml` members + workspace 依赖  
- [x] `picus::UiComponent` / `picus::classes` 重导出，应用只依赖 `picus`  
- [x] `picus_core` 不依赖 proc-macro；宏由 `picus` facade 重导出  
- [x] 生成代码只引用 `picus::__macro_support`，并用 `proc_macro_crate` 处理应用重命名依赖的情况；不得要求应用直接依赖或解析 `picus_core`  
- [x] `__macro_support` 标记 `#[doc(hidden)]`，只暴露宏展开所需的最小稳定面  
- [x] 文档：`docs` 一节「宏参考」+ 每个宏的失败模式  
- [x] `AGENTS.md`：新组件用 `#[derive(UiComponent)]` + 单一批量清单，禁止应用代码调用隐藏注册入口  

#### 5.4.6 宏验收

- [x] `timer` 去掉全部手写 component/resource 注册链，改用 derive + 一处批量清单  
- [x] 编译产物与依赖树不包含 inventory/linkme  
- [x] trybuild 挂在 `picus` facade 测试（`cargo test -p picus --test ui`）；macros crate 本身无运行时测试  
  - CI 可在后续流水线中加入该命令

#### 5.4.7 明确不在宏范围（初期）

- 不生成 `project` 函数体（UI 结构仍手写或 BSN）  
- 不生成完整 Bevy `Component` 反射全套（除非已有基础设施）  
- 不做 JSX 式 `view!` 宏（可列为更远期，不进本必做清单）  

---

### 5.5 样式 DX

- [x] `classes!`（见宏节）  
- [x] `styled(view, &resolved)` / `ctx.styled(view)`  
- [x] InlineStyle builder 补齐常用缺口（padding/gap/bg/text 等）  
- [x] 文档：无主题 = 无可见默认；样式层 0–4  

---

### 5.6 减少双层写作

- [x] 投影 helper（flex 列/行 + 子节点）：`ctx.flex_col` / `ctx.flex_row`  
- [x] `ProjectionCtx` 提供 action-aware button/sender helper，自动携带 source entity 并验证 payload 已注册  
- [x] 文档：何时不要拆 Component（`guide/app.md`）  
- [ ] 细粒度 vs 容器内 map 对照（todo 模式）  
- [ ] 组合控件（按 gallery 缺口）：`UiFormRow`、内容壳等  
- [x] 不做闭包 Component  
- [ ] **（远期）** 函数组件宏 `#[ui_view]` — 非 §5.4 必做核心  

---

### 5.7 应用入口与公共面收敛

**唯一应用入口**

- [x] 应用直接创建 `bevy_app::App`、安装 `PicusPlugin`，再使用 `AppPicusExt`；不新增 wrapper，也不需要 `into_inner()` 逃逸舱  
- [x] `AppPicusExt` 统一提供主题 API、`add_ui_action::<T>()`、窗口配置和消费 `App` 的 `run_picus(title, options)`  
- [x] `run_picus` 取代 `run_app` / `run_app_with_window_options`，窗口 callback 能表达的选项改为 typed `BevyWindowOptions` builder  
- [x] 无主题、空主题和部分主题都可运行；缺失规则保持透明/空属性，不默认选择 variant，也不做完整性报错  

**发布前破坏性清理**

- [x] 删除 `picus` 根的 `pub use picus_core::*` 与 `pub use picus_core as core`  
- [x] 根级只保留 derive/function-like 宏和经过选择的 prelude；普通 API 从 `app`、`components`、`projection`、`styling`、`events`、`overlay`、`runtime` 等分组模块导入  
- [x] `events` 不再导出 `UiEventQueue`、`UiEvent`、`TypedUiEvent`、`handle_widget_actions` 或全局 `emit_ui_action`；只导出 `UiAction<T>`、`UiActionSender<T>` 和应用需要的 typed action  
- [x] `projection` 导出 `ProjectionCtx` 的 action-aware helpers；删除旧的 `button(entity, T, ...)` / `button_with_child(entity, T, ...)` 根级与分组重导出  
  - 内部 runner `run_app*` 改为 `pub(crate)`，仅 `run_picus` 对外
- [x] 原始 projector/registry/手写注册入口移入 `picus::runtime::advanced` 或 `#[doc(hidden)] __macro_support`；常规指南不出现这些符号  
- [x] 应用 crate 只能依赖 facade `picus`；宏、examples 和 rustdoc 不直接引用 `picus_core`  
- [x] 因尚未公开发布，不增加 deprecated alias、compat feature 或旧 runner shim  

---

### 5.8 示例迁移（无单独 minimal）

**原则**

- **不新增** `examples/minimal` 作为特殊圣地  
- 先完整迁移 `timer` 并通过 §5.0 纵向验收，再迁移其余 examples  
- **所有** examples 的应用入口、组件注册和业务动作必须迁移；复杂例只允许保留深度自定义投影、bridge 等领域实现  

**清单（逐项勾选）**

| Example | 迁移期望 |
|---------|----------|
| [x] `timer` | 首个纵向样例：`UiAction` + 宏清单 + `run_picus` + 显式主题；去掉全部 drain |
| [x] `calculator` | 同上 |
| [x] `todo_list` | 同上；动态实体可保留 template |
| [x] `overlay_hit_routing` | Builtin/业务 `UiAction` 路径统一；自定义组件全部进入宏清单 |
| [x] `async_downloader` | 按钮/对话框迁 `UiAction`；异步逻辑保留 |
| [x] `game_2048` | 输入与按钮路径迁 `UiAction`；棋盘投影可保留 |
| [x] `chess_game` | 同上 |
| [x] `gallery` | 注册、动作、runner 全迁；复杂 demo page 投影可保留 |
| [x] `picuscode` | 注册、动作、runner 全迁；CodeWhale bridge 与流式 Markdown 保留 |
| [x] `shared_utils` | 仅当有 UI 样板 helper 时跟着改 |

**每例验收**

- 显式主题（RON 或等价），无「靠框架默认主题」  
- 业务交互全部使用 `MessageReader<UiAction<T>>`，example 代码不可访问内部队列  
- 自定义 `UiComponent` 全部 derive，并在一处 `register_ui_components!` 清单注册  
- 使用 `AppPicusExt::run_picus`，不调用已删除的 runner  

**文档**

- [x] README / examples 说明：以 **timer 或 calculator** 等真实例为入门，不设 parallel minimal  
- [x] 复杂例 README 只说明仍在使用的 advanced 投影/bridge，不将旧应用 API 描述为迁移状态  

---

### 5.9 文档体系改造（DX 必做）

#### 5.9.1 问题

现行 `AGENTS.md`（约 500+ 行）同时承担：

- agent/人的**强制流程与契约**（合适）  
- 仓库布局、CodeWhale 同步、运行时阶段、样式细节、控件清单、Markdown 实现等**业务与架构说明**（对 AGENTS 过重）  

结果：

| 读者 | 体验 |
|------|------|
| Agent | 上下文噪声大；真正「必须遵守」的规则被淹没 |
| 人类学框架 | 没有系统的 `docs/` 教程/架构文；只能啃 AGENTS 或翻代码 |
| README | Quick start 仍是旧式 drain + 手写 template，与目标 DX 脱节 |

文档改造与 API DX **同等重要**：没有正确文档，宏/Message 不会成为默认路径。

#### 5.9.2 分层原则

| 载体 | 应包含 | 不应包含 |
|------|--------|----------|
| **根/嵌套 `AGENTS.md`** | 可执行规则、禁止项、跨模块公共契约；复杂子系统在就近目录保留完整硬约束；链到解释文档 | 教程、产品 walkthrough、控件百科、token 参考表 |
| **`docs/**`** | 架构解释、应用指南、子系统深潜、示例索引、宏参考、主题契约 | 已删除 API 的用户迁移指南；与 rustdoc 重复的签名堆砌 |
| **`README.md`** | 一句话定位、安装、**与目标 DX 一致的** Quick start、文档地图、examples 入口 | 完整架构书 |
| **rustdoc (`picus`)** | 类型/函数契约、简短示例 | 跨 crate 系统设计长文 |
| **`docs/plans/*`** | 进行中的计划（本文）；完成后可归档或链到已落地 docs | 当作用户手册 |

**单一事实来源**：解释与示例只在 docs 写全；必须自动执行的硬约束在适用范围的 AGENTS 写成自足规则，并链接解释。两者职责不同，不以删除硬约束换取机械缩短。

#### 5.9.3 目标目录结构（建议）

```text
docs/
  README.md                 # 文档地图（索引）
  architecture/
    overview.md             # Bevy + Masonry + 投影总览
    crates.md               # crate 边界与依赖方向
    runtime.md              # 调度阶段、窗口 runtime、绘制
    input-ime-hit.md        # 输入 / IME / hit testing
    projection.md           # 合成、失效、UiProjectorRegistry
  guide/
    app.md                  # 应用怎么写：AppPicusExt、主题、UiAction、BSN
    components-bsn.md       # BSN、Default+Clone 契约、内置控件
    styling-themes.md       # 无主题不显示、RON、variant、token、backdrop
    events-messages.md      # UiAction、UiActionSender、UiEmit 与调度契约
    macros.md               # derive / classes! / 注册
    overlays-scroll.md
    markdown-streaming.md
    i18n-fonts-icons.md
    multi-window.md
    testing.md
  examples/
    index.md                # 各 example 教什么、使用哪些 advanced 能力
  reference/
    public-modules.md       # picus facade 模块地图
    style-tokens.md         # 需要时从 AGENTS/主题拆出的参考表
  contributing/
    codewhale-submodule.md  # 从 AGENTS §1.1–1.2 迁出的完整流程
  plans/
    app-dx.md               # 本计划
```

实现时可合并邻近文件，但**主题归属**应覆盖上表；名称可微调。

#### 5.9.4 从现行 `AGENTS.md` 迁出的内容映射

| 现行 AGENTS 区块（约） | 迁往 | 根 `AGENTS.md` 仅保留 |
|------------------------|------|------------------------|
| §1 Workspace 长描述 / crate 列表 | `docs/architecture/crates.md` | 一句：依赖 `picus` facade；禁止 reintroduce 上游 masonry/xilem app crate；链到 crates.md |
| §1.1 picuscode / CodeWhale 产品与 bridge | `examples/picuscode/README.md` 与/或 `docs/examples/` | 一句：picuscode 为集成例；测试勿碰用户 `~/.codewhale/` |
| §1.2 同步 CodeWhale submodule | `docs/contributing/codewhale-submodule.md` | 根保留先读要求；具体强制步骤放 `thirdparty/AGENTS.md`，避免修改 submodule 内文件 |
| §2 Runtime Architecture | `docs/architecture/runtime.md` | 保留所有跨模块运行时硬不变量；局部 surface 规则可移入 `crates/picus_surface/AGENTS.md` |
| §3 Input / IME / Hit | `docs/architecture/input-ime-hit.md` | 保留影响实现正确性的输入源、坐标和事件顺序契约 |
| §4 ECS UI Model / 控件清单 | `docs/guide/components-bsn.md` + rustdoc | BSN：`Default+Clone` 公共组件契约一条 |
| §5 BSN 迁移规则 | `docs/guide/components-bsn.md` | 同上契约 + 链 |
| §6 Synthesis / Events | `docs/architecture/projection.md` + `docs/guide/events-messages.md` | 投影依赖随 derive 元数据注册；应用只读 `UiAction<T>`；内部队列不得公开 |
| §7 Styling | `docs/guide/styling-themes.md` | **缺失样式不显示且不报错**；允许部分主题；生产色来自 RON 非 widget 默认；链 |
| §8 Scroll / Overlay | `docs/guide/overlays-scroll.md` | 根保留跨 overlay/scroll 的命中与路由不变量；局部细节可放 `crates/picus_core/AGENTS.md` |
| §8.1 Markdown | `docs/guide/markdown-streaming.md` | 缓存与流式追加硬约束移入 `crates/picus_core/AGENTS.md` |
| §9 Assets / i18n / icons | `docs/guide/i18n-fonts-icons.md` | 跨窗口字体广播等运行时硬约束保留在根或 core 嵌套 AGENTS |
| §10 Surface | `docs/architecture/runtime.md` 或 `surface.md` | surface 错误处理与 present 不变量移入 `crates/picus_surface/AGENTS.md` |
| §11 Plugin helpers | `docs/guide/app.md` | 根保留唯一 `AppPicusExt` / `run_picus` 应用入口契约 |

#### 5.9.5 瘦身后的 `AGENTS.md` 建议结构

不设行数 KPI。删除可被 docs/rustdoc 替代的叙述，但以“agent 不打开额外文件也能遵守当前作用域硬约束”为验收标准。对子系统专属且篇幅较长的规则，新增就近的嵌套 `AGENTS.md`，而不是只留下一个普通 docs 链接。

```markdown
# AGENTS.md
## 角色与范围          # 本文件是强制规则，不是教程
## 依赖与边界          # picus facade；禁止项；链 architecture/crates
## 应用默认写法        # UiAction + 宏清单 + 显式主题 + run_picus；链 guide/app
## 必须遵守的契约      # BSN；投影依赖；部分主题；运行时/输入/样式跨域不变量
## 禁止项              # 列表短而硬
## 文档地图            # 链 docs/README.md
## 子模块 / 特殊流程   # 仅链 codewhale-submodule、picuscode 测试路径注意
## 改本文的规则        # 契约变更同步更新可执行规则与 docs 解释
```

- [x] 按上表拆分并落地链接  
- [x] 删除 AGENTS 中已迁走的长文，避免双份  
- [ ] 在 `crates/picus_core`、`crates/picus_surface`、`examples/picuscode`、`thirdparty` 等确有局部硬规则的目录按需增加嵌套 `AGENTS.md`；不要为此修改 CodeWhale submodule 内容  
- [x] 系统/工具只注入适用 AGENTS 时，仍能获得完成该目录任务所需的全部硬约束；docs 链接用于解释而非补全缺失规则

#### 5.9.6 README 改造

- [x] Quick start **与目标 DX 一致**（显式主题、`UiAction`、宏/BSN 内置控件、`run_picus`；不出现已删除 queue/drain）  
- [x] 文档地图：Architecture / App guide / Styling / Examples  
- [x] examples 表：各例用途 + 链 `docs/examples/index.md`  
- [x] 不记录面向未发布旧 API 的迁移步骤；README 只展示最终公共面  

#### 5.9.7 与 DX 其它工作项的文档交付绑定

§5.0 的纵向验证冻结契约后，每项 API/行为改动都要**同步**更新对应 docs（禁止只改代码）。冻结前只更新本计划和实现级设计说明，不把试验 API 写成用户稳定入口：

| 能力 | 文档落点 |
|------|----------|
| 主题契约 / `AppPicusExt` 主题 API | `guide/styling-themes.md` + `guide/app.md` |
| `UiAction` / `UiActionSender` / `UiEmit` | `guide/events-messages.md` |
| 宏 | `guide/macros.md` |
| `run_picus` 与窗口入口 | `guide/app.md` |
| 双层写作 / helper | `guide/components-bsn.md` 或 `guide/app.md` |
| example 教学范围 / advanced 用法 | `examples/index.md` + 各 example 短 README（可选） |

- [x] 约定：合并 DX 相关改动时 checklist 含「docs 已更新」  

#### 5.9.8 rustdoc 与 `docs/` 分工

- [x] `picus` facade 模块级 rustdoc：一句话 + 指向 guide 章节名  
- [x] 长教程不进 rustdoc  
- [x] 公共契约类型（`UiComponentTemplate`、`UiAction<T>`、`UiEmit`）与 guide 表述一致；rustdoc 不暴露内部队列  

#### 5.9.9 文档验收

- [x] 根 AGENTS 无教程/控件百科；跨域硬规则完整，子系统硬规则在适用目录的嵌套 AGENTS 中可直接执行  
- [x] `docs/README.md` 可当目录覆盖主路径  
- [x] 新人只读 README + `guide/app.md` + 一个已迁移 example 能写简单应用  
- [x] Agent 硬规则可从当前作用域 AGENTS 直接执行；docs 提供原理和示例  
- [x] 部分主题 / `UiAction` / 宏清单等契约在 docs 与 AGENTS 中**一致**  
- [x] 旧 README Quick start 不再教过时主路径  

#### 5.9.10 文档工作项勾选汇总

- [x] 建立 `docs/` 地图与目录骨架（§5.9.3）  
- [x] 从 `AGENTS.md` 按映射表迁出内容并改写为文档体裁（§5.9.4）  
- [x] 瘦身 `AGENTS.md`（§5.9.5）  
- [x] 重写 README 入口与 Quick start（§5.9.6）  
- [x] `guide/app.md` / `events-messages.md` / `styling-themes.md` / `macros.md` 等与代码同步  
- [x] `docs/examples/index.md` + 全 example 教学范围与 advanced 用法  
- [x] `contributing/codewhale-submodule.md`  
- [ ] rustdoc 交叉链接策略（§5.9.8）  
- [x] DX 改动的 docs checklist 约定（§5.9.7）  
- [ ] i18n / a11y / 多窗口 / 性能指引落在对应 guide（非堆回 AGENTS）  

---

### 5.10 测试与调试

- [x] 主题契约测试：无 sheet/无 variant → 无「框架塞入的可见默认色」  
- [x] 部分主题测试：只实现一个组件或部分属性可正常加载；未覆盖组件保持透明；无效 RON/类型/token 仍报错  
- [x] 输入动作调度：retained 入队 → PreUpdate 单消费者 drain/dispatch → 同帧 Update 的 reader 收到  
- [x] 两个独立 `MessageReader<UiAction<T>>` 均恰好收到同一动作一次  
- [x] 两个 Bevy `App` 实例的 action sink 互不串流；同一 App 的多个 window 共享同一 app sink  
- [ ] `UiEmit` 业务动作、Builtin fallback、disabled 三条互斥路径（disabled 已有投影测试；UiEmit 分发路径仍可补 headless 点击）  
- [ ] 未注册 payload 的 debug/test 失败与 release 丢弃日志；队列不会跨帧积压该 payload  
- [x] FIFO 与每帧动作上限测试：handler 派生动作排在已有动作之后，自触发循环能被确定性中止  
- [x] macros trybuild + 注册幂等（trybuild 覆盖 Default/Clone/runtime_only；幂等随 register 实现）  
- [ ] 扩展 headless helpers：click → `UiAction` → resource，作为 `timer` 纵向验收  
- [ ] facade compile test：应用只依赖 `picus`；根级 core 重导出、公开 queue/drain 和旧 runner 不可用  
- [ ] **（可选）** dirty 原因 debug 输出  
- [x] 全 examples：逐个运行 `cargo check -p <example-package>`；关键例保留主题 parse 与交互测试  

---

### 5.11 脚手架与工具

- [ ] **（可选）** `picus new` / cargo-generate：生成**带显式主题 RON** 的应用骨架（不是「无主题 Hello」）  
- [x] 或文档：复制已迁移的 timer/calculator 骨架  

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 擦除 payload 的 TypeId 未注册或 downcast 不一致 | 单一 `UiActionRegistry` 同时记录 TypeId 与 typed dispatcher；debug/test fail-fast，release 记录并丢弃 |
| 同帧顺序被后续系统改动破坏 | 公开 `PicusUiSet` 并 chain；headless 测试固定点击在同帧 Update 可见 |
| 显式组件清单漏项 | 所有自定义组件集中在一处宏调用；timer 先验证；debug 合成诊断报告存在逻辑组件但无 projector 的实体 |
| 全 examples 迁移量大 | 先纵向迁移 timer，API 冻结后逐例迁移；不以保留旧公共 API 降低迁移量 |
| 误加「默认 dark」 | §5.1 契约测试锁死 |
| 部分主题被误判为不完整配置 | 测试“只覆盖一个组件”的合法主题；仅结构错误返回失败 |
| 宏生成代码难调试 | 独立注册 trait 保持展开简单；trybuild 覆盖错误；文档提供 expand 方法 |
| facade 清理后内部 API 再次外泄 | facade compile test + grouped module allowlist；宏仅走隐藏 support 模块 |
| AGENTS 与 docs 漂移 | AGENTS 只写可执行规则、docs 写解释；契约变更同时更新二者并由目录级 AGENTS 限定作用域 |

---

## 7. 变更记录

| 日期 | 摘要 |
|------|------|
| 2026-07-14 | 初稿 |
| 2026-07-14 | 去轮次，扁平清单 |
| 2026-07-14 | 按反馈修订：(1) 取消独立 minimal，全量/部分迁移全部 examples；(2) 取消默认主题，保持无主题不显示，主题入口对齐 builder；(3) 业务消息与 Bevy `Message` 结合的架构结论与工作项；(4) 宏改为必做并写详细设计 |
| 2026-07-14 | 文档纳入 DX 必做：AGENTS 过重/docs 过少的职责拆分、目标目录、内容映射、瘦身结构、README/rustdoc 分工与验收 |
| 2026-07-14 | 架构收敛修订：内部队列单消费者 + `UiAction<T>`；非泛型 `UiEmit`；显式宏清单；唯一 `AppPicusExt`/`run_picus` 入口；删除过渡公共面；增加实施决策门与目录级 AGENTS；明确部分主题缺失样式合法且不报错 |
| 2026-07-14 | 核心 DX 落地并勾选已完成项：`UiAction`/`UiEmit`/`run_picus`/`picus_macros`；全 examples 迁 runner/动作路径；AGENTS 瘦身 + docs 主路径；未勾选项为后续收敛（trybuild、完全内化 widget drain、删除 `emit_ui_action`/`run_app` 残留、`styled` helper、嵌套 AGENTS 等） |
| 2026-07-14 | 收敛公共面：删除 facade `core`/`emit_ui_action`/自由 `button` 导出；examples 迁 `ctx.button`+`UiActionSender`；`WindowRuntime` 持有 app sink；`styled`/InlineStyle builder/flex helper；主题与 FIFO/sink 隔离测试；trybuild 挂 `picus` |

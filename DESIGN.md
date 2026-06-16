# CUI 框架设计文档

## 一、设计哲学

CUI（Context UI）是一个面向 LLM Agent 的上下文 UI 框架。它将 LLM 的上下文窗口视为渲染画布，借鉴 React/Dioxus 的组件模型，提供声明式、可交互的组件树。

**核心理念**：

- **组件即上下文** — 每个组件渲染为紧凑 Markdown，LLM 通过 `component_action` 工具与之交互
- **容量自适应** — 上下文窗口有 token 上限，框架根据容量自动降级/升级组件展示粒度
- **数据与渲染分离** — ComponentTree 是纯数据模型，Context 持有渲染状态机和管线
- **类型驱动** — 声明 `type: tool` 即可获得默认行为，减少样板代码

## 二、核心数据流

```
外部调用 → Context（核心 API）
              ├── tick: u64          # 渲染计时
              ├── cycle: RenderCycle # 渲染状态机
              ├── tree: ComponentTree # 组件数据模型
              ├── dialogue           # 对话管理
              └── event_bus          # 事件总线

Context::render() 管线：
  1. cycle → Preparing
  2. tree.prepare(budget, tick) → 容量规划 + 可见性评估
  3. cycle → Rendering
  4. tree.render_plan(plan, tick) → 生成输出字符串
  5. tree.commit() → 清理副作用 / 推进 recent_ticks
  6. tick += 1，清理过期 temp_expand
  7. cycle → Idle
```

**关键设计**：虚拟渲染（`render_volatile`）走 prepare → render_plan → abort，不推进 tick，不影响组件状态。

## 三、组件系统

### 3.1 BaseComponent trait

所有组件的核心接口：

```rust
pub trait BaseComponent: Send {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn priority(&self) -> PriorityLevel;
    fn render(&self, level: RenderLevel) -> String;
    fn handle_action(&mut self, action: &str, params: &str) -> ActionResult;

    // 可选方法
    fn visibility_condition(&self) -> VisibilityCondition { Always }
    fn is_static(&self) -> bool { false }
    fn action_variants(&self) -> &'static [ActionVariant] { &[] }
    // ...
}
```

### 3.2 ComponentNode

统一节点类型，承载 BaseComponent 实例和元数据：

- `Leaf` — 叶节点，包装单个 BaseComponent
- `Composite` — 复合节点，可包含子节点（类似 HTML div）

节点持有 `level`（当前渲染级别）、`dirty` 标记、`lifecycle` 钩子等状态。

### 3.3 ComponentTree

纯数据模型，存储：

- `roots` — 根组件列表
- `global_state` — 全局键值状态
- `component_state` — 按组件 ID 命名空间的状态
- `temp_expand` — 临时展开标记（id + expires_at）
- `triggered` — 已触发事件集合
- `recent` / `recent_ticks` — 操作记录（保留 3 tick 后清理）
- `active_conditions` — 当前活跃条件集（内部使用）

**不与 tick/渲染状态机耦合** — 这些由 Context 管理。

### 3.4 Context

CUI 框架的统一入口和核心数据结构：

- 持有 `ComponentTree`（数据）、`tick`（计时）、`RenderCycle`（渲染状态机）
- 提供 `register`、`remove`、`write`、`render`、`component_action` 等 API
- 管理对话消息缓冲、事件总线、处理器注册表
- 对外暴露 `ComponentStore`、`Renderer`、`ActionDispatcher` trait 用于 mock 测试

## 四、渲染级别

```rust
pub enum RenderLevel {
    Hidden = 0,   // 完全隐藏
    Title = 1,    // 仅标题
    Summary = 2,  // 一句话摘要
    Standard = 3, // 标准渲染（默认）
    Detailed = 4, // 完整详情
}
```

级别支持 `degrade()` / `upgrade()` 操作，有下界（Hidden）和上界（Detailed）。

## 五、容量规划

基于 token 预算的迭代算法（`system/capacity.rs`）：

1. 每个组件按其 `PriorityLevel` 获得保底级别
2. 超预算 → 降级低优先级组件
3. 有剩余 → 升级高优先级组件（Critical 享有 0.5 偏置因子）
4. 热组件（`heat > 0`，最近交互过）在降级阶段获得等效优先级提升

预算单位是 token（通过 `tokenizer::estimate` 估算）。

## 六、可见性条件

```rust
pub enum VisibilityCondition {
    Always,              // 始终可见
    When(String),        // 条件匹配时可见
    OnTrigger(String),   // 外部事件触发后可见
}
```

渲染期条件过滤 API（chain 风格，渲染后自动清除）：

```rust
ctx.render()                                    // 无条件，仅 Always
ctx.in_condition("plan").render()               // When("plan") + Always
ctx.in_condition("plan").and("act").render()    // OR 逻辑
ctx.in_condition("plan").with_budget(800).render()

ctx.trigger("config_changed");                  // OnTrigger 事件
ctx.render();                                   // OnTrigger 组件当次可见
```

## 七、Tick 系统

`tick` 是 CUI 的基本时间单位，与一次实际渲染绑定：

- 每次 `Context::render()` 推进 tick
- 虚拟渲染（`render_volatile`）不推进 tick
- `temp_expand` 使用绝对 tick 作为过期时间：`expires_at = current_tick + duration`
- 每次渲染后自动清理过期 temp_expand（`tick >= expires_at`）
- `recent` 操作记录每 3 tick 清空一次，帮助 AI 记住近期操作

## 八、内置组件

| 组件 | 说明 |
|------|------|
| `TextBlock` | 文本块，支持 static 模式（始终 Summary+） |
| `ConditionalBlock` | 条件渲染容器 |
| `ListBlock` | 列表容器 |
| `GroupComponent` | 分组容器（可折叠） |
| `Label` | 只读标签 |
| `Body` | 可变正文字段 |
| `Button` | 按钮，处理 AI 动作 |
| `DataSlot` | 数据槽位（支持 Overwrite/Append/Clear） |
| `Toast` | 临时通知，自动消失 |
| `CuiFileLeaf` | .cui 文件叶节点 |

## 九、适配器系统

`.cui` 文件是 YAML frontmatter + Markdown body 的声明式模板：

```yaml
---
type: tool
id: bash
title: Bash 执行
priority: high
when: act
---
执行 Shell 命令并返回结果。
```

编译管道将其解析为 `ComponentNode`，类型注册表根据 `type` 字段提供默认行为。

## 十、输出格式

渲染输出为紧凑 Markdown 格式，title 即组件标题：

```text
## [工作流引擎]
当前正在执行 Agent 工作流。

## [规划方案]
分析用户意图并生成执行计划。

`[展开]` `[折叠]`

## [_recent]                    ← 近 3 轮操作记录
  ·[Bash 执行] expand ✓

## [_overview]                  ← 隐藏组件列表
  `bash` `review` `[expand_hidden]`
```

- `## [title]` — 用 title 做标题，id 保留给 `component_action` 引用
- `## [title] ●` — dirty 标记，组件有未消费的数据变更
- `` `[action]` `` — 内联代码格式的可交互动作
- `[_recent]` — 近期操作记录，每 3 tick 清空
- `[_overview]` — 一行紧凑列出所有隐藏组件 ID
- AI 通过 `component_action(component_id, action, params)` 与组件交互

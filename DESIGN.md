# CUI 框架设计文档

## 一、设计哲学

CUI（Context UI）是一个面向 LLM Agent 的上下文 UI 框架。它将 LLM 的上下文窗口视为渲染画布，借鉴 React/Dioxus 的组件模型，提供声明式、可交互的组件树。

**核心理念**：

- **组件即上下文** — 每个组件渲染为紧凑 Markdown，LLM 通过 `component_action` 工具与之交互
- **容量自适应** — 上下文窗口有 token 上限，框架根据容量自动降级/升级组件展示粒度
- **数据与渲染分离** — ComponentTree 是纯数据模型，Context 持有渲染状态机和管线
- **类型驱动** — 声明 `type: tool` 即可获得默认行为，减少样板代码
- **数据输入统一** — 通过 `inputs:` / `{{input:name}}` / `with_input()` 单一机制注入运行时数据

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

`render_volatile` 走 prepare → render_plan → abort，不推进 tick。

## 三、组件系统

### 3.1 BaseComponent trait

```rust
pub trait BaseComponent: Send {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn priority(&self) -> PriorityLevel;
    fn render(&self, level: RenderLevel) -> String;
    fn handle_action(&mut self, action: &str, params: &str) -> ActionResult;

    fn visibility_condition(&self) -> VisibilityCondition { Always }
    fn is_static(&self) -> bool { false }
    fn is_pinned(&self) -> bool { false }
    fn action_variants(&self) -> &'static [ActionVariant] { &[] }
    fn estimated_tokens(&self, level: RenderLevel) -> usize;
    fn write(&mut self, mode: DataMode, data: &str) {}
}
```

### 3.2 ComponentNode

- `Leaf` — 叶节点，包装单个 BaseComponent
- `Composite` — 复合节点，可包含子节点

节点持有 `level`、`collapsible/collapsed`、`pinned`、`lifecycle` 等状态。

### 3.3 ComponentTree

纯数据模型，存储 roots、global_state、component_state、temp_expand、triggered、recent、active_conditions。

**不与 tick/渲染状态机耦合** — 这些由 Context 管理。

## 四、渲染级别

```
Hidden (0) → Title (1) → Summary (2) → Standard (3) → Detailed (4)
```

级别支持 `degrade()` / `upgrade()`。pinned 组件保底 Standard。

## 五、容量规划

基于 token 预算的迭代算法：

1. 每个组件按 `PriorityLevel` 获得保底级别（pinned → Standard，Critical/High → Summary，Normal → Title，Low/Minimal → Hidden）
2. 超预算 → 降级低优先级组件（跳过 pinned）
3. 有剩余 → 升级高优先级组件（pinned 优先）
4. 热组件（`heat > 0`）在降级阶段获得等效优先级提升

## 六、可见性条件

```rust
pub enum VisibilityCondition { Always, When(String), OnTrigger(String) }
```

```rust
ctx.render()
ctx.in_condition("plan").render()
ctx.in_condition("plan").and("act").render()   // OR
ctx.in_condition("plan").with_budget(800).render()
ctx.trigger("config_changed");
ctx.render();
```

## 七、数据输入

```yaml
# .cui 文件
inputs:
  - {name: branch, default_value: "main"}
  - {name: target, required: true, description: "构建目标"}
---
分支: {{input:branch}}
目标: {{input:target}}
```

```rust
// Rust API
CuiFileLeaf::new("task", "任务", body)
    .with_input("branch", "feat/login")
    .with_input("target", "x86_64")
    .build();
```

`{{input:name}}` 在 `fill_slots` 中单次扫描 O(L) 替换，未匹配占位符静默清空。

## 八、工具与技能

```rust
Cui::init()
    .type_registry(my_types)
    .tools("tools", "可用工具", PriorityLevel::High, (
        "tools/read_file.cui",
        "tools/run_test.cui",
    ))
    .skills("skills", "技能参考", (
        ("Rust 审查", "检查 unsafe 块"),
        ("安全审计", "扫描 SQL 注入"),
    ))
    .handlers(&registry)
    .build();
```

- `tools()` 接受 1–12 元组或 `Vec<String>`，打包为可折叠分组
- `skills()` 接受 1–12 元组，合并为惰性参考列表（`priority: Low + inert`）
- 工具通过 `resolve_tool()` 统一入口解析：类型注册表 → 动作合并 → 节点构建

## 九、动作处理器

```rust
pub trait ActionHandler: Send + Sync {
    fn execute(&self, params: &str, ctx: &mut dyn ActionContext)
        -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>>;
    fn id(&self) -> &str { "" }
}
```

错误类型使用 `Box<dyn std::error::Error>`，兼容 anyhow、thiserror 及手动 Error impl。

## 十、用户覆盖

```rust
Cui::init()
    .load_dir("cui/")
    .with_user_overrides_from("~/.cui/user/")
    .build();
```

用户 `.cui` 文件按 `id` 匹配开发者组件，覆盖 `title`、`body`、`inputs`，设置 `pinned: true` 可固定组件不被预算降级。

## 十一、内置组件

| 组件 | 说明 |
|------|------|
| `TextBlock` | 文本块 |
| `GroupComponent` | 分组容器（可折叠） |
| `Label` | 只读标签 |
| `Body` | 可变正文字段 |
| `Button` | 交互按钮 |
| `DataSlot` | 数据槽位（Overwrite/Append/Clear） |
| `Toast` | 临时通知 |
| `CuiFileLeaf` | .cui 文件叶节点 |

## 十二、输出格式

```
## [title]          ← 组件标题
  body              ← Markdown 正文
  `[action1]` `[action2]`   ← 可交互动作
## [title] ●        ← dirty 标记
## [_recent]         ← 近 3 轮操作记录
## [_overview]       ← 隐藏组件列表
```

AI 通过 `component_action(component_id, action, params)` 与组件交互。

## 十三、API 设计原则（进行中）

### 当前问题

部分 API 使用 `method_x_y()` 命名，未利用 Rust 的类型系统将相关操作分组：

```rust
// 当前 —— 平铺在 Context 上
ctx.push_message(msg);
ctx.read_messages();
ctx.scroll_dialogue(pos);
ctx.scroll_dialogue_by_cycles(n);
ctx.expand_cold_zone(start, end);
ctx.close_cold_zone();
ctx.collect_persistable();
ctx.last_render_stats();
```

### 目标模式

将相关操作收敛到子访问器，通过类型系统组织：

```rust
// 目标 —— 按语义分组到子句柄
ctx.dialogue_mut().push(msg);
ctx.dialogue_mut().read();
ctx.dialogue_mut().scroll(pos).cycles(n);
ctx.dialogue_mut().expand_cold(start, end).close();

ctx.persistence().collect();
ctx.render().stats();
```

### 变更日志

| 旧 API | 新 API |
|--------|--------|
| `ctx.push_message(msg)` | `ctx.dialogue_mut().push(msg)` |
| `ctx.read_messages()` | `ctx.dialogue_mut().read()` |
| `ctx.read_all_messages()` | `ctx.dialogue_mut().read_all()` |
| `ctx.scroll_dialogue(pos)` | `ctx.dialogue_mut().scroll(pos)` |
| `ctx.scroll_dialogue_by_cycles(n)` | `ctx.dialogue_mut().scroll_cycles(n)` |
| `ctx.align_dialogue_to_turn_boundary()` | `ctx.dialogue_mut().align_turns()` |
| `ctx.expand_cold_zone(s, e)` | `ctx.dialogue_mut().expand_cold(s, e)` |
| `ctx.close_cold_zone()` | `ctx.dialogue_mut().close_cold()` |
| `ctx.request_cold_zone()` | `ctx.dialogue_mut().request_cold()` |
| `ctx.tick_cold_zone_countdown()` | `ctx.dialogue_mut().tick_cold()` |
| `ctx.collect_persistable()` | `ctx.persistence().collect()` |

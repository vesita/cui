# CUI —— 面向 LLM Agent 的上下文 UI 框架

**CUI** 将 LLM 的上下文窗口视为**渲染画布**。它借鉴 React/Dioxus 的组件模型，提供声明式、可交互的组件树，渲染为 LLM 可解析的紧凑文本格式。

## 核心理念

- **组件即上下文** — 每个组件渲染为紧凑 Markdown。LLM 通过 `component_action` 工具与之交互。
- **容量自适应** — 根据 token 预算自动降级/升级组件的展示粒度。
- **数据与渲染分离** — `ComponentTree` 是纯数据模型，`Context` 持有渲染状态机和管线。
- **类型驱动** — 声明 `type: tool` 即可获得默认行为，零样板代码。

## 快速开始

```rust
use cui::Cui;

let mut ctx = Cui::init()
    .section("essential/goals.cui")
    .build();

// 条件渲染 —— 渲染后条件自动清除
let output = ctx.in_condition("plan").render();

// 多条件 OR + 预算控制
let output = ctx.in_condition("plan")
    .and("act")
    .with_budget(50000)
    .render();
```

## 渲染级别

```
Hidden → Title → Summary → Standard → Detailed
```

组件根据剩余 token 预算和优先级自动降级/升级。

## .cui 文件格式

```yaml
---
type: tool
id: bash
title: Bash 执行器
priority: high
when: act
---
执行 Shell 命令并返回结果。
```

## 输出格式

```
## [标题]              ← 组件标题
正文内容...             ← Markdown
`[expand]` `[collapse]`  ← 交互动作

## [_recent]           ← 近期操作（3 轮记忆）
  ·[Bash] expand ✓

## [_overview]         ← 隐藏组件列表
  `plan` `review` `[expand_hidden]`
```

## 功能特性

| 特性开关 | 说明 |
|---|---|
| `prompts`（默认） | 提示词文件加载 |
| `instructions` | 系统指令解析（需要网络） |
| `test-utils` | 下游 crate 测试工具 |

## 子包

| 包名 | 说明 |
|---|---|
| `cui` | 核心库 |
| `crate/cui-derive` | 派生宏（`#[derive(BaseComponent)]`、`#[derive(ActionHandler)]`） |
| `crate/state-macro` | 状态机代码生成（`states!`、`transitions!`、`facade!`） |

## 许可证

MIT

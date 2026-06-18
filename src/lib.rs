//! CUI（Context UI）—— 基于 CuiComponent 的上下文 UI 框架。
//!
//! 借鉴 Dioxus 组件模型，将组件渲染改造为
//! 声明式、可交互的组件树。核心概念：
//!
//! - **CuiComponent** — 核心组件接口，有 id/title/priority/actions
//! - **RenderLevel** — 控制组件在不同容量压力下的展示粒度
//! - **CapacityPlanner** — 迭代降级/升级算法，按优先级分配预算
//! - **ComponentTree/ComponentNode** — 树形组件管理
//! - **TypeRegistry** — 语义类型注册表，声明 `type: tool` 自动提供默认行为
//!
//! 输出格式：YAML frontmatter + Markdown body，AI 通过 `component_action`
//! 工具与组件交互（展开、折叠、滚动等）。
//!
//! # 模块组织
//!
//! - [`types`] — 基础类型（RenderLevel、DataMode、Action 等）
//! - [`component`] — CuiComponent/ComponentTree/ComponentNode 体系
//! - [`render`] — 渲染管线（容量规划、状态机）
//! - [`context`] — 运行时上下文管理器
//! - [`compile`] — 编译管道：.cui 源码 → 模板填充 → 组件树
//! - [`content`] — 内容加载：内置资源、提示词、系统指令
//! - [`runtime`] — 运行时服务（事件、处理器、类型注册表、对话管理）
//! - [`adapter`] — .cui 模板 → 组件节点的轻量适配

// ── 基础类型 ──────────────────────────────────────────

pub mod types;
pub use types::{action, condition, data, keyword, level, manage};

// ── 外部子 crate ─────────────────────────────────────

pub use cui_tokenizer as tokenizer;

// ── 组件模型 ────────────────────────────────────────

pub mod component;
pub use component::builtin::{
    Body, Button, DataSlot, GroupBuilder, Label, Toast, body, button, data_slot, group, label,
    text_block, toast,
};
pub use component::{
    CuiComponent, ComponentLifecycle, ComponentNode, ComponentTree, Persistable,
    builtin,
};

// ── 运行时服务 ──────────────────────────────────────

pub mod runtime;
pub use runtime::context::Context;
pub use runtime::registry::{
    ComponentTypeDef, TypeRegistry, builtin_registry,
};
pub use runtime::ordering;

// ── 语法高亮 ────────────────────────────────────────

pub mod syntax;

// ── 编译管道 ────────────────────────────────────────

pub mod compile;
pub use compile::compiler::{Compiler, CompileError, ValidationWarning, compile_sources, ToolPaths, SkillEntries, expand_multi_document, is_multi_document};
pub use compile::file::{CuiDirectory, CuiFileComponent};
pub use compile::template::TemplateResolver;

// ── 内容加载 ────────────────────────────────────────

pub mod content;

#[cfg(feature = "instructions")]
pub use content::instructions;
#[cfg(feature = "instructions")]
pub use content::instructions::{
    resolve_nearby_instructions, system_paths, system_prompt, system_prompt_and_sources,
    system_prompt_and_sources_with_cache,
};

// ── 适配器 ──────────────────────────────────────────

pub mod adapter;

// ── 类型级 re-export ─────────────────────────────────

pub use action::{
    ActionDef, ActionRequest, ActionResult, ActionVariant, DialogueOps, VisibilityRule,
};
pub use condition::VisibilityCondition;
pub use data::{DataMode, TruncatePolicy};
pub use keyword::{ComponentKind, IoDef, IoType, PriorityLevel};
pub use level::RenderLevel;
pub use ordering::OrderingStrategy;
pub use runtime::handler::{ActionContext, ActionHandler, ActionHandlerRef, ActionOutput, HandlerRegistry};

// ── 测试工具模块（仅在 test 下编译） ───────────────

#[cfg(any(test, feature = "test-utils"))]
pub use runtime::test_utils;

// ── CUI 格式化 ──────────────────────────────────────

/// 将键值对和正文格式化为 CUI 块（YAML frontmatter + body）。
///
/// 输出格式：
/// ```text
/// ---
/// key1: value1
/// key2: value2
/// ---
/// body
/// ```
///
/// 若 fields 为空，仅返回 body（无 frontmatter）。
pub fn format_cui_block(fields: &[(&str, &str)], body: &str) -> String {
    if fields.is_empty() {
        return body.to_string();
    }
    let mut out = String::from("---\n");
    for (k, v) in fields {
        out.push_str(k);
        out.push_str(": ");
        out.push_str(v);
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}

// ── 宏导出 ──────────────────────────────────────────

pub use cui_derive::{ActionHandler, CuiComponent};

// ── 框架入口 ────────────────────────────────────────

/// CUI 框架入口。
///
/// ```ignore
/// use cui::Cui;
/// let ctx = Cui::init()
///     .section("essential/goals.cui")
///     .build();
/// ```
pub struct Cui;

impl Cui {
    /// 创建 [`Compiler`]，开始声明式组装 Context。
    pub fn init() -> Compiler {
        Compiler::new()
    }
}

//! CUI 框架构建器 —— 声明式组装 Context。
//!
//! 用法：
//! ```ignore
//! use cui::Cui;
//! let ctx = Cui::init()
//!     .section("essential/goals.cui")
//!     .component(my_dynamic_node)
//!     .handlers(&registry)
//!     .build();
//! ```

use std::sync::{Arc, Mutex};

use crate::action::DialogueOps;
use crate::component::ComponentNode;
use crate::component::builtin::TextBlock;

use crate::CuiFileComponent;
use crate::context::Context;
use crate::data::DataMode;
use crate::runtime::handler::{ActionHandler, HandlerRegistry};

/// CUI 框架构建器。
///
/// 通过 [`Cui::init()`] 获取实例，链式调用装配组件，最后 [`build()`](Self::build) 产出 [`Context`]。
pub struct CuiBuilder {
    ctx: Context,
    include_intro: bool,
}

impl CuiBuilder {
    pub fn new() -> Self {
        let mut builder = Self {
            ctx: Context::new(),
            include_intro: true,
        };
        builder.inject_introduction();
        builder
    }

    /// 跳过自动注入的介绍组件（`_cui_introduction`）。
    ///
    /// 默认情况下，`Cui::init()` 会注册内置的 CUI 框架参考文档，
    /// 调用此方法可移除已注入的介绍组件并阻止后续注入。
    pub fn without_introduction(mut self) -> Self {
        self.ctx.remove("_cui_introduction");
        self.include_intro = false;
        self
    }

    fn inject_introduction(&mut self) {
        if !self.include_intro {
            return;
        }
        const INTRO_CONTENT: &str = include_str!("../../cui/_cui_introduction.cui");
        let comp = match CuiFileComponent::from_str(INTRO_CONTENT, "_cui_introduction") {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("内置 _cui_introduction.cui 解析失败: {e}");
                return;
            }
        };
        let content = comp.body().to_string();
        let id = comp.id().to_string();
        let title = comp.title().to_string();
        let mut tb = TextBlock::new(&id, &title, &content);
        tb = Self::apply_frontmatter(tb, &comp);
        self.ctx.register(tb.build());
    }

    /// 从 `.cui` 文件加载，返回 (body, component) 对。
    fn load_file(path: &str) -> (String, Option<CuiFileComponent>) {
        match CuiFileComponent::from_file(path) {
            Ok(comp) => (comp.body().to_string(), Some(comp)),
            Err(_) => (String::new(), None),
        }
    }

    fn apply_frontmatter(tb: TextBlock, fm: &CuiFileComponent) -> TextBlock {
        let mut tb = tb.priority(fm.priority());
        if fm.is_inert() {
            tb = tb.inert();
        }
        if fm.collapsible() {
            tb = tb.collapsible();
        }
        if fm.collapsed() {
            tb = tb.collapsed(true);
        }
        tb.with_condition(fm.visibility_condition())
    }

    // ── Section ──────────────────────────────────────────

    /// 从 `.cui` 文件解析出 TextBlock（含 frontmatter 应用），返回后调用方仍可修改。
    fn load_text_block(path: &str) -> TextBlock {
        let (content, fm) = Self::load_file(path);
        let (id, title) = fm
            .as_ref()
            .map(|c| (c.id().to_string(), c.title().to_string()))
            .unwrap_or_else(|| {
                let stem = path
                    .split('/')
                    .next_back()
                    .unwrap_or(path)
                    .trim_end_matches(".cui");
                (stem.to_string(), stem.to_string())
            });
        let mut tb = TextBlock::new(&id, &title, &content);
        if let Some(ref fm) = fm {
            tb = Self::apply_frontmatter(tb, fm);
        }
        tb
    }

    /// 从 `.cui` 文件加载静态 section。
    ///
    /// 自动读取 frontmatter 中的 `id`、`title`、`priority`、`inert`、`collapsible`、
    /// `collapsed` 等元数据构建 [`TextBlock`] 并注册。
    ///
    /// frontmatter 缺失时，以文件路径推导 id/title。
    /// `path` 为 `.cui` 文件的完整路径。
    pub fn section(mut self, path: &str) -> Self {
        self.ctx.register(Self::load_text_block(path).build());
        self
    }

    /// 从 `.cui` 文件加载 section，允许对 [`TextBlock`] 做动态修改（如追加运行时数据）。
    ///
    /// `path` 为 `.cui` 文件的完整路径。
    pub fn section_with(mut self, path: &str, f: impl FnOnce(&mut TextBlock)) -> Self {
        let mut tb = Self::load_text_block(path);
        f(&mut tb);
        self.ctx.register(tb.build());
        self
    }

    // ── 批量加载 ────────────────────────────────────────

    /// 批量加载 `.cui` 文件作为 section。
    ///
    /// 每个文件的前置元数据（priority、when、collapsible、inert 等）自动应用。
    /// `path` 为 `.cui` 文件的完整路径。
    pub fn load_sections(mut self, paths: &[&str]) -> Self {
        for path in paths {
            self = self.section(path);
        }
        self
    }

    /// 向已注册的 section 追加运行时数据。
    ///
    /// 必须在对应的 section 已注册后调用。
    ///
    /// ```ignore
    /// Cui::init()
    ///     .load_sections(&["essential/constraints.cui"])
    ///     .data("constraints", "- 禁止: rm -rf /")
    /// ```
    pub fn data(mut self, id: &str, value: &str) -> Self {
        self.ctx.write(id, DataMode::Append, value);
        self
    }

    // ── 组件注册 ────────────────────────────────────────

    /// 注册一个已构建好的 [`ComponentNode`]（用于非 `.cui` 来源的动态组件）。
    pub fn component(mut self, node: ComponentNode) -> Self {
        self.ctx.register(node);
        self
    }

    /// 批量注册组件。
    pub fn components(mut self, nodes: impl IntoIterator<Item = ComponentNode>) -> Self {
        self.ctx.register_all(nodes);
        self
    }

    /// 注册对话组件（同时绑定 [`DialogueOps`]）。
    pub fn dialogue(
        mut self,
        node: ComponentNode,
        ops: Arc<Mutex<Box<dyn DialogueOps + Send>>>,
    ) -> Self {
        self.ctx.register_dialogue_node(node, ops);
        self
    }

    // ── 处理器 ──────────────────────────────────────────

    /// 注册处理器注册表（批量导入）。
    pub fn handlers(mut self, registry: &HandlerRegistry) -> Self {
        self.ctx.register_handlers(registry);
        self
    }

    /// 注册单个命名处理器。
    pub fn handler(mut self, name: impl Into<String>, handler: Arc<dyn ActionHandler>) -> Self {
        self.ctx.register_handler(name, handler);
        self
    }

    // ── 扩展 ────────────────────────────────────────────

    /// 注入后端扩展资源（handler 通过 `ActionContext::resource::<T>()` 获取）。
    pub fn extension<T: 'static + Send + Sync>(mut self, ext: T) -> Self {
        self.ctx.set_extension(ext);
        self
    }

    // ── 内部访问 ────────────────────────────────────────

    /// 获取 Context 的可变引用，用于需要直接操作 Context 的场景
    /// （如 `set_ordering` 等）。
    pub fn ctx_mut(&mut self) -> &mut Context {
        &mut self.ctx
    }

    // ── 构建 ────────────────────────────────────────────

    /// 产出 [`Context`]，此后可通过 `&mut Context` 操作组件树和渲染。
    pub fn build(self) -> Context {
        self.ctx
    }
}

impl Default for CuiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

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
use crate::keyword::PriorityLevel;

use crate::CuiFileComponent;
use crate::context::Context;
use crate::data::DataMode;
use crate::runtime::handler::{ActionHandler, HandlerRegistry};
use crate::runtime::registry::{TypeRegistry, builtin_registry};

/// CUI 框架构建器。
///
/// 通过 [`Cui::init()`] 获取实例，链式调用装配组件，最后 [`build()`](Self::build) 产出 [`Context`]。
///
/// ```ignore
/// use cui::Cui;
/// let ctx = Cui::init()
///     .without_introduction()
///     .type_registry(my_types)
///     .tools("tools", "可用工具", PriorityLevel::High, (
///         "tools/read_file.cui",
///         "tools/run_test.cui",
///     ))
///     .skills("skills", "技能参考", (
///         ("Rust 审查", "检查 unsafe 块"),
///         ("安全审计", "扫描 SQL 注入"),
///     ))
///     .handlers(&registry)
///     .build();
/// ```
pub struct CuiBuilder {
    ctx: Context,
    include_intro: bool,
    type_registry: TypeRegistry,
}

impl CuiBuilder {
    pub fn new() -> Self {
        let mut builder = Self {
            ctx: Context::new(),
            include_intro: true,
            type_registry: builtin_registry(),
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

    /// 从多文档 `.cui` 文件加载，返回所有 (body, component) 对。
    fn load_file_multi(path: &str) -> Vec<(String, Option<CuiFileComponent>)> {
        match CuiFileComponent::from_file_multi(path) {
            Ok(comps) => comps
                .into_iter()
                .map(|c| (c.body().to_string(), Some(c)))
                .collect(),
            Err(_) => vec![],
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

    /// 批量加载 `.cui` 文件作为 section。自动展开多文档文件。
    ///
    /// 每个文件的前置元数据（priority、when、collapsible、inert 等）自动应用。
    /// `path` 为 `.cui` 文件的完整路径。
    pub fn load_sections(mut self, paths: &[&str]) -> Self {
        for path in paths {
            for (content, fm) in Self::load_file_multi(path) {
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
                self.ctx.register(tb.build());
            }
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

    // ── 用户覆盖 ────────────────────────────────────────

    /// 从目录加载用户覆盖，应用到已注册组件。
    ///
    /// 应在所有 `section()` / `load_dir()` 之后、`build()` 之前调用。
    /// 按 `id` 匹配；支持 `pinned: true`、`title`、`body` 和 `inputs` 覆盖。
    pub fn with_user_overrides_from(mut self, dir: impl AsRef<std::path::Path>) -> Self {
        let overrides = crate::compile::file::load_user_overrides(dir.as_ref());
        for o in &overrides {
            if let Some(node) = self.ctx.tree_mut().find_mut(&o.id) {
                crate::component::builtin::leaf_apply_override(
                    node,
                    o.title.as_deref(),
                    o.body.as_deref(),
                    &o.inputs,
                    o.pinned,
                );
            }
        }
        self
    }

    // ── 目录加载 ────────────────────────────────────────

    /// 从目录批量加载 `.cui` 文件（单文档模式）。
    ///
    /// 自动跳过 `_` 和 `.` 开头的文件。
    pub fn load_dir(mut self, dir: impl AsRef<std::path::Path>) -> Self {
        let cui_dir = crate::compile::file::CuiDirectory::new(dir.as_ref());
        match cui_dir.load() {
            Ok(comps) => {
                for comp in comps {
                    let mut tb = TextBlock::new(comp.id(), comp.title(), comp.body());
                    tb = Self::apply_frontmatter(tb, &comp);
                    self.ctx.register(tb.build());
                }
            }
            Err(e) => tracing::warn!("load_dir failed: {}", e),
        }
        self
    }

    // ── 类型注册表 ──────────────────────────────────────

    /// 设置自定义类型注册表（替换内置的 `tool` / `section` 类型）。
    ///
    /// 用于注册项目特有的工具类型，如 `tool.code_review`。
    /// 传入的注册表会与内置类型合并。
    pub fn type_registry(mut self, registry: TypeRegistry) -> Self {
        let mut merged = builtin_registry();
        for (_, def) in registry.into_types() {
            merged.register(def);
        }
        self.type_registry = merged;
        self
    }

    /// 获取类型注册表的引用（用于外部扩展）。
    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.type_registry
    }

    // ── 工具 ────────────────────────────────────────────

    /// 加载单个工具 `.cui` 文件并立即注册。
    pub fn tool(mut self, path: &str) -> Self {
        if let Some(node) = build_tool(path, &self.type_registry) {
            self.ctx.register(node);
        }
        self
    }

    /// 批量加载工具，打包为一个可折叠分组。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// Cui::init()
    ///     .tools("tools", "可用工具", PriorityLevel::High, (
    ///         "tools/read_file.cui",
    ///         "tools/run_test.cui",
    ///     ))
    ///     .build();
    /// ```
    pub fn tools(
        mut self,
        id: &str,
        title: &str,
        priority: PriorityLevel,
        paths: impl ToolPaths,
    ) -> Self {
        let nodes: Vec<ComponentNode> = paths
            .collect_paths()
            .iter()
            .filter_map(|p| build_tool(p, &self.type_registry))
            .collect();
        if !nodes.is_empty() {
            let mut group = crate::component::builtin::group(id, title)
                .priority(priority)
                .collapsible()
                .collapsed(true);
            for node in nodes {
                group = group.push(node);
            }
            self.ctx.register(group.build());
        }
        self
    }

    // ── 技能 ────────────────────────────────────────────

    /// 添加单个技能条目（惰性参考，`priority: Low` + `inert`）。
    pub fn skill(mut self, name: &str, desc: &str) -> Self {
        let content = format!("- **{}**: {}\n", name, desc);
        let node = TextBlock::new(name, name, &content)
            .priority(PriorityLevel::Low)
            .inert()
            .build();
        self.ctx.register(node);
        self
    }

    /// 批量添加技能，合并为一个列表组件。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// Cui::init()
    ///     .skills("skills", "技能参考", (
    ///         ("Rust 审查", "检查 unsafe 块"),
    ///         ("安全审计", "扫描 SQL 注入"),
    ///     ))
    ///     .build();
    /// ```
    pub fn skills(
        mut self,
        id: &str,
        title: &str,
        entries: impl SkillEntries,
    ) -> Self {
        let skills = entries.collect_entries();
        if !skills.is_empty() {
            let mut content = String::new();
            for (name, desc) in &skills {
                content.push_str(&format!("- **{}**: {}\n", name, desc));
            }
            let node = TextBlock::new(id, title, &content)
                .priority(PriorityLevel::Low)
                .inert()
                .build();
            self.ctx.register(node);
        }
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

// ── 元组 trait：工具路径列表 ──────────────────────────

/// 可转换为工具路径列表的类型。
///
/// 为 1–12 元组实现，支持 Bevy 风格的元组传参。
pub trait ToolPaths {
    fn collect_paths(self) -> Vec<String>;
}

impl ToolPaths for Vec<String> {
    fn collect_paths(self) -> Vec<String> { self }
}

macro_rules! impl_tool_paths {
    ($($T:ident),*) => {
        impl<$($T: AsRef<str>),*> ToolPaths for ($($T,)*) {
            #[allow(non_snake_case)]
            fn collect_paths(self) -> Vec<String> {
                let ($($T,)*) = self;
                vec![$($T.as_ref().to_string()),*]
            }
        }
    };
}

// 1-tuple 到 12-tuple
impl_tool_paths!(A);
impl_tool_paths!(A, B);
impl_tool_paths!(A, B, C);
impl_tool_paths!(A, B, C, D);
impl_tool_paths!(A, B, C, D, E);
impl_tool_paths!(A, B, C, D, E, F);
impl_tool_paths!(A, B, C, D, E, F, G);
impl_tool_paths!(A, B, C, D, E, F, G, H);
impl_tool_paths!(A, B, C, D, E, F, G, H, I);
impl_tool_paths!(A, B, C, D, E, F, G, H, I, J);
impl_tool_paths!(A, B, C, D, E, F, G, H, I, J, K);
impl_tool_paths!(A, B, C, D, E, F, G, H, I, J, K, L);

// ── 元组 trait：技能条目列表 ──────────────────────────

/// 可转换为技能条目的类型。
///
/// 为 1–12 元组实现，每个元素为 `(name, desc)` 对。
pub trait SkillEntries {
    fn collect_entries(self) -> Vec<(String, String)>;
}

macro_rules! impl_skill_entries {
    ($(($N:ident, $D:ident)),*) => {
        impl<$($N: AsRef<str>, $D: AsRef<str>),*> SkillEntries for ($(($N, $D),)*) {
            #[allow(non_snake_case)]
            fn collect_entries(self) -> Vec<(String, String)> {
                let ($(($N, $D),)*) = self;
                vec![$(($N.as_ref().to_string(), $D.as_ref().to_string())),*]
            }
        }
    };
}

impl_skill_entries!((A, B));
impl_skill_entries!((A, B), (C, D));
impl_skill_entries!((A, B), (C, D), (E, F));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T), (U, V));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T), (U, V), (W, X));

// ── helper ────────────────────────────────────────────

fn build_tool(path: &str, registry: &TypeRegistry) -> Option<ComponentNode> {
    let comp = CuiFileComponent::from_file(path).ok()?;
    let mut node = crate::compile::compiler::resolve_tool(&comp, registry);
    if comp.collapsible() {
        node.set_collapsible(true);
        node.set_collapsed(comp.collapsed());
    }
    Some(node)
}

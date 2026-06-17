//! CUI 编译器 —— 声明式配置收集 + 统一编译入口。
//!
//! ```ignore
//! use cui::Cui;
//! let ctx = Cui::init()
//!     .without_introduction()
//!     .load_dir("cui/")
//!     .tools("tools", "可用工具", PriorityLevel::High, (
//!         "tools/read.cui",
//!     ))
//!     .skills("skills", "技能", (
//!         ("Rust", "检查 unsafe 块"),
//!     ))
//!     .handlers(&registry)
//!     .build();
//! ```

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::action::DialogueOps;
use crate::compile::file::CuiFileComponent;
use crate::component::{
    ComponentNode,
    builtin::{CuiFileLeaf, TextBlock, group},
};
use crate::runtime::context::Context;
use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::runtime::handler::{ActionHandler, HandlerRegistry};
use crate::runtime::registry::{TypeRegistry, builtin_registry};
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use crate::keyword::ComponentKind;

// ── Compiler ────────────────────────────────────────────────

/// CUI 编译器 —— 链式收集配置，`build()` 时统一编译。
pub struct Compiler {
    ctx: Context,
    include_intro: bool,
    type_registry: TypeRegistry,
    dirs: Vec<PathBuf>,
    files: Vec<String>,
    tools: Vec<(String, String, PriorityLevel, Vec<String>)>,
    skills: Vec<(String, String, Vec<(String, String)>)>,
    user_override_dir: Option<PathBuf>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ctx: Context::new(),
            include_intro: false,
            type_registry: builtin_registry(),
            dirs: Vec::new(),
            files: Vec::new(),
            tools: Vec::new(),
            skills: Vec::new(),
            user_override_dir: None,
        }
    }

    /// 显式加载 `_cui_introduction` 参考组件（框架说明文档）。
    pub fn with_introduction(mut self) -> Self {
        self.include_intro = true;
        self
    }

    /// [已废弃] 原为关闭 introduction，现为默认行为。保留仅用于向后兼容。
    #[deprecated = "introduction 默认关闭，请删除此调用或改用 with_introduction()"]
    pub fn without_introduction(self) -> Self {
        self
    }

    pub fn load_dir(mut self, dir: impl AsRef<std::path::Path>) -> Self {
        self.dirs.push(dir.as_ref().to_path_buf());
        self
    }

    pub fn section(mut self, path: &str) -> Self {
        self.files.push(path.to_string());
        self
    }

    pub fn data(mut self, id: &str, value: &str) -> Self {
        self.ctx.write(id, DataMode::Append, value);
        self
    }

    pub fn component(mut self, node: ComponentNode) -> Self {
        self.ctx.register(node);
        self
    }

    pub fn components(mut self, nodes: impl IntoIterator<Item = ComponentNode>) -> Self {
        self.ctx.register_all(nodes);
        self
    }

    pub fn dialogue(mut self, node: ComponentNode, ops: Arc<Mutex<Box<dyn DialogueOps + Send>>>) -> Self {
        self.ctx.register_dialogue_node(node, ops);
        self
    }

    pub fn handlers(mut self, registry: &HandlerRegistry) -> Self {
        self.ctx.register_handlers(registry);
        self
    }

    pub fn handler(mut self, name: impl Into<String>, handler: Arc<dyn ActionHandler>) -> Self {
        self.ctx.register_handler(name, handler);
        self
    }

    pub fn extension<T: 'static + Send + Sync>(mut self, ext: T) -> Self {
        self.ctx.set_extension(ext);
        self
    }

    pub fn user_overrides(mut self, dir: impl AsRef<std::path::Path>) -> Self {
        self.user_override_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    pub fn type_registry(mut self, registry: TypeRegistry) -> Self {
        let mut merged = builtin_registry();
        for (_, def) in registry.into_types() { merged.register(def); }
        self.type_registry = merged;
        self
    }

    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.type_registry
    }

    // ── 工具 ────────────────────────────────────────────

    pub fn tool(mut self, path: &str) -> Self {
        if let Some(name) = path.split('/').next_back() {
            let n = name.trim_end_matches(".cui").to_string();
            self.tools.push((n.clone(), n, PriorityLevel::High, vec![path.to_string()]));
        }
        self
    }

    pub fn tools(mut self, id: &str, title: &str, priority: PriorityLevel, paths: impl ToolPaths) -> Self {
        let collected = paths.collect_paths();
        if !collected.is_empty() { self.tools.push((id.to_string(), title.to_string(), priority, collected)); }
        self
    }

    // ── 技能 ────────────────────────────────────────────

    pub fn skill(mut self, name: &str, desc: &str) -> Self {
        self.skills.push((name.to_string(), name.to_string(), vec![(name.to_string(), desc.to_string())]));
        self
    }

    pub fn skills(mut self, id: &str, title: &str, entries: impl SkillEntries) -> Self {
        let collected = entries.collect_entries();
        if !collected.is_empty() { self.skills.push((id.to_string(), title.to_string(), collected)); }
        self
    }

    pub fn ctx_mut(&mut self) -> &mut Context { &mut self.ctx }

    // ── 构建 ────────────────────────────────────────────

    pub fn build(mut self) -> Context {
        if self.include_intro { inject_introduction(&mut self.ctx); }

        let (nodes, warnings) = compile_sources(
            &self.dirs, &self.files, &self.tools, &self.skills,
            &self.type_registry,
        );

        for w in &warnings {
            match w {
                ValidationWarning::UnresolvedHandler { component_id, handler_name } => {
                    tracing::warn!("编译警告: 组件 '{component_id}' 引用的 handler '{handler_name}' 未注册");
                }
            }
        }
        self.ctx.register_all(nodes);

        if let Some(ref dir) = self.user_override_dir {
            let overrides = crate::compile::file::load_user_overrides(dir);
            for o in &overrides {
                if let Some(node) = self.ctx.tree_mut().find_mut(&o.id) {
                    crate::component::builtin::leaf_apply_override(
                        node, o.title.as_deref(), o.body.as_deref(), &o.inputs, o.pinned,
                    );
                }
            }
        }

        self.ctx
    }
}

impl Default for Compiler {
    fn default() -> Self { Self::new() }
}

// ── 编译核心 ────────────────────────────────────────────────

/// 编译源列表为组件节点 + 验证警告。
pub fn compile_sources(
    dirs: &[PathBuf],
    files: &[String],
    tools: &[(String, String, PriorityLevel, Vec<String>)],
    skills: &[(String, String, Vec<(String, String)>)],
    registry: &TypeRegistry,
) -> (Vec<ComponentNode>, Vec<ValidationWarning>) {
    let mut nodes: Vec<ComponentNode> = Vec::new();
    let mut warnings: Vec<ValidationWarning> = Vec::new();

    for dir in dirs {
        let cui_dir = crate::compile::file::CuiDirectory::new(dir);
        if let Ok(comps) = cui_dir.load_multi() {
            for comp in comps {
                let mut tb = TextBlock::new(comp.id(), comp.title(), comp.body());
                tb = apply_meta(tb, &comp);
                nodes.push(tb.build());
            }
        }
    }

    for path in files {
        if let Ok(comps) = CuiFileComponent::from_file_multi(path) {
            for comp in comps {
                let mut tb = TextBlock::new(comp.id(), comp.title(), comp.body());
                tb = apply_meta(tb, &comp);
                nodes.push(tb.build());
            }
        }
    }

    for (group_id, group_title, priority, paths) in tools {
        let mut tool_nodes = Vec::new();
        for path in paths {
            if let Some(node) = build_tool(path, registry) {
                tool_nodes.push(node);
            }
        }
        if !tool_nodes.is_empty() {
            let mut builder = group(group_id, group_title)
                .priority(*priority).collapsible().collapsed(true);
            for tool in tool_nodes { builder = builder.push(tool); }
            nodes.push(builder.build());
        }
    }

    for (id, title, entries) in skills {
        if !entries.is_empty() {
            let mut content = String::new();
            for (name, desc) in entries { content.push_str(&format!("- **{}**: {}\n", name, desc)); }
            nodes.push(TextBlock::new(id, title, &content).priority(PriorityLevel::Low).inert().build());
        }
    }

    (nodes, warnings)
}

// ── 验证 ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ValidationWarning {
    UnresolvedHandler { component_id: String, handler_name: String },
}

// ── 错误 ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CompileError {
    ParseError { message: String },
    IoError { path: String, error: String },
    #[cfg(test)]
    ReferenceNotFound { id: String, source_id: String },
    #[cfg(test)]
    CircularReference { id: String, chain: Vec<String> },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError { message } => write!(f, "编译错误: {message}"),
            Self::IoError { path, error } => write!(f, "IO 错误 ({path}): {error}"),
            #[cfg(test)]
            Self::ReferenceNotFound { id, source_id } => write!(f, "引用未找到: {id} (来源: {source_id})"),
            #[cfg(test)]
            Self::CircularReference { id, chain } => write!(f, "循环引用: {id} → {chain:?}"),
        }
    }
}

impl std::error::Error for CompileError {}

// ── helpers ─────────────────────────────────────────────────

fn inject_introduction(ctx: &mut Context) {
    const INTRO: &str = include_str!("../../cui/_cui_introduction.cui");
    let comp = match CuiFileComponent::from_str(INTRO, "_cui_introduction") {
        Ok(c) => c,
        Err(e) => { tracing::warn!("intro 解析失败: {e}"); return; }
    };
    let mut tb = TextBlock::new(comp.id(), comp.title(), comp.body()).priority(comp.priority());
    if comp.is_inert() { tb = tb.inert(); }
    if comp.collapsible() { tb = tb.collapsible(); }
    if comp.collapsed() { tb = tb.collapsed(true); }
    tb = tb.with_condition(comp.visibility_condition());
    ctx.register(tb.build());
}

fn apply_meta(tb: TextBlock, fm: &CuiFileComponent) -> TextBlock {
    let mut tb = tb.priority(fm.priority());
    if fm.is_inert() { tb = tb.inert(); }
    if fm.collapsible() { tb = tb.collapsible(); }
    if fm.collapsed() { tb = tb.collapsed(true); }
    tb.with_condition(fm.visibility_condition())
}

fn build_tool(path: &str, registry: &TypeRegistry) -> Option<ComponentNode> {
    let comp = CuiFileComponent::from_file(path).ok()?;
    let mut node = resolve_tool(&comp, registry);
    if comp.collapsible() { node.set_collapsible(true); node.set_collapsed(comp.collapsed()); }
    Some(node)
}

fn merge_input_values(comp: &CuiFileComponent, registry: &TypeRegistry) -> Vec<(String, String)> {
    use std::collections::BTreeMap;
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    if let Some(type_name) = comp.component_type()
        && let Some(typedef) = registry.lookup(type_name)
    {
        for input in &typedef.inputs {
            if let Some(ref default) = input.default_value {
                merged.entry(input.name.clone()).or_insert_with(|| default.clone());
            }
        }
    }
    for (name, val) in comp.input_values() { merged.insert(name, val); }
    merged.into_iter().collect()
}

pub fn resolve_tool(comp: &CuiFileComponent, registry: &TypeRegistry) -> ComponentNode {
    let type_name = comp.component_type();
    let resolved = type_name
        .and_then(|tn| registry.resolve(
            tn, comp.id(), comp.title(),
            Some(comp.component_kind()), Some(comp.priority()),
            &comp.actions(), &comp.render_body(crate::level::RenderLevel::Standard),
            comp.summary(), Some(comp.is_inert()), Some(comp.is_static()),
            comp.handler(), comp.component_children(),
            comp.component_source(), comp.persist_key(),
            comp.is_entry(), comp.budget_ratio(),
        ).ok())
        .unwrap_or_else(|| crate::runtime::registry::ResolvedComponent {
            id: comp.id().to_string(), title: comp.title().to_string(),
            kind: comp.component_kind(), priority: comp.priority(),
            summary: comp.summary.clone(), inert: comp.is_inert(),
            is_static: comp.is_static(), actions: comp.actions(),
            body: comp.render_body(crate::level::RenderLevel::Standard),
            children: comp.component_children().to_vec(),
            source: comp.component_source().map(|s| s.to_string()),
            persist: comp.persist_key().map(|s| s.to_string()),
            entry: comp.is_entry(), budget_ratio: comp.budget_ratio(),
            subtype: None,
        });

    let mut leaf = CuiFileLeaf::new(&resolved.id, &resolved.title, &resolved.body)
        .priority(resolved.priority).kind(resolved.kind)
        .with_condition(comp.visibility_condition())
        .with_inputs(comp.inputs().to_vec()).with_outputs(comp.outputs().to_vec());
    if resolved.inert { leaf = leaf.inert(); }
    if resolved.is_static { leaf = leaf.is_static(); }
    if let Some(ref s) = resolved.summary { leaf = leaf.summary(s.as_str()); }
    if let Some(ref pk) = resolved.persist { leaf = leaf.persist(pk.as_str()); }
    if let Some(ref st) = resolved.subtype { leaf = leaf.subtype(st.as_str()); }
    let pairs = merge_input_values(comp, registry);
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    leaf = leaf.with_input_values(&refs);
    let mut node = leaf.build();
    node.set_actions(resolved.actions);
    node
}

// ── 多文档支持 ──────────────────────────────────────────────

pub fn is_multi_document(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let mut doc_starts = 0u8;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "---" {
            if i + 1 < lines.len() && is_yaml_key(lines[i + 1].trim()) { doc_starts += 1; }
        }
        i += 1;
    }
    doc_starts >= 2
}

fn is_yaml_key(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() { return false; }
    let first = t.as_bytes()[0];
    if !first.is_ascii_alphabetic() && first != b'_' { return false; }
    if let Some(colon) = t.find(':') { t[..colon].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') } else { false }
}

fn parse_multi_document(content: &str) -> Result<Vec<(String, String)>, CompileError> {
    let content = content.trim();
    let lines: Vec<&str> = content.lines().collect();
    let mut docs = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "---" {
            i += 1;
            let mut yaml = String::new();
            while i < lines.len() {
                if lines[i].trim() == "---" { i += 1; break; }
                if !yaml.is_empty() { yaml.push('\n'); }
                yaml.push_str(lines[i]); i += 1;
            }
            if yaml.is_empty() {
                if docs.is_empty() { return Err(CompileError::ParseError { message: "未找到闭合 ---".into() }); }
                break;
            }
            let mut body = String::new();
            while i < lines.len() {
                if lines[i].trim() == "---" && i + 1 < lines.len() && is_yaml_key(lines[i+1].trim()) { break; }
                if !body.is_empty() { body.push('\n'); }
                body.push_str(lines[i]); i += 1;
            }
            docs.push((yaml, body));
        } else { i += 1; }
    }
    if docs.is_empty() { return Err(CompileError::ParseError { message: "未找到文档块".into() }); }
    Ok(docs)
}

pub fn expand_multi_document(content: &str, file_id: &str) -> Result<Vec<CuiFileComponent>, String> {
    if !is_multi_document(content) { return Ok(vec![CuiFileComponent::from_str(content, file_id)?]); }
    let docs = parse_multi_document(content).map_err(|e| e.to_string())?;
    let mut components = Vec::new();
    for (i, (yaml, body)) in docs.iter().enumerate() {
        let doc = format!("---\n{}\n---\n{}", yaml, body);
        let default_id = if i == 0 { file_id.to_string() } else { extract_id_from_yaml(yaml).unwrap_or_else(|| format!("{}_{}", file_id, i)) };
        components.push(CuiFileComponent::from_str(&doc, &default_id).map_err(|e| format!("块 {i}: {e}"))?);
    }
    Ok(components)
}

fn extract_id_from_yaml(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.trim().strip_prefix("id:") {
            let id = rest.trim().trim_matches('"').trim_matches('\'');
            if !id.is_empty() { return Some(id.to_string()); }
        }
    }
    None
}

// ── 元组 trait ────────────────────────────────────────────

pub trait ToolPaths { fn collect_paths(self) -> Vec<String>; }
impl ToolPaths for Vec<String> { fn collect_paths(self) -> Vec<String> { self } }

macro_rules! impl_tool_paths {
    ($($T:ident),*) => {
        impl<$($T: AsRef<str>),*> ToolPaths for ($($T,)*) {
            #[allow(non_snake_case)] fn collect_paths(self) -> Vec<String> {
                let ($($T,)*) = self; vec![$($T.as_ref().to_string()),*]
            }
        }
    };
}
impl_tool_paths!(A);
impl_tool_paths!(A, B); impl_tool_paths!(A, B, C); impl_tool_paths!(A, B, C, D);
impl_tool_paths!(A, B, C, D, E); impl_tool_paths!(A, B, C, D, E, F);
impl_tool_paths!(A, B, C, D, E, F, G); impl_tool_paths!(A, B, C, D, E, F, G, H);
impl_tool_paths!(A, B, C, D, E, F, G, H, I); impl_tool_paths!(A, B, C, D, E, F, G, H, I, J);
impl_tool_paths!(A, B, C, D, E, F, G, H, I, J, K); impl_tool_paths!(A, B, C, D, E, F, G, H, I, J, K, L);

pub trait SkillEntries { fn collect_entries(self) -> Vec<(String, String)>; }

macro_rules! impl_skill_entries {
    ($(($N:ident, $D:ident)),*) => {
        impl<$($N: AsRef<str>, $D: AsRef<str>),*> SkillEntries for ($(($N, $D),)*) {
            #[allow(non_snake_case)] fn collect_entries(self) -> Vec<(String, String)> {
                let ($(($N, $D),)*) = self; vec![$(($N.as_ref().to_string(), $D.as_ref().to_string())),*]
            }
        }
    };
}
impl_skill_entries!((A, B)); impl_skill_entries!((A, B), (C, D));
impl_skill_entries!((A, B), (C, D), (E, F)); impl_skill_entries!((A, B), (C, D), (E, F), (G, H));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T), (U, V));
impl_skill_entries!((A, B), (C, D), (E, F), (G, H), (I, J), (K, L), (M, N), (O, P), (Q, R), (S, T), (U, V), (W, X));

// ── test helpers ─────────────────────────────────────────

#[cfg(test)]
pub(crate) fn build_component_tree(components: Vec<CuiFileComponent>) -> Result<ComponentNode, Vec<CompileError>> {
    let entry_id = components.iter().find(|c| c.is_entry() || c.component_kind() == ComponentKind::Group || c.id() == "main")
        .map(|c| c.id().to_string()).unwrap_or_else(|| components.first().map(|c| c.id().to_string()).unwrap_or_default());
    let mut compiler = TestCompiler::new(components)?;
    compiler.resolve_node(&entry_id, &mut vec![]).map_err(|e| vec![e])
}

#[cfg(test)]
fn collect_node_ids(node: &ComponentNode) -> Vec<String> {
    let mut ids = vec![node.id().to_string()];
    if let ComponentNode::Composite { children, .. } = node { for c in children { ids.extend(collect_node_ids(c)); } }
    ids
}

#[cfg(test)]
pub(crate) struct TestCompiler {
    components: HashMap<String, CuiFileComponent>,
    type_registry: TypeRegistry,
}

#[cfg(test)]
impl TestCompiler {
    pub fn new(components: Vec<CuiFileComponent>) -> Result<Self, Vec<CompileError>> {
        let mut map = HashMap::new();
        let mut errors = Vec::new();
        for comp in components {
            if map.contains_key(comp.id()) { errors.push(CompileError::ReferenceNotFound { id: comp.id().to_string(), source_id: "".into() }); }
            map.insert(comp.id().to_string(), comp);
        }
        if !errors.is_empty() { return Err(errors); }
        Ok(Self { components: map, type_registry: builtin_registry() })
    }

    fn resolve_node(&mut self, id: &str, chain: &mut Vec<String>) -> Result<ComponentNode, CompileError> {
        if chain.contains(&id.to_string()) {
            chain.push(id.to_string());
            return Err(CompileError::CircularReference { id: id.to_string(), chain: chain.clone() });
        }
        let comp = self.components.get(id).ok_or_else(|| CompileError::ReferenceNotFound { id: id.to_string(), source_id: chain.last().cloned().unwrap_or_default() })?;
        let resolved = self.resolve_component_type(comp)?;
        let is_group = resolved.kind == ComponentKind::Group || !resolved.children.is_empty();
        if !is_group {
            let leaf = CuiFileLeaf::new(&resolved.id, &resolved.title, &resolved.body)
                .priority(resolved.priority).kind(resolved.kind)
                .with_condition(comp.visibility_condition())
                .with_inputs(comp.inputs().to_vec()).with_outputs(comp.outputs().to_vec());
            let leaf = if resolved.inert { leaf.inert() } else { leaf };
            let leaf = if resolved.is_static { leaf.is_static() } else { leaf };
            let leaf = if let Some(ref s) = resolved.summary { leaf.summary(s.as_str()) } else { leaf };
            let leaf = if let Some(ref pk) = resolved.persist { leaf.persist(pk.as_str()) } else { leaf };
            let leaf = if let Some(ref st) = resolved.subtype { leaf.subtype(st.as_str()) } else { leaf };
            let pairs = merge_input_values(comp, &self.type_registry);
            let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let mut node = leaf.with_input_values(&refs).build();
            node.set_actions(resolved.actions);
            return Ok(node);
        }
        let child_ids: Vec<String> = comp.component_children().to_vec();
        chain.push(id.to_string());
        let mut builder = group(&resolved.id, &resolved.title).priority(resolved.priority).with_condition(comp.visibility_condition());
        if let Some(ratio) = resolved.budget_ratio { builder = builder.ratio(ratio); }
        if resolved.inert { builder = builder.inert(); }
        for cid in &child_ids { let child = self.resolve_node(cid, chain)?; builder = builder.push(child); }
        chain.pop();
        let mut composite = builder.build();
        composite.set_actions(resolved.actions);
        Ok(composite)
    }

    fn resolve_component_type(&self, comp: &CuiFileComponent) -> Result<crate::runtime::registry::ResolvedComponent, CompileError> {
        let Some(type_name) = comp.component_type() else {
            return Ok(crate::runtime::registry::ResolvedComponent {
                id: comp.id().to_string(), title: comp.title().to_string(), kind: comp.component_kind(),
                priority: comp.priority(), summary: comp.summary.clone(), inert: comp.is_inert(),
                is_static: comp.is_static(), actions: comp.actions(), body: comp.render_body(crate::level::RenderLevel::Standard),
                children: comp.component_children().to_vec(), source: comp.component_source().map(|s| s.to_string()),
                persist: comp.persist_key().map(|s| s.to_string()), entry: comp.is_entry(), budget_ratio: comp.budget_ratio(),
                subtype: None,
            });
        };
        self.type_registry.resolve(type_name, comp.id(), comp.title(), Some(comp.component_kind()), Some(comp.priority()),
            &comp.actions(), &comp.render_body(crate::level::RenderLevel::Standard), comp.summary.as_deref(),
            Some(comp.is_inert()), Some(comp.is_static()), comp.handler(), comp.component_children(),
            comp.component_source(), comp.persist_key(), comp.is_entry(), comp.budget_ratio())
            .map_err(|msg| CompileError::ParseError { message: msg })
    }
}

#[cfg(test)]
pub(crate) fn build_tree_nodes(components: Vec<CuiFileComponent>) -> Result<ComponentNode, Vec<CompileError>> {
    let entry_id = components.iter().find(|c| c.is_entry() || c.component_kind() == ComponentKind::Group || c.id() == "main")
        .map(|c| c.id().to_string()).unwrap_or_else(|| components.first().map(|c| c.id().to_string()).unwrap_or_default());
    if components.iter().all(|c| c.id() != entry_id) { return Err(vec![CompileError::ReferenceNotFound { id: entry_id.clone(), source_id: String::new() }]); }
    let mut compiler = TestCompiler::new(components)?;
    compiler.resolve_node(&entry_id, &mut vec![]).map_err(|e| vec![e])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::file::CuiFileComponent; use crate::keyword::ComponentKind;
    fn make_comp(src: &str, id: &str) -> CuiFileComponent { CuiFileComponent::from_str(src, id).unwrap() }

    #[test] fn is_multi_document_true() { assert!(is_multi_document("---\nid: a\ntitle: A\n---\nBody A\n---\nid: b\ntitle: B\n---\nBody B")); }
    #[test] fn is_multi_document_false() { assert!(!is_multi_document("---\ntitle: A\n---\nBody")); }
    #[test] fn parse_multi_two() { let d=parse_multi_document("---\nid: a\ntitle: A\n---\nBA\n---\nid: b\ntitle: B\n---\nBB").unwrap(); assert_eq!(d.len(),2); }
    #[test] fn expand_multi_two() { let c=expand_multi_document("---\nid: parent\ntitle: Parent\nkind: group\nchildren: [child]\n---\n---\nid: child\ntitle: Child\n---\nCB","m").unwrap(); assert_eq!(c.len(),2); assert_eq!(c[0].id(),"parent"); }
    #[test] fn expand_multi_single() { let c=expand_multi_document("---\ntitle: S\n---\nB","s").unwrap(); assert_eq!(c.len(),1); }
    #[test] fn multi_ignore_body_sep() { let d=parse_multi_document("---\nid: a\ntitle: A\n---\nBefore\n---\nAfter\n---\nid: b\ntitle: B\n---\nBB").unwrap(); assert_eq!(d.len(),2); }
    #[test] fn compiler_simple() { let t=TestCompiler::new(vec![make_comp("---\nid: root\ntitle: Root\nkind: group\nchildren: [leaf]\n---\nR","root"),make_comp("---\nid: leaf\ntitle: Leaf\n---\nLB","leaf")]).unwrap().resolve_node("root",&mut vec![]).unwrap(); assert_eq!(t.id(),"root"); }
    #[test] fn compiler_dup() { assert!(TestCompiler::new(vec![make_comp("---\nid: dup\ntitle: First\n---\n","dup"),make_comp("---\nid: dup\ntitle: Second\n---\n","dup")]).is_err()); }
    #[test] fn compiler_circular() { let r=TestCompiler::new(vec![make_comp("---\nid: a\ntitle: A\nkind: group\nchildren: [b]\n---\n","a"),make_comp("---\nid: b\ntitle: B\nkind: group\nchildren: [a]\n---\n","b")]).unwrap().resolve_node("a",&mut vec![]); assert!(r.is_err()); }
    #[test] fn compiler_ref_not_found() { let r=TestCompiler::new(vec![make_comp("---\nid: root\ntitle: Root\nkind: group\nchildren: [ghost]\n---\n","root")]).unwrap().resolve_node("root",&mut vec![]); assert!(r.is_err()); }
    #[test] fn compiler_entry_not_found() { let r=TestCompiler::new(vec![make_comp("---\nid: a\ntitle: A\n---\nbody","a")]).unwrap().resolve_node("nonexistent",&mut vec![]); assert!(r.is_err()); }
    #[test] fn auto_detect_entry() { let t=build_tree_nodes(vec![make_comp("---\nid: a\ntitle: A\nkind: group\n---\nA","a"),make_comp("---\nid: b\ntitle: B\n---\nB","b")]).unwrap(); assert_eq!(t.id(),"a"); }
}

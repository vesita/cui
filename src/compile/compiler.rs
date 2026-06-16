//! CUI 编译器 —— 从 `.cui` 文件构建组件树、解析引用、报告错误。
//!
//! # 使用流程
//!
//! ```ignore
//! let dir = CuiDirectory::new("cui/");
//! let comps = dir.load()?;
//! let tree = build_component_tree(comps)?;
//! ```

use crate::compile::file::CuiFileComponent;
use crate::component::{
    ComponentNode,
    builtin::{CuiFileLeaf, group},
};
use crate::keyword::ComponentKind;
use crate::runtime::registry::{TypeRegistry, builtin_registry};
use std::collections::HashMap;
use std::path::PathBuf;

// ── 编译器错误 ──────────────────────────────────────────────────────

/// 编译器错误类型。
#[derive(Debug, Clone)]
pub enum CompilerError {
    /// 引用的组件 ID 不存在。
    ReferenceNotFound { id: String, source_id: String },
    /// 重复的组件 ID。
    DuplicateId { id: String },
    /// 循环引用（A → B → A）。
    CircularReference { id: String, chain: Vec<String> },
    /// 入口组件未找到。
    EntryNotFound { entry_id: String },
    /// 多文档解析错误。
    ParseError { message: String },
    /// source: 指定了文件但 source_dir 未设置。
    SourceDirNotSet { id: String, source: String },
    /// 未知的组件类型（`type:` 字段值未在 TypeRegistry 中注册）。
    UnknownType {
        type_name: String,
        known_types: Vec<String>,
    },
}

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerError::ReferenceNotFound { id, source_id } => {
                write!(
                    f,
                    "error[REF001]: 组件 '{source_id}' 引用了不存在的组件 '{id}'"
                )
            }
            CompilerError::DuplicateId { id } => {
                write!(f, "error[REF002]: 组件 ID '{id}' 出现重复")
            }
            CompilerError::CircularReference { id, chain } => {
                write!(
                    f,
                    "error[REF003]: 循环引用：组件 '{id}' 引用链: {}",
                    chain.join(" → ")
                )
            }
            CompilerError::EntryNotFound { entry_id } => {
                write!(f, "error[REF004]: 入口组件 '{entry_id}' 未找到")
            }
            CompilerError::ParseError { message } => {
                write!(f, "error[PARSER]: {message}")
            }
            CompilerError::SourceDirNotSet { id, source } => {
                write!(
                    f,
                    "error[S001]: 组件 '{id}' 指定了 source: '{source}' 但 source_dir 未设置"
                )
            }
            CompilerError::UnknownType {
                type_name,
                known_types,
            } => {
                write!(
                    f,
                    "error[T001]: 未知组件类型 '{}'，已知类型：{}",
                    type_name,
                    known_types.join(", ")
                )
            }
        }
    }
}

// ── 多文档解析 ──────────────────────────────────────────────────────

/// 判断 .cui 内容是否为多文档格式。
///
/// 多文档格式标准：至少 2 段 `---\nkey: value` 模式的前言块。
/// 仅靠行首 `---` 计数会误判 body 中的水平线，因此额外验证
/// `---` 后紧跟的行是否以 YAML key 开头。
pub fn is_multi_document(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let mut doc_starts = 0u8;
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i].trim() == "---" {
            // 检查下一行是否像 YAML key（字母开头 + 冒号）
            if i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if is_yaml_key(next) {
                    doc_starts += 1;
                }
            }
        }
        i += 1;
    }
    doc_starts >= 2
}

/// 检查行是否像 YAML 键值对（严格匹配，避免正文中 `---` 后的误判）。
fn is_yaml_key(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return false;
    }
    // 键名：字母/数字/下划线，后紧跟冒号
    if let Some(colon) = trimmed.find(':') {
        let key = &trimmed[..colon];
        key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    } else {
        false
    }
}

/// 解析多文档 `.cui` 内容，返回每个文档的 (frontmatter, body) 对。
///
/// 当 body 中出现单独的 `---` 行时（如 Markdown 水平线），
/// 会向前检查下一行是否符合 YAML 键值对格式，只有符合时才将其视为文档分隔符。
pub fn parse_multi_document(content: &str) -> Result<Vec<(String, String)>, CompilerError> {
    let content = content.trim();
    let lines: Vec<&str> = content.lines().collect();
    let mut docs = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].trim() == "---" {
            i += 1;
            // 读取 YAML frontmatter
            let mut yaml = String::new();
            while i < lines.len() {
                if lines[i].trim() == "---" {
                    i += 1;
                    break;
                }
                if !yaml.is_empty() {
                    yaml.push('\n');
                }
                yaml.push_str(lines[i]);
                i += 1;
            }
            if yaml.is_empty() {
                // 未闭合的 --- 或空 frontmatter
                if docs.is_empty() {
                    return Err(CompilerError::ParseError {
                        message: "文件以 `---` 开头但未找到闭合 `---`".into(),
                    });
                }
                // 非首个文档的 --- 未闭合 → 将其视为 body 内容的一部分
                break;
            }

            // 读取 body（直到下一个文档边界或文件结束）
            let mut body = String::new();
            while i < lines.len() {
                if lines[i].trim() == "---" {
                    // 只有 --- 后紧跟 YAML key 才视为真正的文档边界
                    if i + 1 < lines.len() && is_yaml_key(lines[i + 1].trim()) {
                        break;
                    }
                }
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(lines[i]);
                i += 1;
            }
            docs.push((yaml, body));
        } else {
            i += 1;
        }
    }

    if docs.is_empty() {
        return Err(CompilerError::ParseError {
            message: "未找到任何文档块，文件应以 `---` 开头".into(),
        });
    }

    Ok(docs)
}

/// 展开多文档文件：将 `---` 分隔的多个文档解析为多个 `CuiFileComponent`。
pub fn expand_multi_document(
    content: &str,
    file_id: &str,
) -> Result<Vec<CuiFileComponent>, String> {
    if !is_multi_document(content) {
        return Ok(vec![CuiFileComponent::from_str(content, file_id)?]);
    }

    let docs = parse_multi_document(content).map_err(|e| e.to_string())?;
    let mut components = Vec::new();

    for (i, (yaml, body)) in docs.iter().enumerate() {
        let doc_content = format!("---\n{}\n---\n{}", yaml, body);
        let default_id = if i == 0 {
            file_id.to_string()
        } else {
            extract_id_from_yaml(yaml).unwrap_or_else(|| format!("{}_{}", file_id, i))
        };
        let comp = CuiFileComponent::from_str(&doc_content, &default_id)?;
        components.push(comp);
    }

    Ok(components)
}

/// 从 YAML 字符串中扫描提取 `id` 字段值。
fn extract_id_from_yaml(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("id:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

// ── 引用解析 ──────────────────────────────────────────────────────

/// 编译器：解析组件引用、检测循环、构建组件树。
pub struct Compiler {
    /// 所有可用组件（ID → 组件）。
    components: HashMap<String, CuiFileComponent>,
    /// `source:` 引用解析的根目录。
    source_dir: Option<PathBuf>,
    /// 类型注册表（用于 `type:` 字段解析）。
    type_registry: TypeRegistry,
}

impl Compiler {
    /// 从组件列表创建编译器，自动检测重复 ID。
    /// 默认使用内置类型注册表。
    pub fn new(components: Vec<CuiFileComponent>) -> Result<Self, Vec<CompilerError>> {
        let mut map = HashMap::new();
        let mut errors = Vec::new();

        for comp in components {
            let id = comp.id().to_string();
            match map.entry(id.clone()) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    errors.push(CompilerError::DuplicateId { id });
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(comp);
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(Self {
            components: map,
            source_dir: None,
            type_registry: builtin_registry(),
        })
    }

    /// 设置 `source:` 引用解析的根目录。
    pub fn with_source_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.source_dir = Some(dir.into());
        self
    }

    /// 设置自定义类型注册表（替换默认内置类型）。
    pub fn with_type_registry(mut self, registry: TypeRegistry) -> Self {
        self.type_registry = registry;
        self
    }
}

// ── ComponentNode 输出 ─────────────────────────────────────────────

/// 从组件列表构建 `ComponentNode` 树。
///
/// 返回 `ComponentNode::Composite`（GroupComponent 作为容器 BaseComponent），
/// 叶子节点通过 `ComponentNode::from_text_component()` 桥接。
///
/// 入口检测（按优先级）：
/// 1. `entry: true` 显式声明
/// 2. `kind: group` 或有 `children` 列表
/// 3. id 为 `"main"` 的组件
/// 4. 第一个组件
pub fn build_tree_nodes(
    components: Vec<CuiFileComponent>,
) -> Result<ComponentNode, Vec<CompilerError>> {
    let entry_id = components
        .iter()
        .find(|c| c.is_entry())
        .or_else(|| {
            components.iter().find(|c| {
                c.component_kind() == ComponentKind::Group || !c.component_children().is_empty()
            })
        })
        .or_else(|| components.iter().find(|c| c.id() == "main"))
        .map(|c| c.id().to_string())
        .unwrap_or_else(|| {
            components
                .first()
                .map(|c| c.id().to_string())
                .unwrap_or_default()
        });

    if entry_id.is_empty() {
        return Err(vec![CompilerError::EntryNotFound {
            entry_id: "(空)".into(),
        }]);
    }

    let mut compiler = Compiler::new(components)?;
    let root = compiler
        .resolve_node(&entry_id, &mut vec![])
        .map_err(|e| vec![e])?;

    // 检测未被引用的组件
    let referenced = collect_node_ids(&root);
    let all_ids: Vec<&String> = compiler.components.keys().collect();
    let unreferenced: Vec<&&String> = all_ids
        .iter()
        .filter(|id| !referenced.contains(*id))
        .collect();
    if !unreferenced.is_empty() {
        tracing::warn!(
            "cui 编译警告: {} 个组件未被引用（非 entry 且不在任何 children 列表中）: {}",
            unreferenced.len(),
            unreferenced
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    Ok(root)
}

/// 合并实例 slot 值与类型 slot 默认值（实例优先，类型默认补底）。
fn merge_slot_defaults(comp: &CuiFileComponent, registry: &TypeRegistry) -> Vec<(String, String)> {
    use std::collections::BTreeMap;
    let mut merged: BTreeMap<String, String> = BTreeMap::new();

    // 先填入类型默认值（如果类型定义了该 slot）
    if let Some(type_name) = comp.component_type()
        && let Some(typedef) = registry.lookup(type_name)
    {
        for slot in &typedef.slots {
            if let Some(ref default) = slot.default {
                merged
                    .entry(slot.name.clone())
                    .or_insert_with(|| default.clone());
            }
        }
    }

    // 实例值覆盖类型默认
    for slot in comp.slots() {
        let val = slot.default.as_deref().unwrap_or("");
        merged.insert(slot.name.clone(), val.to_string());
    }

    merged.into_iter().collect()
}

/// 递归收集 ComponentNode 树中所有组件 ID。
fn collect_node_ids(node: &ComponentNode) -> Vec<String> {
    let mut ids = vec![node.id().to_string()];
    if let ComponentNode::Composite { children, .. } = node {
        for child in children {
            ids.extend(collect_node_ids(child));
        }
    }
    ids
}

impl Compiler {
    /// 对组件进行类型解析，返回完整的 `ResolvedComponent`。
    /// 若组件未设置 `type:`，从实例字段构造等价结构。
    fn resolve_component_type(
        &self,
        comp: &CuiFileComponent,
    ) -> Result<crate::runtime::registry::ResolvedComponent, CompilerError> {
        let Some(type_name) = comp.component_type() else {
            return Ok(crate::runtime::registry::ResolvedComponent {
                id: comp.id().to_string(),
                title: comp.title().to_string(),
                kind: comp.component_kind(),
                priority: comp.priority(),
                summary: comp.summary.clone(),
                inert: comp.is_inert(),
                is_static: comp.is_static(),
                actions: comp.actions(),
                body: comp.render_body(crate::level::RenderLevel::Standard),
                children: comp.component_children().to_vec(),
                source: comp.component_source().map(|s| s.to_string()),
                persist: comp.persist_key().map(|s| s.to_string()),
                entry: comp.is_entry(),
                budget_ratio: comp.budget_ratio(),
                subtype: None,
            });
        };

        let instance_summary: Option<&str> = comp.summary.as_deref();
        self.type_registry
            .resolve(
                type_name,
                comp.id(),
                comp.title(),
                Some(comp.component_kind()),
                Some(comp.priority()),
                &comp.actions(),
                &comp.render_body(crate::level::RenderLevel::Standard),
                instance_summary,
                Some(comp.is_inert()),
                Some(comp.is_static()),
                comp.handler(),
                comp.component_children(),
                comp.component_source(),
                comp.persist_key(),
                comp.is_entry(),
                comp.budget_ratio(),
            )
            .map_err(|msg| CompilerError::ParseError { message: msg })
    }

    /// 递归解析单个组件引用为 `ComponentNode`。
    fn resolve_node(
        &mut self,
        id: &str,
        chain: &mut Vec<String>,
    ) -> Result<ComponentNode, CompilerError> {
        if chain.contains(&id.to_string()) {
            chain.push(id.to_string());
            return Err(CompilerError::CircularReference {
                id: id.to_string(),
                chain: chain.clone(),
            });
        }

        let comp = self
            .components
            .get(id)
            .ok_or_else(|| CompilerError::ReferenceNotFound {
                id: id.to_string(),
                source_id: chain.last().cloned().unwrap_or_default(),
            })?;

        // 类型解析：合并 type 默认值与实例字段
        let resolved = self.resolve_component_type(comp)?;

        let is_group = resolved.kind == ComponentKind::Group || !resolved.children.is_empty();

        if !is_group {
            let leaf = CuiFileLeaf::new(&resolved.id, &resolved.title, &resolved.body)
                .priority(resolved.priority)
                .kind(resolved.kind)
                .with_inputs(comp.inputs().to_vec())
                .with_outputs(comp.outputs().to_vec());
            let leaf = if resolved.inert { leaf.inert() } else { leaf };
            let leaf = if resolved.is_static {
                leaf.is_static()
            } else {
                leaf
            };
            let leaf = if let Some(ref s) = resolved.summary {
                leaf.summary(s.as_str())
            } else {
                leaf
            };
            let leaf = if let Some(ref pk) = resolved.persist {
                leaf.persist(pk.as_str())
            } else {
                leaf
            };
            let leaf = if let Some(ref st) = resolved.subtype {
                leaf.subtype(st.as_str())
            } else {
                leaf
            };
            // 合并实例 slot 值与类型 slot 默认值（实例优先）
            let slot_pairs: Vec<(String, String)> = merge_slot_defaults(comp, &self.type_registry);
            let slot_refs: Vec<(&str, &str)> = slot_pairs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let leaf = leaf.with_slots(&slot_refs);
            let mut node = leaf.build();
            node.set_actions(resolved.actions);
            return Ok(node);
        }

        // 容器节点：构建 Composite
        let child_ids: Vec<String> = comp.component_children().to_vec();
        chain.push(id.to_string());

        let mut builder = group(&resolved.id, &resolved.title)
            .priority(resolved.priority)
            .with_condition(comp.visibility_condition());

        if let Some(ratio) = resolved.budget_ratio {
            builder = builder.ratio(ratio);
        }

        if resolved.inert {
            builder = builder.inert();
        }

        for child_id in &child_ids {
            let child = self.resolve_node(child_id, chain)?;
            builder = builder.push(child);
        }

        chain.pop();
        let mut composite = builder.build();
        composite.set_actions(resolved.actions);
        Ok(composite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::file::CuiFileComponent;

    fn make_comp(src: &str, id: &str) -> CuiFileComponent {
        CuiFileComponent::from_str(src, id).unwrap()
    }

    // ── 多文档解析 ──────────────────────────────────────────────

    #[test]
    fn is_multi_document_true() {
        let content = "---\nid: a\ntitle: A\n---\nBody A\n---\nid: b\ntitle: B\n---\nBody B";
        assert!(is_multi_document(content));
    }

    #[test]
    fn is_multi_document_false_for_single() {
        let content = "---\ntitle: A\n---\nBody";
        assert!(!is_multi_document(content));
    }

    #[test]
    fn parse_multi_document_two_docs() {
        let content = "---\nid: a\ntitle: A\n---\nBody A\n---\nid: b\ntitle: B\n---\nBody B";
        let docs = parse_multi_document(content).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].0, "id: a\ntitle: A"); // yaml
        assert_eq!(docs[0].1, "Body A"); // body
        assert_eq!(docs[1].0, "id: b\ntitle: B");
        assert_eq!(docs[1].1, "Body B");
    }

    #[test]
    fn expand_multi_document_two_components() {
        let content = "---\nid: parent\ntitle: Parent\nkind: group\nchildren: [child]\n---\n---\nid: child\ntitle: Child\n---\nChild Body";
        let comps = expand_multi_document(content, "main").unwrap();
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0].id(), "parent");
        assert_eq!(comps[0].component_kind(), ComponentKind::Group);
        assert_eq!(comps[1].id(), "child");
        assert_eq!(
            comps[1].render_body(crate::level::RenderLevel::Standard),
            "Child Body"
        );
    }

    #[test]
    fn expand_multi_document_single_falls_back() {
        let content = "---\ntitle: Single\n---\nJust body";
        let comps = expand_multi_document(content, "single").unwrap();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].title(), "Single");
    }

    #[test]
    fn parse_multi_document_ignores_body_separator() {
        // body 中的 --- 行（Markdown 水平线）不应被误判为文档边界
        let content =
            "---\nid: a\ntitle: A\n---\nBefore\n---\nAfter\n---\nid: b\ntitle: B\n---\nBody B";
        let docs = parse_multi_document(content).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].0, "id: a\ntitle: A");
        assert_eq!(docs[0].1, "Before\n---\nAfter");
        assert_eq!(docs[1].0, "id: b\ntitle: B");
        assert_eq!(docs[1].1, "Body B");
    }

    // ── 编译器 ──────────────────────────────────────────────────

    #[test]
    fn compiler_builds_simple_tree() {
        let comps = vec![
            make_comp(
                "---\nid: root\ntitle: Root\nkind: group\nchildren: [leaf]\n---\nRoot",
                "root",
            ),
            make_comp("---\nid: leaf\ntitle: Leaf\n---\nLeaf Body", "leaf"),
        ];
        let tree = Compiler::new(comps)
            .unwrap()
            .resolve_node("root", &mut vec![])
            .unwrap();
        assert_eq!(tree.id(), "root");
    }

    #[test]
    fn compiler_duplicate_id_detected() {
        let comps = vec![
            make_comp("---\nid: dup\ntitle: First\n---\nFirst", "dup"),
            make_comp("---\nid: dup\ntitle: Second\n---\nSecond", "dup"),
        ];
        assert!(Compiler::new(comps).is_err());
    }

    #[test]
    fn compiler_reference_not_found() {
        let comps = vec![make_comp(
            "---\nid: root\ntitle: Root\nkind: group\nchildren: [missing]\n---\nRoot",
            "root",
        )];
        let result = Compiler::new(comps)
            .unwrap()
            .resolve_node("root", &mut vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn compiler_circular_reference_detected() {
        let comps = vec![
            make_comp(
                "---\nid: a\ntitle: A\nkind: group\nchildren: [b]\n---\nA",
                "a",
            ),
            make_comp(
                "---\nid: b\ntitle: B\nkind: group\nchildren: [a]\n---\nB",
                "b",
            ),
        ];
        let result = Compiler::new(comps).unwrap().resolve_node("a", &mut vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn compiler_entry_not_found() {
        let comps = vec![make_comp("---\nid: leaf\ntitle: Leaf\n---\nLeaf", "leaf")];
        let result = Compiler::new(comps)
            .unwrap()
            .resolve_node("nonexistent", &mut vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn build_component_tree_auto_detects_entry() {
        let comps = vec![
            make_comp(
                "---\nid: main\ntitle: Main\nkind: group\nchildren: [tool1]\n---\nMain",
                "main",
            ),
            make_comp(
                "---\nid: tool1\ntitle: Tool1\nkind: block\n---\nTool1",
                "tool1",
            ),
        ];
        let tree = build_tree_nodes(comps).unwrap();
        assert_eq!(tree.id(), "main");
    }
}

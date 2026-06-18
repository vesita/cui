//! 工具适配器 —— 从 .cui 文件构建工具组件节点。
//!
//! 重型适配见 [`crate::compile::Compiler`]。

use crate::compile::compiler::resolve_tool;
use crate::compile::file::CuiFileComponent;
use crate::component::builtin::group;
use crate::component::ComponentNode;
use crate::runtime::registry::TypeRegistry;
use crate::keyword::PriorityLevel;

/// 从 .cui 文件构建单个工具组件节点。
pub fn tool_node(path: &str, registry: &TypeRegistry) -> Option<ComponentNode> {
    let comp = match CuiFileComponent::from_file(path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("加载工具文件失败: {path}: {e}");
            return None;
        }
    };
    let mut node = resolve_tool(&comp, registry);
    if comp.collapsible() {
        node.set_collapsible(true);
        node.set_collapsed(comp.collapsed());
    }
    Some(node)
}

/// 构建工具分组容器组件节点。
pub fn tool_section(
    id: &str,
    label: &str,
    priority: PriorityLevel,
    budget_ratio: Option<f32>,
    _trigger: Option<&str>,
    collapsed: bool,
    components: Vec<ComponentNode>,
) -> ComponentNode {
    let mut g = group(id, label)
        .priority(priority)
        .collapsible()
        .collapsed(collapsed);
    if let Some(ratio) = budget_ratio {
        g = g.ratio(ratio);
    }
    for node in components {
        g = g.push(node);
    }
    g.build()
}

/// 从多文档 YAML frontmatter 字符串解析多个 [`CuiFileComponent`]。
///
/// 这是 [`crate::expand_multi_document`] 的便捷封装，
/// 默认 id 为 `"doc"`，解析失败时返回空列表。
pub fn parse_multi_cui(content: &str) -> Vec<CuiFileComponent> {
    crate::expand_multi_document(content, "doc").unwrap_or_default()
}

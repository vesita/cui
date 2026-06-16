//! 工具适配器 —— 从 .cui 模板文件直接构建 CUI 组件节点。
//!
//! 每个工具 .cui 文件通过 `type: tool` 声明类型，适配器将其包装为
//! collapsible Group + Body 节点。动作列表由类型系统自动提供。

use crate::action::ActionDef;
use crate::component::builtin::{Body, group};
use crate::runtime::registry::TypeRegistry;
use crate::{ComponentNode, CuiFileComponent, PriorityLevel, RenderLevel, VisibilityCondition};

/// 从 .cui 文件加载单个工具节点（collapsible Group + Body）。
///
/// `cui_path` 为 `.cui` 文件的完整路径。
/// 若 .cui 文件声明了 `type:`，通过类型系统解析动作列表；
/// 否则使用文件自身的 actions。
pub fn tool_node(cui_path: &str, registry: &TypeRegistry) -> Option<ComponentNode> {
    let comp = CuiFileComponent::from_file(cui_path).ok()?;
    let actions = resolve_actions(&comp, registry);
    let body_id = format!("{}_body", comp.id());
    let mut builder = group(comp.id(), comp.title()).priority(comp.priority());
    if comp.is_inert() {
        builder = builder.inert();
    }
    if comp.collapsible() {
        builder = builder.collapsible().collapsed(comp.collapsed());
    }
    let mut node = builder
        .push(Body::new(&body_id, comp.render_body(RenderLevel::Standard)).build())
        .build();
    node.set_actions(actions);
    Some(node)
}

/// 解析组件动作列表：优先通过类型系统，回退到文件自身定义。
fn resolve_actions(comp: &CuiFileComponent, registry: &TypeRegistry) -> Vec<ActionDef> {
    let Some(type_name) = comp.component_type() else {
        return comp.actions();
    };
    registry
        .resolve(
            type_name,
            comp.id(),
            comp.title(),
            Some(comp.component_kind()),
            Some(comp.priority()),
            &comp.actions(),
            &comp.render_body(RenderLevel::Standard),
            comp.summary.as_deref(),
            Some(comp.is_inert()),
            Some(comp.is_static()),
            comp.handler(),
            comp.component_children(),
            comp.component_source(),
            comp.persist_key(),
            comp.is_entry(),
            comp.budget_ratio(),
        )
        .map(|r| r.actions)
        .unwrap_or_else(|e| {
            tracing::warn!(
                type_name = comp.component_type(),
                component_id = comp.id(),
                error = %e,
                "类型解析失败，回退到文件自身 actions"
            );
            comp.actions()
        })
}

/// 构建工具分组容器节点。
///
/// 将多个工具节点包装在一个 collapsible Group 中。
pub fn tool_section(
    id: &str,
    label: &str,
    priority: PriorityLevel,
    budget_ratio: Option<f32>,
    condition: Option<VisibilityCondition>,
    collapsed: bool,
    tools: Vec<ComponentNode>,
) -> ComponentNode {
    let mut builder = group(id, label)
        .priority(priority)
        .collapsible()
        .collapsed(collapsed);
    if let Some(ratio) = budget_ratio {
        builder = builder.ratio(ratio);
    }
    if let Some(cond) = condition {
        builder = builder.with_condition(cond);
    }
    for tool in tools {
        builder = builder.push(tool);
    }
    builder.build()
}

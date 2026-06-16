//! 分组组件 —— 逻辑分组容器，无自身渲染内容，仅用于组织子节点。
//!
//! 通过 [`GroupBuilder`] 构建，支持预算比例分配。

use crate::PriorityLevel;
use crate::action::{ActionResult, ActionVariant};
use crate::component::{BaseComponent, ComponentNode};
use crate::condition::VisibilityCondition;
use crate::level::RenderLevel;

// ── GroupBuilder ──────────────────────────────────────────

/// 快速创建分组复合节点的构建器。
pub struct GroupBuilder {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) children: Vec<ComponentNode>,
    pub(crate) budget_ratio: Option<f32>,
    pub(crate) condition: VisibilityCondition,
    pub(crate) inert: bool,
    pub(crate) collapsible: bool,
    pub(crate) collapsed: bool,
}

impl GroupBuilder {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            priority: PriorityLevel::Normal,
            children: Vec::new(),
            budget_ratio: None,
            condition: VisibilityCondition::Always,
            inert: false,
            collapsible: false,
            collapsed: true,
        }
    }

    pub fn priority(mut self, p: PriorityLevel) -> Self {
        self.priority = p;
        self
    }
    pub fn ratio(mut self, r: f32) -> Self {
        self.budget_ratio = Some(r.clamp(0.0, 1.0));
        self
    }
    pub fn with_condition(mut self, c: VisibilityCondition) -> Self {
        self.condition = c;
        self
    }
    pub fn inert(mut self) -> Self {
        self.inert = true;
        self
    }
    /// 标记为可折叠分组：默认折叠（仅显示标题），AI 可通过 expand_group 展开子节点。
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }
    /// 设为 `false` 时即使可折叠也不自动折叠，初始展示完整内容。
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    pub fn push(mut self, child: ComponentNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn build(self) -> ComponentNode {
        let mut node = ComponentNode::composite(
            GroupComponent {
                id: self.id,
                title: self.title,
                priority: self.priority,
                condition: self.condition,
                inert: self.inert,
            },
            self.children,
        );
        if let ComponentNode::Composite {
            ref mut budget_ratio,
            ..
        } = node
        {
            *budget_ratio = self.budget_ratio;
        }
        node.set_collapsible(self.collapsible);
        node.set_collapsed(self.collapsed);
        if self.collapsible && self.collapsed {
            node.set_level(RenderLevel::Summary);
        }
        node
    }
}

// ── GroupComponent ────────────────────────────────────────

/// 分组组件（内部实现，通过 GroupBuilder 使用）。
pub(crate) struct GroupComponent {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) condition: VisibilityCondition,
    pub(crate) inert: bool,
}

impl BaseComponent for GroupComponent {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn priority(&self) -> PriorityLevel {
        self.priority
    }
    fn is_inert(&self) -> bool {
        self.inert
    }
    fn visibility_condition(&self) -> VisibilityCondition {
        self.condition.clone()
    }

    fn action_variants(&self) -> &'static [ActionVariant] {
        static GROUP_ACTIONS: &[ActionVariant] = &[
            ActionVariant::new("expand_group", "展开此分组"),
            ActionVariant::new("collapse_group", "折叠此分组"),
        ];
        GROUP_ACTIONS
    }

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Title => String::new(),
            RenderLevel::Summary => self.title.clone(),
            RenderLevel::Standard | RenderLevel::Detailed => String::new(),
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        match action {
            "expand_group" => ActionResult::new(&self.id, action.to_string())
                .with_message("分组已展开")
                .with_new_level(RenderLevel::Standard),
            "collapse_group" => ActionResult::new(&self.id, action.to_string())
                .with_message("分组已折叠")
                .with_new_level(RenderLevel::Summary),
            _ => ActionResult::error(&self.id, action, format!("未知动作: {action}")),
        }
    }
}

/// 快速创建分组复合节点的工厂函数。
pub fn group(id: &str, title: &str) -> GroupBuilder {
    GroupBuilder::new(id, title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentNode;
    use crate::component::builtin::text_block;
    use crate::condition::VisibilityCondition;
    use crate::keyword::PriorityLevel;
    use crate::level::RenderLevel;

    #[test]
    fn group_builder_new() {
        let gb = GroupBuilder::new("g", "分组");
        assert_eq!(gb.id, "g");
        assert_eq!(gb.title, "分组");
    }

    #[test]
    fn group_builder_push() {
        let child = text_block("c", "子", "内容");
        let gb = GroupBuilder::new("g", "分组").push(child);
        assert_eq!(gb.children.len(), 1);
    }

    #[test]
    fn group_builder_build_returns_composite() {
        let child = text_block("c", "子", "内容");
        let node = GroupBuilder::new("g", "分组").push(child).build();
        match &node {
            ComponentNode::Composite { .. } => {}
            _ => panic!("build() should return Composite"),
        }
        assert_eq!(node.id(), "g");
    }

    #[test]
    fn group_builder_with_ratio() {
        let child = text_block("c", "子", "内容");
        let node = GroupBuilder::new("g", "分组")
            .ratio(0.5)
            .push(child)
            .build();
        match &node {
            ComponentNode::Composite { budget_ratio, .. } => {
                assert_eq!(*budget_ratio, Some(0.5));
            }
            _ => panic!("not composite"),
        }
    }

    #[test]
    fn group_builder_ratio_clamps() {
        assert_eq!(
            GroupBuilder::new("g", "分组").ratio(1.5).budget_ratio,
            Some(1.0)
        );
        assert_eq!(
            GroupBuilder::new("g", "分组").ratio(-0.1).budget_ratio,
            Some(0.0)
        );
    }

    #[test]
    fn group_builder_builder_methods() {
        let node = GroupBuilder::new("g", "分组")
            .priority(PriorityLevel::Critical)
            .with_condition(VisibilityCondition::when("review"))
            .inert()
            .push(text_block("c", "子", "内容"))
            .build();
        assert_eq!(node.priority(), PriorityLevel::Critical);
        assert_eq!(
            node.visibility_condition(),
            VisibilityCondition::when("review")
        );
    }

    #[test]
    fn group_component_actions() {
        let child = text_block("c", "子", "内容");
        let node = super::group("g", "分组").push(child).build();
        let actions = node.actions(RenderLevel::Standard);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].id(), "expand_group");
        assert_eq!(actions[1].id(), "collapse_group");
    }

    #[test]
    fn group_component_expand_action() {
        let child = text_block("c", "子", "内容");
        let mut node = super::group("g", "分组").push(child).build();
        let result = node.handle_action("expand_group", "");
        assert!(result.is_success());
        assert_eq!(result.new_level(), Some(RenderLevel::Standard));
    }

    #[test]
    fn group_component_collapse_action() {
        let child = text_block("c", "子", "内容");
        let mut node = super::group("g", "分组").push(child).build();
        let result = node.handle_action("collapse_group", "");
        assert!(result.is_success());
        assert_eq!(result.new_level(), Some(RenderLevel::Summary));
    }

    #[test]
    fn group_factory_function() {
        let child = text_block("c", "子", "内容");
        let node = super::group("g", "分组").push(child).build();
        assert_eq!(node.id(), "g");
        assert_eq!(node.title(), "分组");
        match &node {
            ComponentNode::Composite { children, .. } => {
                assert_eq!(children.len(), 1);
            }
            _ => panic!("not composite"),
        }
    }

    #[test]
    fn group_through_tree() {
        let mut tree = crate::component::ComponentTree::new();
        let node = super::group("section", "分区")
            .push(text_block("a", "A", "内容 A"))
            .push(text_block("b", "B", "内容 B"))
            .build();
        tree.push(node);
        let output = tree.render(500, None, 0);
        assert!(output.contains("[section]"));
        assert!(output.contains("[a]"));
        assert!(output.contains("[b]"));
    }
}

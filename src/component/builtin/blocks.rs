//! 内置叶块组件 —— TextBlock、ConditionalBlock、ListBlock。
//!
//! 每个组件实现 [`BaseComponent`](crate::component::BaseComponent)，
//! 通过 `ComponentNode::leaf()` 即可使用。

use std::fmt::Write;

use crate::PriorityLevel;
use crate::action::{ActionResult, ActionVariant};
use crate::component::{BaseComponent, ComponentNode};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::level::RenderLevel;

// ── TextBlock ─────────────────────────────────────────────

/// 静态文本块 —— 标题 + 内容的简单组合。
pub struct TextBlock {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) content: String,
    pub(crate) condition: VisibilityCondition,
    pub(crate) inert: bool,
    pub(crate) collapsible: bool,
    pub(crate) collapsed: bool,
}

impl TextBlock {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            priority: PriorityLevel::Normal,
            content: content.into(),
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
    pub fn with_condition(mut self, c: VisibilityCondition) -> Self {
        self.condition = c;
        self
    }
    /// 向已有内容追加文本（用于 builder 链）。
    pub fn with_content(mut self, text: &str) -> Self {
        self.content.push_str(text);
        self
    }
    /// 向已有内容追加文本（用于 closure 回调，如 `section_with`）。
    pub fn append_content(&mut self, text: &str) {
        self.content.push_str(text);
    }
    pub fn inert(mut self) -> Self {
        self.inert = true;
        self
    }
    /// 标记为可折叠：无交互时自动收起到 Summary（标题+首行），
    /// 数据更新或手动展开后恢复完整内容。
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }
    /// 设为 `false` 时即使可折叠也不自动折叠，初始展示完整内容。
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    pub fn build(self) -> ComponentNode {
        let collapsed = self.collapsed;
        let collapsible = self.collapsible;
        let mut node = ComponentNode::leaf(self);
        node.set_collapsible(collapsible);
        node.set_collapsed(collapsed);
        if collapsible && collapsed {
            node.set_level(RenderLevel::Summary);
        }
        node
    }
}

impl BaseComponent for TextBlock {
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

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Title => String::new(),
            RenderLevel::Summary => self
                .content
                .lines()
                .next()
                .map(|l| l.to_string())
                .unwrap_or_default(),
            RenderLevel::Standard | RenderLevel::Detailed => self.content.clone(),
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, "TextBlock has no actions")
    }
}

// ── ConditionalBlock ──────────────────────────────────────

/// 条件显示块 —— 通过 show/hide 动作控制渲染级别。
///
/// 初始默认可见（Standard 级别）。show/hide 通过改变组件在树中的级别实现。
pub struct ConditionalBlock {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) content: String,
    pub(crate) condition: VisibilityCondition,
    pub(crate) inert: bool,
    pub(crate) collapsible: bool,
    pub(crate) collapsed: bool,
}

impl ConditionalBlock {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            priority: PriorityLevel::Normal,
            content: content.into(),
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
    pub fn with_condition(mut self, c: VisibilityCondition) -> Self {
        self.condition = c;
        self
    }
    pub fn inert(mut self) -> Self {
        self.inert = true;
        self
    }
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }
    /// 设为 `false` 时即使可折叠也不自动折叠，初始展示完整内容。
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    pub fn build(self) -> ComponentNode {
        let collapsed = self.collapsed;
        let collapsible = self.collapsible;
        let mut node = ComponentNode::leaf(self);
        node.set_collapsible(collapsible);
        node.set_collapsed(collapsed);
        if collapsible && collapsed {
            node.set_level(RenderLevel::Summary);
        }
        node
    }
}

impl BaseComponent for ConditionalBlock {
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
        static ACTIONS: &[ActionVariant] = &[
            ActionVariant::new("show", "显示"),
            ActionVariant::new("hide", "隐藏"),
        ];
        ACTIONS
    }

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Title => String::new(),
            RenderLevel::Summary => self
                .content
                .lines()
                .next()
                .map(|l| l.to_string())
                .unwrap_or_default(),
            RenderLevel::Standard | RenderLevel::Detailed => self.content.clone(),
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        match action {
            "show" => ActionResult::new(&self.id, action.to_string())
                .with_new_level(RenderLevel::Standard)
                .with_snapshot(self.render(RenderLevel::Standard)),
            "hide" => ActionResult::new(&self.id, action.to_string())
                .with_new_level(RenderLevel::Hidden)
                .with_snapshot(String::new()),
            _ => ActionResult::error(&self.id, action, "ConditionalBlock unknown action"),
        }
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => self.content = data.to_string(),
            DataMode::Append => {
                if !self.content.is_empty() {
                    self.content.push('\n');
                }
                self.content.push_str(data);
            }
            DataMode::Clear => self.content.clear(),
        }
    }
}

// ── ListBlock ──────────────────────────────────────────────

/// 列表块 —— 将结构化的条目列表渲染为 Markdown 列表。
pub struct ListBlock {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) items: Vec<String>,
    pub(crate) condition: VisibilityCondition,
    pub(crate) inert: bool,
    pub(crate) collapsible: bool,
    pub(crate) collapsed: bool,
}

impl ListBlock {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            priority: PriorityLevel::Normal,
            items: Vec::new(),
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
    pub fn with_condition(mut self, c: VisibilityCondition) -> Self {
        self.condition = c;
        self
    }
    pub fn inert(mut self) -> Self {
        self.inert = true;
        self
    }
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }
    /// 设为 `false` 时即使可折叠也不自动折叠，初始展示完整内容。
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    pub fn items(mut self, items: impl IntoIterator<Item = String>) -> Self {
        self.items = items.into_iter().collect();
        self
    }
    pub fn build(self) -> ComponentNode {
        let collapsed = self.collapsed;
        let collapsible = self.collapsible;
        let mut node = ComponentNode::leaf(self);
        node.set_collapsible(collapsible);
        node.set_collapsed(collapsed);
        if collapsible && collapsed {
            node.set_level(RenderLevel::Summary);
        }
        node
    }
}

impl BaseComponent for ListBlock {
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

    fn render(&self, level: RenderLevel) -> String {
        if self.items.is_empty() {
            return String::new();
        }
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Summary => {
                let count = self.items.len().min(2);
                let mut out = String::new();
                for item in &self.items[..count] {
                    let _ = writeln!(out, "- {}", item);
                }
                if self.items.len() > 2 {
                    let _ = writeln!(out, "  ... 共 {} 项", self.items.len());
                }
                out
            }
            RenderLevel::Title => String::new(),
            RenderLevel::Standard | RenderLevel::Detailed => {
                let mut out = String::new();
                for item in &self.items {
                    let _ = writeln!(out, "- {}", item);
                }
                out
            }
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, "ListBlock has no actions")
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => {
                self.items.clear();
                for line in data.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.items.push(trimmed.to_string());
                    }
                }
            }
            DataMode::Append => {
                for line in data.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.items.push(trimmed.to_string());
                    }
                }
            }
            DataMode::Clear => self.items.clear(),
        }
    }
}

// ── 工厂函数 ─────────────────────────────────────────────

/// 快速创建 TextBlock 的工厂函数。
pub fn text_block(id: &str, title: &str, content: &str) -> ComponentNode {
    TextBlock::new(id, title, content).build()
}

/// 快速创建 ConditionalBlock 的工厂函数（初始可见）。
pub fn conditional_block(id: &str, title: &str, content: &str) -> ComponentNode {
    ConditionalBlock::new(id, title, content).build()
}

/// 快速创建 ConditionalBlock 的工厂函数（初始隐藏）。
pub fn hidden_block(id: &str, title: &str, content: &str) -> ComponentNode {
    let mut node = ConditionalBlock::new(id, title, content).build();
    node.set_level(RenderLevel::Hidden);
    node
}

/// 快速创建 ListBlock 的工厂函数。
pub fn list_block(id: &str, title: &str) -> ComponentNode {
    ListBlock::new(id, title).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentNode;
    use crate::data::DataMode;
    use crate::keyword::PriorityLevel;
    use crate::level::RenderLevel;

    // ── TextBlock 测试 ─────────────────────────────────────

    #[test]
    fn text_block_new() {
        let tb = TextBlock::new("t", "标题", "内容");
        assert_eq!(tb.id(), "t");
        assert_eq!(tb.title(), "标题");
        assert_eq!(BaseComponent::priority(&tb), PriorityLevel::Normal);
        assert!(!tb.is_inert());
    }

    #[test]
    fn text_block_defaults() {
        let tb = TextBlock::new("t", "T", "C");
        assert_eq!(BaseComponent::priority(&tb), PriorityLevel::Normal);
        assert!(!tb.is_inert());
    }

    #[test]
    fn text_block_priority() {
        let tb = TextBlock::new("t", "T", "C").priority(PriorityLevel::Critical);
        assert_eq!(BaseComponent::priority(&tb), PriorityLevel::Critical);
    }

    #[test]
    fn text_block_render_respects_level() {
        let tb = TextBlock::new("t", "标题", "内容1\n内容2\n内容3");
        assert_eq!(tb.render(RenderLevel::Hidden), "");
        assert_eq!(tb.render(RenderLevel::Title), "");
        assert_eq!(tb.render(RenderLevel::Summary), "内容1");
        assert_eq!(tb.render(RenderLevel::Standard), "内容1\n内容2\n内容3");
        assert_eq!(tb.render(RenderLevel::Detailed), "内容1\n内容2\n内容3");
    }

    #[test]
    fn text_block_handle_action_errors() {
        let mut tb = TextBlock::new("t", "T", "C");
        let result = tb.handle_action("expand", "");
        assert!(!result.is_success());
    }

    #[test]
    fn text_block_build_returns_leaf() {
        let node = TextBlock::new("t", "T", "C").build();
        match &node {
            ComponentNode::Leaf { .. } => {}
            _ => panic!("not leaf"),
        }
    }

    #[test]
    fn text_block_builder_methods() {
        let node = TextBlock::new("t", "T", "C")
            .priority(PriorityLevel::High)
            .inert()
            .build();
        assert_eq!(node.priority(), PriorityLevel::High);
        assert!(node.is_inert());
    }

    #[test]
    fn text_block_inert_flag_through_node() {
        let node = TextBlock::new("t", "T", "C").inert().build();
        assert!(node.is_inert());
    }

    #[test]
    fn text_block_factory() {
        let node = text_block("t", "T", "C");
        assert_eq!(node.id(), "t");
        assert_eq!(node.priority(), PriorityLevel::Normal);
    }

    #[test]
    fn text_block_through_tree() {
        let mut tree = crate::component::ComponentTree::new();
        tree.push(TextBlock::new("t", "标题", "内容").build());
        let rendered = tree.render(9999, None, 0);
        assert!(rendered.contains("标题"));
        assert!(rendered.contains("内容"));
    }

    // ── ConditionalBlock 测试 ──────────────────────────────

    #[test]
    fn conditional_block_new() {
        let cb = ConditionalBlock::new("c", "条件", "内容");
        assert_eq!(cb.id(), "c");
    }

    #[test]
    fn conditional_block_render_hidden_returns_empty() {
        let cb = ConditionalBlock::new("c", "条件", "内容");
        assert_eq!(cb.render(RenderLevel::Hidden), "");
    }

    #[test]
    fn conditional_block_show_action_returns_level() {
        let mut cb = ConditionalBlock::new("c", "条件", "内容");
        let result = cb.handle_action("show", "");
        assert!(result.is_success());
        assert_eq!(result.new_level(), Some(RenderLevel::Standard));
        assert_eq!(result.snapshot(), Some("内容"));
    }

    #[test]
    fn conditional_block_hide_action_returns_level() {
        let mut cb = ConditionalBlock::new("c", "条件", "内容");
        let result = cb.handle_action("hide", "");
        assert!(result.is_success());
        assert_eq!(result.new_level(), Some(RenderLevel::Hidden));
        assert_eq!(result.snapshot(), Some(""));
    }

    #[test]
    fn conditional_block_unknown_action() {
        let mut cb = ConditionalBlock::new("c", "条件", "内容");
        let result = cb.handle_action("unknown", "");
        assert!(!result.is_success());
    }

    #[test]
    fn conditional_block_write_append() {
        let mut cb = ConditionalBlock::new("c", "条件", "第一部分");
        cb.write(DataMode::Append, "第二部分");
        assert_eq!(cb.render(RenderLevel::Standard), "第一部分\n第二部分");
    }

    #[test]
    fn conditional_block_write_overwrite() {
        let mut cb = ConditionalBlock::new("c", "条件", "初始");
        cb.write(DataMode::Overwrite, "更新");
        assert_eq!(cb.render(RenderLevel::Standard), "更新");
    }

    #[test]
    fn conditional_block_write_clear() {
        let mut cb = ConditionalBlock::new("c", "条件", "内容");
        cb.write(DataMode::Clear, "");
        assert_eq!(cb.render(RenderLevel::Standard), "");
    }

    #[test]
    fn conditional_block_show_hide_through_node() {
        let mut node = ConditionalBlock::new("c", "条件", "内容").build();
        let body = node.render_body_only(RenderLevel::Standard);
        assert_eq!(body, "内容");
        node.set_level(RenderLevel::Hidden);
        assert_eq!(node.level(), RenderLevel::Hidden);
        assert_eq!(node.render_body_only(RenderLevel::Hidden), "");
        node.set_level(RenderLevel::Standard);
        assert_eq!(node.render_body_only(RenderLevel::Standard), "内容");
    }

    #[test]
    fn conditional_block_render_title_always_empty() {
        let cb = ConditionalBlock::new("c", "条件", "内容");
        assert_eq!(cb.render(RenderLevel::Title), "");
    }

    #[test]
    fn conditional_block_render_summary_returns_first_line() {
        let cb = ConditionalBlock::new("c", "条件", "第一行\n第二行");
        assert_eq!(cb.render(RenderLevel::Summary), "第一行");
    }

    #[test]
    fn conditional_block_render_standard_returns_content() {
        let cb = ConditionalBlock::new("c", "条件", "内容");
        assert_eq!(cb.render(RenderLevel::Standard), "内容");
    }

    #[test]
    fn conditional_block_build_returns_leaf() {
        let node = ConditionalBlock::new("c", "条件", "内容").build();
        match &node {
            ComponentNode::Leaf { .. } => {}
            _ => panic!("not leaf"),
        }
    }

    #[test]
    fn conditional_block_builder_methods() {
        let node = ConditionalBlock::new("c", "条件", "内容")
            .priority(PriorityLevel::High)
            .inert()
            .build();
        assert_eq!(node.priority(), PriorityLevel::High);
        assert!(node.is_inert());
    }

    #[test]
    fn conditional_block_factory() {
        let node = conditional_block("c", "条件", "内容");
        assert_eq!(node.id(), "c");
    }

    #[test]
    fn conditional_block_action_variants() {
        let cb = ConditionalBlock::new("c", "条件", "内容");
        assert_eq!(cb.action_variants().len(), 2);
    }

    #[test]
    fn conditional_block_actions_through_node() {
        let node = ConditionalBlock::new("c", "条件", "内容").build();
        let actions = node.actions(RenderLevel::Standard);
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn conditional_block_through_tree() {
        let mut tree = crate::component::ComponentTree::new();
        tree.push(ConditionalBlock::new("c", "条件", "内容").build());
        let rendered = tree.render(9999, None, 0);
        assert!(rendered.contains("条件"));
        assert!(rendered.contains("内容"));
    }

    // ── ListBlock 测试 ─────────────────────────────────────

    #[test]
    fn list_block_new() {
        let lb = ListBlock::new("l", "列表");
        assert_eq!(lb.id(), "l");
        assert!(lb.items.is_empty());
    }

    #[test]
    fn list_block_render_empty() {
        let lb = ListBlock::new("l", "列表");
        assert_eq!(lb.render(RenderLevel::Standard), "");
    }

    #[test]
    fn list_block_render_hidden() {
        let mut lb = ListBlock::new("l", "列表");
        lb.write(DataMode::Overwrite, "item1\nitem2");
        assert_eq!(lb.render(RenderLevel::Hidden), "");
    }

    #[test]
    fn list_block_render_title() {
        let mut lb = ListBlock::new("l", "列表");
        lb.write(DataMode::Overwrite, "item1");
        assert_eq!(lb.render(RenderLevel::Title), "");
    }

    #[test]
    fn list_block_render_normal() {
        let mut lb = ListBlock::new("l", "列表");
        lb.write(DataMode::Overwrite, "item1\nitem2");
        assert_eq!(lb.render(RenderLevel::Standard), "- item1\n- item2\n");
    }

    #[test]
    fn list_block_render_detailed() {
        let mut lb = ListBlock::new("l", "列表");
        lb.write(DataMode::Overwrite, "item1");
        assert_eq!(lb.render(RenderLevel::Detailed), "- item1\n");
    }

    #[test]
    fn list_block_render_summary() {
        let lb = ListBlock::new("l", "列表").items(vec!["a".into(), "b".into(), "c".into()]);
        let rendered = lb.render(RenderLevel::Summary);
        assert!(rendered.contains("- a"));
        assert!(rendered.contains("- b"));
        assert!(rendered.contains("共 3 项"));
    }

    #[test]
    fn list_block_render_summary_one_item() {
        let lb = ListBlock::new("l", "列表").items(vec!["a".into()]);
        let rendered = lb.render(RenderLevel::Summary);
        assert_eq!(rendered, "- a\n");
    }

    #[test]
    fn list_block_render_summary_two_items() {
        let lb = ListBlock::new("l", "列表").items(vec!["a".into(), "b".into()]);
        let rendered = lb.render(RenderLevel::Summary);
        assert!(!rendered.contains("共"));
    }

    #[test]
    fn list_block_write_overwrite_replaces() {
        let mut lb = ListBlock::new("l", "列表").items(vec!["old".into()]);
        lb.write(DataMode::Overwrite, "new1\nnew2");
        assert_eq!(lb.items, vec!["new1", "new2"]);
    }

    #[test]
    fn list_block_write_append() {
        let mut lb = ListBlock::new("l", "列表").items(vec!["a".into()]);
        lb.write(DataMode::Append, "b\nc");
        assert_eq!(lb.items, vec!["a", "b", "c"]);
    }

    #[test]
    fn list_block_write_clear() {
        let mut lb = ListBlock::new("l", "列表").items(vec!["a".into()]);
        lb.write(DataMode::Clear, "");
        assert!(lb.items.is_empty());
    }

    #[test]
    fn list_block_skips_empty_lines() {
        let mut lb = ListBlock::new("l", "列表");
        lb.write(DataMode::Overwrite, "a\n\nb\n  \nc");
        assert_eq!(lb.items, vec!["a", "b", "c"]);
    }

    #[test]
    fn list_block_handle_action_errors() {
        let mut lb = ListBlock::new("l", "列表");
        let result = lb.handle_action("expand", "");
        assert!(!result.is_success());
    }

    #[test]
    fn list_block_build_returns_leaf() {
        let node = ListBlock::new("l", "列表").build();
        match &node {
            ComponentNode::Leaf { .. } => {}
            _ => panic!("not leaf"),
        }
    }

    #[test]
    fn list_block_builder_methods() {
        let node = ListBlock::new("l", "列表")
            .priority(PriorityLevel::Low)
            .inert()
            .build();
        assert_eq!(node.priority(), PriorityLevel::Low);
        assert!(node.is_inert());
    }

    #[test]
    fn list_block_items_builder() {
        let items = vec!["a".to_string(), "b".to_string()];
        let lb = ListBlock::new("l", "列表").items(items);
        assert_eq!(lb.items, vec!["a", "b"]);
    }

    #[test]
    fn list_block_factory() {
        let node = list_block("l", "列表");
        assert_eq!(node.id(), "l");
    }

    #[test]
    fn list_block_through_tree() {
        let mut tree = crate::component::ComponentTree::new();
        let node = ListBlock::new("l", "列表")
            .items(vec!["任务1".into(), "任务2".into()])
            .build();
        tree.push(node);
        let rendered = tree.render(9999, None, 0);
        assert!(rendered.contains("列表"));
        assert!(rendered.contains("任务1"));
        assert!(rendered.contains("任务2"));
    }
}

//! CUI 原语组件 —— 通用 UI 原子：Label、Body、Button、DataSlot。
//!
//! 每个原语职责单一，通过组合产生业务语义（工具、技能、对话等）。

use crate::ComponentKind;
use crate::action::ActionResult;
use crate::component::{BaseComponent, ComponentNode};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;

// ── Label ──────────────────────────────────────────────────

/// 纯标题组件 —— 仅产生 `## [id] title` 行，无正文、无动作。
pub struct Label {
    id: String,
    title: String,
    priority: PriorityLevel,
}

impl Label {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            priority: PriorityLevel::Normal,
        }
    }

    pub fn priority(mut self, p: PriorityLevel) -> Self {
        self.priority = p;
        self
    }

    pub fn build(self) -> ComponentNode {
        ComponentNode::leaf(self)
    }
}

impl BaseComponent for Label {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn priority(&self) -> PriorityLevel {
        self.priority
    }
    fn kind(&self) -> ComponentKind {
        ComponentKind::Block
    }

    fn render(&self, _level: RenderLevel) -> String {
        String::new()
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, "Label 无动作")
    }
}

pub fn label(id: &str, title: &str) -> ComponentNode {
    Label::new(id, title).build()
}

// ── Body ───────────────────────────────────────────────────

/// 内联正文组件 —— 无标题栏，按渲染级别展示不同粒度。
///
/// * `Hidden` / `Title`: 空
/// * `Summary`: 首行（截断至 60 字）
/// * `Standard` / `Detailed`: 全文
pub struct Body {
    id: String,
    content: String,
}

impl Body {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
        }
    }

    pub fn build(self) -> ComponentNode {
        ComponentNode::leaf(self)
    }
}

impl BaseComponent for Body {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        ""
    }
    fn priority(&self) -> PriorityLevel {
        PriorityLevel::Normal
    }
    fn kind(&self) -> ComponentKind {
        ComponentKind::Inline
    }

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden | RenderLevel::Title => String::new(),
            RenderLevel::Summary => self
                .content
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| {
                    let trimmed = l.trim();
                    if trimmed.chars().count() > 60 {
                        format!("{}…", trimmed.chars().take(60).collect::<String>())
                    } else {
                        trimmed.to_string()
                    }
                })
                .unwrap_or_default(),
            RenderLevel::Standard | RenderLevel::Detailed => self.content.clone(),
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, "Body 无动作")
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => self.content = data.to_string(),
            DataMode::Append => self.content.push_str(data),
            DataMode::Clear => self.content.clear(),
        }
    }
}

pub fn body(id: &str, content: &str) -> ComponentNode {
    Body::new(id, content).build()
}

// ── Button ─────────────────────────────────────────────────

/// 行动组件 —— 类似 UI 按钮，标题 + 命令。
///
/// 渲染为 `` `[label]` `` 内联动作按钮，无标题栏。
/// 动作匹配由 `handle_action` 直接处理，不通过 `action_variants`。
pub struct Button {
    id: String,
    label: String,
    command: String,
    target_level: Option<RenderLevel>,
}

impl Button {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            command: command.into(),
            target_level: None,
        }
    }

    pub fn target(mut self, level: RenderLevel) -> Self {
        self.target_level = Some(level);
        self
    }

    pub fn build(self) -> ComponentNode {
        ComponentNode::leaf(self)
    }
}

impl BaseComponent for Button {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.label
    }
    fn priority(&self) -> PriorityLevel {
        PriorityLevel::Normal
    }
    fn kind(&self) -> ComponentKind {
        ComponentKind::Action
    }

    fn render(&self, _level: RenderLevel) -> String {
        format!("`[{}]`", self.label)
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        if action == self.command {
            let mut result = ActionResult::new(&self.id, action.to_string())
                .with_message(format!("{} 已完成", self.label))
                .with_snapshot(self.render(RenderLevel::Standard));
            if let Some(lvl) = self.target_level {
                result = result.with_new_level(lvl);
            }
            result
        } else {
            ActionResult::error(&self.id, action, format!("未知动作: {action}"))
        }
    }
}

pub fn button(id: &str, label: &str, command: &str) -> ComponentNode {
    Button::new(id, label, command).build()
}

// ── DataSlot ───────────────────────────────────────────────

/// 数据槽组件 —— 内联可写数据，替代 StateBlock。
///
/// 通过 `write()` 接收外部数据，按级别渲染。
pub struct DataSlot {
    id: String,
    title: String,
    data: String,
    priority: PriorityLevel,
    condition: VisibilityCondition,
    collapsible: bool,
}

impl DataSlot {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            data: String::new(),
            priority: PriorityLevel::Normal,
            condition: VisibilityCondition::Always,
            collapsible: false,
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

    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }

    pub fn build(self) -> ComponentNode {
        let collapsible = self.collapsible;
        let mut node = ComponentNode::leaf(self);
        node.set_collapsible(collapsible);
        node
    }
}

impl BaseComponent for DataSlot {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn priority(&self) -> PriorityLevel {
        self.priority
    }
    fn kind(&self) -> ComponentKind {
        ComponentKind::Inline
    }
    fn visibility_condition(&self) -> VisibilityCondition {
        self.condition.clone()
    }

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden | RenderLevel::Title => String::new(),
            RenderLevel::Summary => self.data.lines().next().unwrap_or("(空)").to_string(),
            RenderLevel::Standard | RenderLevel::Detailed => self.data.clone(),
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, "DataSlot 无动作")
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => self.data = data.to_string(),
            DataMode::Append => {
                self.data.push_str(data);
            }
            DataMode::Clear => self.data.clear(),
        }
    }
}

pub fn data_slot(id: &str, title: &str) -> ComponentNode {
    DataSlot::new(id, title).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Label ──────────────────────────────────────────────

    #[test]
    fn label_new() {
        let l = Label::new("l", "标题");
        assert_eq!(l.id, "l");
        assert_eq!(l.title, "标题");
    }

    #[test]
    fn label_render_empty() {
        let l = Label::new("l", "标题");
        assert_eq!(l.render(RenderLevel::Standard), "");
        assert_eq!(l.render(RenderLevel::Summary), "");
    }

    #[test]
    fn label_kind_is_block() {
        let l = Label::new("l", "标题");
        assert_eq!(l.kind(), ComponentKind::Block);
    }

    #[test]
    fn label_action_returns_error() {
        let mut l = Label::new("l", "标题");
        let r = l.handle_action("x", "");
        assert!(!r.is_success());
    }

    #[test]
    fn label_renders_via_render_node() {
        let node = label("my_label", "我的标题");
        let out = node.render_node(RenderLevel::Standard);
        assert!(out.contains("[我的标题]"), "output: {out}");
        assert!(out.contains("我的标题"), "output: {out}");
    }

    // ── Body ───────────────────────────────────────────────

    #[test]
    fn body_renders_full_at_standard() {
        let b = Body::new("b", "hello world\nline two");
        assert_eq!(b.render(RenderLevel::Standard), "hello world\nline two");
    }

    #[test]
    fn body_renders_first_line_at_summary() {
        let b = Body::new("b", "  hello world\nline two");
        assert_eq!(b.render(RenderLevel::Summary), "hello world");
    }

    #[test]
    fn body_truncates_long_first_line() {
        let long = "a".repeat(100);
        let b = Body::new("b", &long);
        let s = b.render(RenderLevel::Summary);
        assert!(s.ends_with('…'), "should end with ellipsis: {s}");
        assert!(
            s.chars().count() <= 61,
            "should be <=61 chars: {}",
            s.chars().count()
        );
    }

    #[test]
    fn body_hidden_returns_empty() {
        let b = Body::new("b", "content");
        assert_eq!(b.render(RenderLevel::Hidden), "");
    }

    #[test]
    fn body_kind_is_inline() {
        let b = Body::new("b", "content");
        assert_eq!(b.kind(), ComponentKind::Inline);
    }

    #[test]
    fn body_write() {
        let mut b = Body::new("b", "old");
        b.write(DataMode::Overwrite, "new");
        assert_eq!(b.content, "new");
    }

    #[test]
    fn body_renders_via_render_node() {
        let node = body("b1", "hello world");
        let out = node.render_node(RenderLevel::Standard);
        assert_eq!(out, "hello world");
        assert!(!out.contains("##"), "inline should have no heading: {out}");
    }

    // ── Button ─────────────────────────────────────────────

    #[test]
    fn button_render_format() {
        let b = Button::new("btn", "展开", "expand");
        let out = b.render(RenderLevel::Standard);
        assert_eq!(out, "`[展开]`");
    }

    #[test]
    fn button_handle_action_matching() {
        let mut b = Button::new("btn", "展开", "expand");
        let r = b.handle_action("expand", "");
        assert!(r.is_success());
    }

    #[test]
    fn button_handle_action_non_matching() {
        let mut b = Button::new("btn", "展开", "expand");
        let r = b.handle_action("wrong", "");
        assert!(!r.is_success());
    }

    #[test]
    fn button_with_target_level() {
        let mut b = Button::new("btn", "折叠", "collapse").target(RenderLevel::Summary);
        let r = b.handle_action("collapse", "");
        assert!(r.is_success());
        assert_eq!(r.new_level(), Some(RenderLevel::Summary));
    }

    #[test]
    fn button_kind_is_action() {
        let b = Button::new("btn", "展开", "expand");
        assert_eq!(b.kind(), ComponentKind::Action);
    }

    #[test]
    fn button_node_renders_without_heading() {
        let node = button("btn1", "点我", "click");
        let out = node.render_node(RenderLevel::Standard);
        assert_eq!(out, "`[点我]`");
    }

    // ── DataSlot ───────────────────────────────────────────

    #[test]
    fn slot_new() {
        let s = DataSlot::new("s", "状态");
        assert_eq!(s.id, "s");
        assert_eq!(s.title, "状态");
    }

    #[test]
    fn slot_renders_data() {
        let mut s = DataSlot::new("s", "状态");
        s.write(DataMode::Overwrite, "hello\nworld");
        assert_eq!(s.render(RenderLevel::Standard), "hello\nworld");
        assert_eq!(s.render(RenderLevel::Summary), "hello");
    }

    #[test]
    fn slot_empty_renders_placeholder() {
        let s = DataSlot::new("s", "状态");
        assert_eq!(s.render(RenderLevel::Summary), "(空)");
    }

    #[test]
    fn slot_write_modes() {
        let mut s = DataSlot::new("s", "状态");
        s.write(DataMode::Overwrite, "a");
        assert_eq!(s.data, "a");
        s.write(DataMode::Append, "b");
        assert_eq!(s.data, "ab");
        s.write(DataMode::Clear, "");
        assert_eq!(s.data, "");
    }

    #[test]
    fn slot_kind_is_inline() {
        let s = DataSlot::new("s", "状态");
        assert_eq!(s.kind(), ComponentKind::Inline);
    }

    #[test]
    fn slot_node_renders_without_heading() {
        let mut node = data_slot("s1", "DataSlot标题");
        node.write(DataMode::Overwrite, "slot content");
        let out = node.render_node(RenderLevel::Standard);
        assert_eq!(out, "slot content");
        assert!(!out.contains("##"), "inline should have no heading: {out}");
    }
}

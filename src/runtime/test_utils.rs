//! 测试工具 —— MockComponent 和测试辅助函数。
//!
//! # 示例
//!
//! ```ignore
//! use test_utils::MockComponent;
//!
//! let comp = MockComponent::new("greeting", "问候")
//!     .with_content("你好，世界！")
//!     .with_priority(PriorityLevel::High);
//! let output = comp.render(RenderLevel::Standard);
//! assert_eq!(output, "你好，世界！");
//! ```

use crate::action::{ActionResult, ActionVariant, VisibilityRule};
use crate::component::{CuiComponent, ComponentLifecycle, ComponentNode, Persistable};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;
use crate::manage::ManageEvent;

// ── MockComponent ──────────────────────────────────────────

/// 通用 Mock 组件 —— 通过 builder 模式配置行为。
///
/// 默认行为：
/// - `priority`: Normal
/// - `content`: `"{id} content"`（可通过 `with_content` 覆盖）
/// - `actions`: `["expand", "refresh"]`，其中 refresh 在 Level >= Standard 时隐藏
/// - 无 persist key，compress 返回 true
pub struct MockComponent {
    id: String,
    title: String,
    priority: PriorityLevel,
    content: String,
    action_result: Option<ActionResult>,
    pub written_data: String,
    pub event_fired: bool,
    pub cycle_id: u32,
    pub compressed: bool,
    persist: Option<String>,
    is_static: bool,
    is_inert: bool,
    visibility: VisibilityCondition,
    actions: Vec<ActionVariant>,
    use_empty_actions: bool,
}

impl MockComponent {
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            priority: PriorityLevel::Normal,
            content: format!("{id} content"),
            action_result: None,
            written_data: String::new(),
            event_fired: false,
            cycle_id: 0,
            compressed: false,
            persist: None,
            is_static: false,
            is_inert: false,
            visibility: VisibilityCondition::Always,
            use_empty_actions: false,
            actions: vec![
                ActionVariant::new("expand", "展开"),
                ActionVariant::new("refresh", "刷新")
                    .with_show(VisibilityRule::LevelLessThan(RenderLevel::Standard)),
            ],
        }
    }

    pub fn with_content(mut self, c: &str) -> Self {
        self.content = c.to_string();
        self
    }
    pub fn with_priority(mut self, p: PriorityLevel) -> Self {
        self.priority = p;
        self
    }
    pub fn with_action_result(mut self, r: ActionResult) -> Self {
        self.action_result = Some(r);
        self
    }
    pub fn with_persist(mut self, key: &str) -> Self {
        self.persist = Some(key.to_string());
        self
    }
    pub fn with_static(mut self) -> Self {
        self.is_static = true;
        self
    }
    pub fn with_inert(mut self) -> Self {
        self.is_inert = true;
        self
    }
    pub fn with_visibility(mut self, cond: VisibilityCondition) -> Self {
        self.visibility = cond;
        self
    }
    pub fn add_action(mut self, action: ActionVariant) -> Self {
        self.actions.push(action);
        self
    }
    pub fn with_no_actions(mut self) -> Self {
        self.use_empty_actions = true;
        self
    }
}

impl CuiComponent for MockComponent {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn priority(&self) -> PriorityLevel {
        self.priority
    }
    fn render(&self, _level: RenderLevel) -> String {
        self.content.clone()
    }
    fn is_static(&self) -> bool {
        self.is_static
    }
    fn is_inert(&self) -> bool {
        self.is_inert
    }
    fn visibility_condition(&self) -> VisibilityCondition {
        self.visibility.clone()
    }

    fn action_variants(&self) -> &'static [ActionVariant] {
        if self.use_empty_actions {
            return &[];
        }
        static DEFAULT: &[ActionVariant] = &[
            ActionVariant::new("expand", "展开"),
            ActionVariant::new("refresh", "刷新")
                .with_show(VisibilityRule::LevelLessThan(RenderLevel::Standard)),
        ];
        DEFAULT
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        if let Some(ref r) = self.action_result {
            return r.clone();
        }
        if action == "expand" || action == "refresh" {
            let mut result = ActionResult::new(&self.id, action.to_string());
            if action == "expand" {
                result = result.with_new_level(RenderLevel::Detailed);
            }
            result
        } else {
            ActionResult::error(&self.id, action, "unknown action")
        }
    }

    fn write(&mut self, _mode: DataMode, data: &str) {
        self.written_data = data.to_string();
    }
}

impl ComponentLifecycle for MockComponent {
    fn on_event(&mut self, _event: ManageEvent) {
        self.event_fired = true;
    }
    fn compress(&mut self) -> bool {
        self.compressed = true;
        true
    }
    fn start_new_cycle(&mut self, id: u32) {
        self.cycle_id = id;
    }
}

impl Persistable for MockComponent {
    fn persist_key(&self) -> Option<&str> {
        self.persist.as_deref()
    }
}

impl std::fmt::Debug for MockComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockComponent")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("priority", &self.priority)
            .finish()
    }
}

// ── 渲染辅助 ──────────────────────────────────────────────

/// 渲染组件正文（等价于 `component.render(level)`）。
pub fn render_to_string(comp: &dyn CuiComponent, level: RenderLevel) -> String {
    comp.render(level)
}

/// 断言组件在指定级别的渲染输出与预期一致。
pub fn assert_renders(comp: &dyn CuiComponent, level: RenderLevel, expected: &str) {
    assert_eq!(comp.render(level), expected);
}

// ── 树构建辅助 ────────────────────────────────────────────

/// 快速创建叶子节点（Normal priority，默认行为）。
pub fn build_leaf(id: &str, title: &str) -> ComponentNode {
    ComponentNode::leaf(MockComponent::new(id, title))
}

/// 快速创建 Composite 节点。
pub fn build_composite(id: &str, title: &str, children: Vec<ComponentNode>) -> ComponentNode {
    ComponentNode::composite(MockComponent::new(id, title), children)
}

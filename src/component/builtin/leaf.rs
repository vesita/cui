//! .cui 文件叶节点 —— 编译自 .cui 文件，直接实现 CuiComponent。
//!
//! 渲染规则：
//! - Hidden → ""
//! - Title → `title`
//! - Summary → `summary` 字段 → body 首行 → ""
//! - Standard / Detailed → `body` + (data 非空时追加)
//!
//! `write()` 将数据写入 `data` 字段，与 body 分开存储。

use crate::PriorityLevel;
use crate::action::ActionResult;
use crate::component::{CuiComponent, ComponentNode, Persistable};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::keyword::{ComponentKind, IoDef};
use crate::level::RenderLevel;

/// 由 .cui 文件编译得到的叶节点，跳过 adapter 直接实现 CuiComponent。
pub struct CuiFileLeaf {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) priority: PriorityLevel,
    pub(crate) summary: Option<String>,
    pub(crate) inert: bool,
    pub(crate) is_static: bool,
    pub(crate) body: String,
    pub(crate) data: String,
    pub(crate) condition: VisibilityCondition,
    pub(crate) input_values: Vec<(String, String)>,
    pub(crate) kind: ComponentKind,
    pub(crate) inputs: Vec<IoDef>,
    pub(crate) outputs: Vec<IoDef>,
    pub(crate) persist_key: Option<String>,
    pub(crate) subtype: Option<String>,
    pub(crate) title_override: Option<String>,
    pub(crate) body_override: Option<String>,
}

impl CuiFileLeaf {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            priority: PriorityLevel::Normal,
            summary: None,
            inert: false,
            is_static: false,
            body: body.into(),
            data: String::new(),
            condition: VisibilityCondition::Always,
            input_values: Vec::new(),
            kind: ComponentKind::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            persist_key: None,
            subtype: None,
            title_override: None,
            body_override: None,
        }
    }

    pub fn priority(mut self, p: PriorityLevel) -> Self {
        self.priority = p;
        self
    }
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }
    pub fn inert(mut self) -> Self {
        self.inert = true;
        self
    }
    pub fn is_static(mut self) -> Self {
        self.is_static = true;
        self
    }
    pub fn with_condition(mut self, c: VisibilityCondition) -> Self {
        self.condition = c;
        self
    }
    pub fn kind(mut self, k: ComponentKind) -> Self {
        self.kind = k;
        self
    }
    pub fn persist(mut self, key: impl Into<String>) -> Self {
        self.persist_key = Some(key.into());
        self
    }
    pub fn with_inputs(mut self, inputs: Vec<IoDef>) -> Self {
        self.inputs = inputs;
        self
    }
    pub fn with_outputs(mut self, outputs: Vec<IoDef>) -> Self {
        self.outputs = outputs;
        self
    }
    pub fn with_input(mut self, name: &str, value: &str) -> Self {
        self.input_values.push((name.to_string(), value.to_string()));
        self
    }
    pub fn with_input_values(mut self, values: &[(&str, &str)]) -> Self {
        for (k, v) in values {
            self.input_values.push((k.to_string(), v.to_string()));
        }
        self
    }
    pub fn subtype(mut self, s: impl Into<String>) -> Self {
        self.subtype = Some(s.into());
        self
    }

    pub fn build(self) -> ComponentNode {
        ComponentNode::leaf(self)
    }
}

/// 在已注册的叶节点上应用用户覆盖。
pub(crate) fn leaf_apply_override(
    node: &mut ComponentNode,
    title: Option<&str>,
    body: Option<&str>,
    inputs: &[(String, String)],
    pinned: bool,
) {
    if let ComponentNode::Leaf(info) = node {
        if let Some(comp) = info.component.as_mut().as_any_mut().and_then(|a| a.downcast_mut::<CuiFileLeaf>()) {
            if let Some(t) = title {
                comp.title_override = Some(t.to_string());
            }
            if let Some(b) = body {
                comp.body_override = Some(b.to_string());
            }
            for (name, val) in inputs {
                comp.input_values.retain(|(k, _)| k != name);
                comp.input_values.push((name.to_string(), val.to_string()));
            }
        }
    }
    if pinned {
        node.set_pinned(true);
    }
}

impl CuiComponent for CuiFileLeaf {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        self.title_override.as_deref().unwrap_or(&self.title)
    }
    fn priority(&self) -> PriorityLevel {
        self.priority
    }
    fn is_inert(&self) -> bool {
        self.inert
    }
    fn is_static(&self) -> bool {
        self.is_static
    }
    fn visibility_condition(&self) -> VisibilityCondition {
        self.condition.clone()
    }
    fn kind(&self) -> ComponentKind {
        self.kind
    }
    fn subtype(&self) -> Option<&str> {
        self.subtype.as_deref()
    }
    fn input_schema(&self) -> &[IoDef] {
        &self.inputs
    }
    fn output_schema(&self) -> &[IoDef] {
        &self.outputs
    }

    fn render(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Title => String::new(),
            RenderLevel::Summary => self.summary.clone().unwrap_or_else(|| {
                let refs: Vec<(&str, &str)> = self
                    .input_values
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                let first_line = self.body.lines().next().unwrap_or("").to_string();
                crate::compile::template::TemplateEngine::fill_slots(&first_line, &refs)
            }),
            RenderLevel::Standard | RenderLevel::Detailed => {
                let base_body = self.body_override.as_deref().unwrap_or(&self.body);
                let refs: Vec<(&str, &str)> = self
                    .input_values
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                let body =
                    crate::compile::template::TemplateEngine::fill_slots(base_body, &refs);
                let mut out = body;
                if !self.data.is_empty() {
                    out.push('\n');
                    out.push_str(&self.data);
                }
                out
            }
        }
    }

    fn handle_action(&mut self, action: &str, _params: &str) -> ActionResult {
        ActionResult::error(&self.id, action, format!("未知动作: {action}"))
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => self.data = data.to_string(),
            DataMode::Append => {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(data);
            }
            DataMode::Clear => self.data.clear(),
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl Persistable for CuiFileLeaf {
    fn persist_key(&self) -> Option<&str> {
        self.persist_key.as_deref()
    }
}

/// 快速创建 CuiFileLeaf 的工厂函数。
pub fn cui_file_leaf(id: &str, title: &str, body: &str) -> ComponentNode {
    CuiFileLeaf::new(id, title, body).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentNode;
    use crate::data::DataMode;
    use crate::keyword::PriorityLevel;
    use crate::level::RenderLevel;

    #[test]
    fn cui_file_leaf_new() {
        let leaf = CuiFileLeaf::new("id1", "标题", "内容");
        assert_eq!(leaf.id(), "id1");
        assert_eq!(leaf.title(), "标题");
        assert_eq!(CuiComponent::priority(&leaf), PriorityLevel::Normal);
        assert!(!leaf.is_inert());
        assert!(!CuiComponent::is_static(&leaf));
    }

    #[test]
    fn cui_file_leaf_builder_methods() {
        let leaf = CuiFileLeaf::new("id", "标题", "内容")
            .priority(PriorityLevel::High)
            .summary("摘要")
            .inert()
            .is_static()
            .with_condition(VisibilityCondition::when("review"));
        assert_eq!(CuiComponent::priority(&leaf), PriorityLevel::High);
        assert!(leaf.is_inert());
        assert!(CuiComponent::is_static(&leaf));
    }

    #[test]
    fn cui_file_leaf_build_returns_leaf() {
        let node = CuiFileLeaf::new("id", "标题", "内容").build();
        match &node {
            ComponentNode::Leaf { .. } => {}
            _ => panic!("build() should return Leaf"),
        }
        assert_eq!(node.id(), "id");
    }

    #[test]
    fn cui_file_leaf_render_respects_level() {
        let leaf = CuiFileLeaf::new("id", "标题", "body 内容\n第二行");
        assert_eq!(leaf.render(RenderLevel::Hidden), "");
        assert_eq!(leaf.render(RenderLevel::Title), "");
        // Summary: 显式 summary 未设置，回退到 body 首行
        assert_eq!(leaf.render(RenderLevel::Summary), "body 内容");
        let std = leaf.render(RenderLevel::Standard);
        assert!(std.contains("body 内容"));
        assert!(std.contains("第二行"));
    }

    #[test]
    fn cui_file_leaf_render_with_explicit_summary() {
        let leaf = CuiFileLeaf::new("id", "标题", "body").summary("自定义摘要");
        assert_eq!(leaf.render(RenderLevel::Summary), "自定义摘要");
    }

    #[test]
    fn cui_file_leaf_render_with_data_appended() {
        let mut leaf = CuiFileLeaf::new("id", "标题", "body");
        leaf.write(DataMode::Append, "注入数据");
        let rendered = leaf.render(RenderLevel::Standard);
        assert!(rendered.contains("body"));
        assert!(rendered.contains("注入数据"));
    }

    #[test]
    fn cui_file_leaf_write_overwrite() {
        let mut leaf = CuiFileLeaf::new("id", "标题", "body");
        leaf.write(DataMode::Overwrite, "新数据");
        let rendered = leaf.render(RenderLevel::Standard);
        assert!(rendered.contains("body"));
        assert!(rendered.contains("新数据"));
    }

    #[test]
    fn cui_file_leaf_write_append() {
        let mut leaf = CuiFileLeaf::new("id", "标题", "body");
        leaf.write(DataMode::Overwrite, "第一");
        leaf.write(DataMode::Append, "第二");
        let rendered = leaf.render(RenderLevel::Standard);
        assert!(rendered.contains("第一\n第二"));
    }

    #[test]
    fn cui_file_leaf_write_clear() {
        let mut leaf = CuiFileLeaf::new("id", "标题", "body");
        leaf.write(DataMode::Overwrite, "数据");
        leaf.write(DataMode::Clear, "");
        let rendered = leaf.render(RenderLevel::Standard);
        assert!(!rendered.contains("数据"));
    }

    #[test]
    fn cui_file_leaf_handle_action_errors() {
        let mut leaf = CuiFileLeaf::new("id", "标题", "body");
        let result = leaf.handle_action("anything", "");
        assert!(!result.is_success());
        assert!(result.message().unwrap().contains("未知动作"));
    }

    #[test]
    fn cui_file_leaf_with_input() {
        let leaf = CuiFileLeaf::new("id", "标题", "hello {{input:name}}").with_input("name", "世界");
        let rendered = leaf.render(RenderLevel::Standard);
        assert!(rendered.contains("hello 世界"));
    }

    #[test]
    fn cui_file_leaf_factory() {
        let node = cui_file_leaf("c", "标题", "body");
        assert_eq!(node.id(), "c");
        assert_eq!(node.title(), "标题");
    }
}

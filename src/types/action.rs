//! CUI 动作系统 —— 声明式动作定义，连接 AI 交互与后端执行。
//!
//! 三层设计：
//!
//! - [`ActionVariant`] — 静态动作描述（`&'static str`，零分配）。
//!   `action_variants()` 是组件声明动作的首选方式。
//! - [`ActionDef`] — 拥有所有权的动作数据（运行时构建，用于序列化输出）。
//! - [`ActionHandler`] / [`ActionHandlerRef`] — 动作绑定的后端处理器。
//!
//! AI 通过 `component_action` 工具传字符串 → 框架匹配 id → 有 handler 则执行，无则改渲染级别。

use crate::keyword::IoDef;
use crate::level::RenderLevel;
use crate::runtime::handler::{ActionHandler, ActionHandlerRef};
use std::sync::Arc;

use serde::de::DeserializeOwned;

/// 静态动作变体 —— 编译期定义，零动态分配。
///
/// 组件通过 `BaseComponent::action_variants()` 返回 `&'static [ActionVariant]`，
/// 框架自动构建 `ActionDef` 用于序列化，并驱动默认的 `handle_action`。
///
/// ## 后端绑定
///
/// 设置 `handler` 字段可将动作绑定到后端处理器。
/// - `None`（默认）：纯展示动作，执行时改变 `target_level`
/// - `Some(handler)`：行为动作，执行时调用 `handler.execute()`
#[derive(Clone, Debug)]
pub struct ActionVariant {
    /// 动作唯一标识。
    pub id: &'static str,
    /// AI 可见的按钮标签。
    pub label: &'static str,
    /// 动作执行后预期的渲染级别（展示动作用）。
    pub target_level: Option<RenderLevel>,
    /// 可选：可见性条件。
    pub show_when: Option<VisibilityRule>,
    /// 可选：预设参数。
    pub params: Option<&'static [(&'static str, &'static str)]>,
    /// 可选：后端处理器引用。
    pub handler: Option<ActionHandlerRef>,
    /// 可选：参数 schema 定义。
    pub params_schema: Option<&'static [IoDef]>,
}

impl ActionVariant {
    /// 快速构造一个纯展示动作。
    pub const fn new(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            target_level: None,
            show_when: None,
            params: None,
            handler: None,
            params_schema: None,
        }
    }

    /// 设置目标级别。
    pub const fn with_target(mut self, target: RenderLevel) -> Self {
        self.target_level = Some(target);
        self
    }

    /// 设置可见性条件。
    pub const fn with_show(mut self, rule: VisibilityRule) -> Self {
        self.show_when = Some(rule);
        self
    }

    /// 绑定后端处理器。
    pub fn with_handler(mut self, handler: ActionHandlerRef) -> Self {
        self.handler = Some(handler);
        self
    }

    /// 绑定后端处理器（便捷方法：Arc<dyn ActionHandler>）。
    pub fn with_handler_inline(mut self, handler: Arc<dyn ActionHandler>) -> Self {
        self.handler = Some(ActionHandlerRef::Inline(handler));
        self
    }

    /// 绑定后端处理器（便捷方法：命名引用）。
    pub fn with_handler_named(mut self, name: impl Into<String>) -> Self {
        self.handler = Some(ActionHandlerRef::Named(name.into()));
        self
    }
}

impl From<&ActionVariant> for ActionDef {
    fn from(v: &ActionVariant) -> Self {
        let mut def = ActionDef::new(v.id, v.label);
        if let Some(tl) = v.target_level {
            def = def.with_target_level(tl);
        }
        if let Some(sw) = v.show_when {
            def = def.with_show_when(sw);
        }
        if let Some(p) = v.params {
            def = def.with_params(
                p.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            );
        }
        if let Some(h) = v.handler.clone() {
            def = def.with_handler(h);
        }
        if let Some(ps) = v.params_schema {
            def = def.with_params_schema(ps.to_vec());
        }
        def
    }
}

/// 组件暴露给 AI 的动作声明（拥有所有权版本，用于序列化）。
///
/// 渲染为 YAML frontmatter 的 `actions` 字段：
/// ```yaml
/// actions:
///   - {id: expand, label: 展开完整内容, show: <detailed}
///   - {id: execute, label: 执行, handler: tool.bash}
/// ```
///
/// 大多数场景下组件只需定义 [`ActionVariant`]，框架自动转换为此类型。
#[derive(Clone, Debug)]
pub struct ActionDef {
    /// 动作唯一标识，传给 `handle_action`。
    id: String,
    /// AI 可见的按钮标签。
    label: String,
    /// 动作执行后预期的渲染级别。
    target_level: Option<RenderLevel>,
    /// 可选：可见性条件。
    show_when: Option<VisibilityRule>,
    /// 可选：预设参数。
    params: Option<std::collections::HashMap<String, String>>,
    /// 可选：后端处理器引用。
    handler: Option<ActionHandlerRef>,
    /// 可选：参数 schema 定义。
    params_schema: Option<Vec<IoDef>>,
}

impl ActionDef {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            target_level: None,
            show_when: None,
            params: None,
            handler: None,
            params_schema: None,
        }
    }

    pub fn with_target_level(mut self, level: RenderLevel) -> Self {
        self.target_level = Some(level);
        self
    }
    pub fn with_show_when(mut self, rule: VisibilityRule) -> Self {
        self.show_when = Some(rule);
        self
    }
    pub fn with_params(mut self, params: std::collections::HashMap<String, String>) -> Self {
        self.params = Some(params);
        self
    }
    pub fn with_handler(mut self, handler: ActionHandlerRef) -> Self {
        self.handler = Some(handler);
        self
    }
    pub fn with_params_schema(mut self, schema: Vec<IoDef>) -> Self {
        self.params_schema = Some(schema);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn target_level(&self) -> Option<RenderLevel> {
        self.target_level
    }
    pub fn show_when(&self) -> Option<&VisibilityRule> {
        self.show_when.as_ref()
    }
    pub fn params(&self) -> Option<&std::collections::HashMap<String, String>> {
        self.params.as_ref()
    }
    pub fn handler(&self) -> Option<&ActionHandlerRef> {
        self.handler.as_ref()
    }
    pub fn params_schema(&self) -> Option<&[IoDef]> {
        self.params_schema.as_deref()
    }

    pub fn set_handler(&mut self, handler: ActionHandlerRef) {
        self.handler = Some(handler);
    }
}

/// 动作按钮的可见性条件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibilityRule {
    /// 当前渲染级别低于指定级别时显示。
    LevelLessThan(RenderLevel),
    /// 当前渲染级别高于指定级别时显示。
    LevelGreaterThan(RenderLevel),
}

/// AI 通过 `component_action` 工具发来的动作请求。
#[derive(Clone, Debug)]
pub struct ActionRequest {
    /// 目标组件的 `id`。
    pub component_id: String,
    /// 动作标识，匹配 `ActionDef.id`。
    pub action: String,
    /// 可选的 JSON 参数。
    pub params: Option<String>,
}

impl ActionRequest {
    /// 解析整个 params 为指定类型 T。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// #[derive(Deserialize)]
    /// struct MyParams { key: String, count: u32 }
    /// let p: MyParams = req.params_as()?;
    /// ```
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<T, String> {
        let json_str = self.params.as_deref().unwrap_or("{}");
        serde_json::from_str(json_str).map_err(|e| format!("参数解析失败: {e}"))
    }

    /// 从 params JSON 中提取指定 key 的值，反序列化为 T。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let count: u32 = req.param("count")?;
    /// ```
    pub fn param<T: DeserializeOwned>(&self, key: &str) -> Result<T, String> {
        let json_str = self.params.as_deref().unwrap_or("{}");
        let map: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("参数 JSON 解析失败: {e}"))?;
        let value = map.get(key).ok_or_else(|| format!("缺少参数 '{key}'"))?;
        serde_json::from_value(value.clone()).map_err(|e| format!("参数 '{key}' 类型不匹配: {e}"))
    }

    /// 从 params JSON 中提取可选 key 的值，不存在返回 None。
    pub fn param_opt<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, String> {
        let json_str = self.params.as_deref().unwrap_or("{}");
        let map: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("参数 JSON 解析失败: {e}"))?;
        match map.get(key) {
            Some(val) if !val.is_null() => serde_json::from_value(val.clone())
                .map(Some)
                .map_err(|e| format!("参数 '{key}' 类型不匹配: {e}")),
            _ => Ok(None),
        }
    }
}

/// 动作执行结果。
#[derive(Clone, Debug)]
pub struct ActionResult {
    component_id: String,
    action: String,
    success: bool,
    message: Option<String>,
    new_level: Option<RenderLevel>,
    rendered_snapshot: Option<String>,
}

impl ActionResult {
    /// 构造成功结果。
    pub fn new(component_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            component_id: component_id.into(),
            action: action.into(),
            success: true,
            message: None,
            new_level: None,
            rendered_snapshot: None,
        }
    }

    /// 构造失败结果。
    pub fn error(
        component_id: impl Into<String>,
        action: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            component_id: component_id.into(),
            action: action.into(),
            success: false,
            message: Some(message.into()),
            new_level: None,
            rendered_snapshot: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_new_level(mut self, level: RenderLevel) -> Self {
        self.new_level = Some(level);
        self
    }

    pub fn with_snapshot(mut self, snapshot: impl Into<String>) -> Self {
        self.rendered_snapshot = Some(snapshot.into());
        self
    }

    pub fn with_success(mut self, v: bool) -> Self {
        self.success = v;
        self
    }

    pub fn component_id(&self) -> &str {
        &self.component_id
    }

    pub fn action(&self) -> &str {
        &self.action
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn new_level(&self) -> Option<RenderLevel> {
        self.new_level
    }

    pub fn snapshot(&self) -> Option<&str> {
        self.rendered_snapshot.as_deref()
    }

    pub fn set_snapshot(&mut self, snapshot: impl Into<String>) {
        self.rendered_snapshot = Some(snapshot.into());
    }

    pub fn set_component_id(&mut self, id: impl Into<String>) {
        self.component_id = id.into();
    }
}

/// 对话组件操作接口。
///
/// 由对话节实现（如 `DialogueSectionOps`），通过 `Context` 的共享 Arc 访问。
pub trait DialogueOps {
    /// 滚动到绝对位置（0=开头，-1=末尾）。
    fn scroll_to(&mut self, position: i32) -> Option<String>;
    /// 按轮次相对步数滚动。
    fn scroll_by_cycles(&mut self, step: i32) -> Option<String>;
    /// 对齐到轮次边界。
    fn align_to_turn_boundary(&mut self) -> bool;
    /// 展开冷区域消息范围。
    fn expand_cold_zone(&mut self, start: i32, end: i32) -> Option<String>;
    /// 关闭冷区域。
    fn close_cold_zone(&mut self) -> bool;
    /// 冷区域续期。
    fn request_cold_zone(&mut self) -> bool;
    /// 倒计时 tick。
    fn tick_cold_zone_countdown(&mut self) -> bool;
    /// 返回热窗消息的 JSON 序列化列表（用于 LLM 注入）。
    fn hot_messages_json(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_new() {
        let v = ActionVariant::new("expand", "展开");
        assert_eq!(v.id, "expand");
        assert_eq!(v.label, "展开");
        assert_eq!(v.target_level, None);
        assert_eq!(v.show_when, None);
    }

    #[test]
    fn variant_with_target() {
        let v = ActionVariant::new("expand", "展开").with_target(RenderLevel::Detailed);
        assert_eq!(v.target_level, Some(RenderLevel::Detailed));
    }

    #[test]
    fn variant_with_show() {
        let v = ActionVariant::new("expand", "展开")
            .with_show(VisibilityRule::LevelLessThan(RenderLevel::Standard));
        assert_eq!(
            v.show_when,
            Some(VisibilityRule::LevelLessThan(RenderLevel::Standard))
        );
    }

    #[test]
    fn variant_to_def() {
        let v = ActionVariant::new("fold", "折叠").with_target(RenderLevel::Summary);
        let def: ActionDef = (&v).into();
        assert_eq!(def.id(), "fold");
        assert_eq!(def.label(), "折叠");
        assert_eq!(def.target_level(), Some(RenderLevel::Summary));
        assert_eq!(def.show_when(), None);
    }

    #[test]
    fn action_result_error() {
        let r = ActionResult::error("comp1", "act1", "something went wrong");
        assert!(!r.is_success());
        assert_eq!(r.component_id(), "comp1");
        assert_eq!(r.action(), "act1");
        assert_eq!(r.message(), Some("something went wrong"));
        assert_eq!(r.new_level(), None);
    }

    #[test]
    fn action_result_success_defaults() {
        let r = ActionResult::new("c", "a");
        assert!(r.is_success());
        assert_eq!(r.component_id(), "c");
    }

    #[test]
    fn visibility_rule_equality() {
        let a = VisibilityRule::LevelLessThan(RenderLevel::Detailed);
        let b = VisibilityRule::LevelLessThan(RenderLevel::Detailed);
        let c = VisibilityRule::LevelGreaterThan(RenderLevel::Standard);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn action_request_basic() {
        let req = ActionRequest {
            component_id: "git".into(),
            action: "expand".into(),
            params: None,
        };
        assert_eq!(req.component_id, "git");
        assert_eq!(req.action, "expand");
        assert!(req.params.is_none());
    }

    #[test]
    fn action_variant_debug_clone() {
        let v = ActionVariant::new("a", "b");
        let v2 = v.clone();
        let _ = format!("{:?}", v2);
    }

    #[test]
    fn action_def_debug_clone() {
        let def = ActionDef::new("x", "y")
            .with_target_level(RenderLevel::Title)
            .with_show_when(VisibilityRule::LevelGreaterThan(RenderLevel::Hidden));
        let def2 = def.clone();
        let _ = format!("{:?}", def2);
    }

    #[test]
    fn action_request_params_as_typed() {
        use serde::Deserialize;
        #[derive(Deserialize, PartialEq, Debug)]
        struct Cmd {
            name: String,
            args: Vec<String>,
        }
        let req = ActionRequest {
            component_id: "a".into(),
            action: "run".into(),
            params: Some(r#"{"name":"ls","args":["-la"]}"#.into()),
        };
        let cmd: Cmd = req.params_as().unwrap();
        assert_eq!(cmd.name, "ls");
        assert_eq!(cmd.args, vec!["-la"]);
    }

    #[test]
    fn action_request_params_as_empty() {
        let req = ActionRequest {
            component_id: "a".into(),
            action: "run".into(),
            params: None,
        };
        // 无参时应反序列化为空结构体
        #[derive(serde::Deserialize)]
        struct Empty {}
        let _: Empty = req.params_as().unwrap();
    }

    #[test]
    fn action_request_param_extract() {
        let req = ActionRequest {
            component_id: "x".into(),
            action: "y".into(),
            params: Some(r#"{"count":42,"name":"test"}"#.into()),
        };
        let count: u32 = req.param("count").unwrap();
        assert_eq!(count, 42);
        let name: String = req.param("name").unwrap();
        assert_eq!(name, "test");
    }

    #[test]
    fn action_request_param_missing() {
        let req = ActionRequest {
            component_id: "x".into(),
            action: "y".into(),
            params: Some(r#"{"a":1}"#.into()),
        };
        let result: Result<String, String> = req.param("missing");
        assert!(result.is_err());
    }

    #[test]
    fn action_request_param_opt_some() {
        let req = ActionRequest {
            component_id: "x".into(),
            action: "y".into(),
            params: Some(r#"{"key":"val"}"#.into()),
        };
        let v: Option<String> = req.param_opt("key").unwrap();
        assert_eq!(v, Some("val".into()));
    }

    #[test]
    fn action_request_param_opt_none() {
        let req = ActionRequest {
            component_id: "x".into(),
            action: "y".into(),
            params: Some(r#"{"other":1}"#.into()),
        };
        let v: Option<String> = req.param_opt("missing").unwrap();
        assert_eq!(v, None);
    }

    #[test]
    fn action_request_param_opt_null() {
        let req = ActionRequest {
            component_id: "x".into(),
            action: "y".into(),
            params: Some(r#"{"key":null}"#.into()),
        };
        let v: Option<String> = req.param_opt("key").unwrap();
        assert_eq!(v, None);
    }
}

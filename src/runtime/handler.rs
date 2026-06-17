//! 动作处理器 —— CUI 动作与后端系统的桥接层。
//!
//! # 设计
//!
//! `ActionHandler` trait 是外部系统（工具执行、数据查询等）接入 CUI 的唯一接口。
//! 组件声明动作时绑定 handler，AI 触发动作时框架调用 handler。
//!
//! `HandlerRegistry` 是独立的处理器注册表，可从 Context 中提取使用，
//! 便于后端在 Context 创建前预先配置处理器。
//!
//! # 绑定方式
//!
//! - `ActionHandlerRef::Inline(Arc<dyn ActionHandler>)` — Rust 代码直接注入
//! - `ActionHandlerRef::Named(String)` — `.cui` 文件中按名称引用，运行时从注册表解析

use crate::action::ActionRequest;
use crate::component::ComponentNode;
use crate::data::DataMode;
use crate::level::RenderLevel;
use std::sync::Arc;

/// 动作处理器 —— 后端系统实现此 trait 以响应 CUI 动作。
pub trait ActionHandler: Send + Sync {
    /// 执行动作。`params` 为已合并 preset + request 的 JSON 字符串。
    fn execute(
        &self,
        params: &str,
        ctx: &mut dyn ActionContext,
    ) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>>;

    /// 执行动作（类型化参数访问）。默认调用 `execute()`，覆写后可用
    /// `request.param::<T>("key")` 直接解析参数，无需手动操作 JSON。
    fn execute_request(
        &self,
        request: &ActionRequest,
        ctx: &mut dyn ActionContext,
    ) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
        self.execute(request.params.as_deref().unwrap_or("{}"), ctx)
    }

    /// 参数 JSON Schema（可选，用于 AI 理解参数格式）。
    fn params_schema(&self) -> Option<String> {
        None
    }

    /// 处理器唯一标识（可选，用于注册表查找和日志）。
    fn id(&self) -> &str {
        ""
    }

    /// 处理器显示名称（可选，用于 AI 展示）。
    fn display_name(&self) -> &str {
        ""
    }
}

/// 动作处理器引用。
#[derive(Clone)]
pub enum ActionHandlerRef {
    /// Rust 代码直接注入的处理器。
    Inline(Arc<dyn ActionHandler>),
    /// `.cui` 文件中通过名称引用的处理器，运行时从注册表解析。
    Named(String),
    /// 类型模板中的槽位引用 —— 编译时从实例 `handler:` 字段解析为 `Named`。
    Unresolved(String),
}

impl std::fmt::Debug for ActionHandlerRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline(_) => f.debug_tuple("Inline").field(&"<handler>").finish(),
            Self::Named(name) => f.debug_tuple("Named").field(name).finish(),
            Self::Unresolved(name) => f.debug_tuple("Unresolved").field(name).finish(),
        }
    }
}

/// 处理器注册表 —— 独立于 Context 的命名处理器容器。
///
/// 后端可在 Context 创建前预先注册处理器，后续通过 `apply_to()` 注入 Context。
///
/// # 示例
///
/// ```ignore
/// let mut registry = HandlerRegistry::new();
/// registry.register("tool.bash", Arc::new(BashHandler));
/// registry.apply_to(&mut ctx);
/// ```
#[derive(Clone, Default)]
pub struct HandlerRegistry {
    handlers: std::collections::HashMap<String, Arc<dyn ActionHandler>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    /// 注册命名处理器。
    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn ActionHandler>) {
        self.handlers.insert(name.into(), handler);
    }

    /// 按名称查找处理器。
    pub fn resolve(&self, name: &str) -> Option<Arc<dyn ActionHandler>> {
        self.handlers.get(name).cloned()
    }

    /// 解析 `ActionHandlerRef`：`Inline` 直接返回，`Named` 查注册表。
    /// `Unresolved` 应在编译阶段已解析为 `Named`，运行时遇到则返回 None。
    pub fn resolve_ref(&self, r: &ActionHandlerRef) -> Option<Arc<dyn ActionHandler>> {
        match r {
            ActionHandlerRef::Inline(h) => Some(h.clone()),
            ActionHandlerRef::Named(name) => self.resolve(name),
            ActionHandlerRef::Unresolved(name) => {
                tracing::warn!(
                    "ActionHandlerRef::Unresolved({}) 未在编译时解析，运行时忽略",
                    name
                );
                None
            }
        }
    }

    /// 遍历所有已注册的处理器名称和引用。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn ActionHandler>)> {
        self.handlers.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// 检查指定名称的处理器是否已注册。
    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// 将注册表中的所有处理器注入到 Context 中。
    pub fn apply_to(&self, ctx: &mut crate::runtime::context::Context) {
        for (name, handler) in &self.handlers {
            ctx.register_handler(name.clone(), handler.clone());
        }
    }

    /// 注册表是否为空。
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 注册表中的处理器数量。
    pub fn len(&self) -> usize {
        self.handlers.len()
    }
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.handlers.keys().map(|s| s.as_str()).collect();
        f.debug_struct("HandlerRegistry")
            .field("handlers", &names)
            .finish()
    }
}

/// 受限的框架访问接口，传给 ActionHandler::execute()。
///
/// 只暴露安全的读写和查询操作，不暴露树管理、渲染等能力。
/// 由 `Context` 实现。
pub trait ActionContext {
    /// 向指定组件写入数据。
    fn write(&mut self, component_id: &str, mode: DataMode, data: &str);
    /// 读取指定组件的渲染内容。
    fn read(&self, component_id: &str) -> Option<String>;
    /// 发送事件（跨组件通信）。
    fn emit(&mut self, source: &str, kind: &str, data: &str);
    /// 订阅事件（跨组件通信）。
    fn on(&mut self, _pattern: &str, _handler: Box<dyn Fn(&crate::runtime::event::ComponentEvent) + Send>) {}

    /// 读取全局状态值。
    fn state(&self, key: &str) -> Option<String>;
    /// 设置全局状态值。
    fn set_state(&mut self, key: &str, value: &str);
    /// 获取后端注入的扩展资源（调用方通过 `downcast_ref::<T>()` 下行）。
    /// 默认返回 `None`，Context 实现可通过 `set_extension` 注入。
    fn resource(&self) -> Option<&dyn std::any::Any> {
        None
    }

    // ── 以下方法供后端 handler 查询组件树状态 ──

    /// 检查指定 ID 的组件是否已注册。
    fn component_exists(&self, _id: &str) -> bool {
        false
    }
    /// 读取指定组件的当前渲染级别。
    fn component_level(&self, _id: &str) -> Option<RenderLevel> {
        None
    }
    /// 列出所有已注册的组件 ID 及其当前渲染级别。
    fn list_components(&self) -> Vec<(String, RenderLevel)> {
        Vec::new()
    }
    /// 注册一个新的动作处理器。
    fn register_handler(&mut self, _name: &str, _handler: Arc<dyn ActionHandler>) {}

    /// 动态注册新组件节点（handler 可在运行时创建组件）。
    fn register(&mut self, _node: ComponentNode) {}
}

/// 动作执行结果。
#[derive(Clone, Debug)]
pub struct ActionOutput {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<String>,
    pub new_level: Option<RenderLevel>,
    pub events: Vec<(String, String)>,
}

impl ActionOutput {
    /// 快速构造成功结果。
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: None,
            new_level: None,
            events: Vec::new(),
        }
    }

    /// 快速构造成功结果并携带数据。
    pub fn success_with_data(message: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(message.into()),
            data: Some(data.into()),
            new_level: None,
            events: Vec::new(),
        }
    }

    /// 快速构造失败结果。
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(message.into()),
            data: None,
            new_level: None,
            events: Vec::new(),
        }
    }

    /// 附加级别变更。
    pub fn with_level(mut self, level: RenderLevel) -> Self {
        self.new_level = Some(level);
        self
    }

    /// 附加事件。
    pub fn with_event(mut self, event: impl Into<String>, data: impl Into<String>) -> Self {
        self.events.push((event.into(), data.into()));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    impl ActionHandler for TestHandler {
        fn execute(
            &self,
            params: &str,
            _ctx: &mut dyn ActionContext,
        ) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
            Ok(ActionOutput::success_with_data("ok", params))
        }
        fn id(&self) -> &str {
            "test_handler"
        }
    }

    #[test]
    fn handler_registry_register_and_resolve() {
        let mut reg = HandlerRegistry::new();
        reg.register("test", Arc::new(TestHandler));
        assert!(reg.contains("test"));
        assert!(!reg.contains("unknown"));
        let resolved = reg.resolve("test");
        assert!(resolved.is_some());
    }

    #[test]
    fn handler_registry_resolve_ref_inline() {
        let reg = HandlerRegistry::new();
        let handler = Arc::new(TestHandler) as Arc<dyn ActionHandler>;
        let r = ActionHandlerRef::Inline(handler.clone());
        let resolved = reg.resolve_ref(&r);
        assert!(resolved.is_some());
    }

    #[test]
    fn handler_registry_resolve_ref_named() {
        let mut reg = HandlerRegistry::new();
        reg.register("foo", Arc::new(TestHandler));
        let r = ActionHandlerRef::Named("foo".into());
        let resolved = reg.resolve_ref(&r);
        assert!(resolved.is_some());
    }

    #[test]
    fn handler_registry_resolve_ref_named_missing() {
        let reg = HandlerRegistry::new();
        let r = ActionHandlerRef::Named("missing".into());
        let resolved = reg.resolve_ref(&r);
        assert!(resolved.is_none());
    }

    #[test]
    fn handler_registry_iter() {
        let mut reg = HandlerRegistry::new();
        reg.register("a", Arc::new(TestHandler));
        reg.register("b", Arc::new(TestHandler));
        let names: Vec<&str> = reg.iter().map(|(n, _)| n).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn handler_registry_is_empty() {
        let reg = HandlerRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_handler_has_id() {
        let h = TestHandler;
        assert_eq!(h.id(), "test_handler");
    }

    #[test]
    fn action_output_success() {
        let o = ActionOutput::success("done");
        assert!(o.success);
        assert_eq!(o.message, Some("done".to_string()));
        assert!(o.data.is_none());
    }

    #[test]
    fn action_output_error() {
        let o = ActionOutput::error("fail");
        assert!(!o.success);
        assert_eq!(o.message, Some("fail".to_string()));
    }

    #[test]
    fn action_output_with_level() {
        let o = ActionOutput::success("ok").with_level(RenderLevel::Detailed);
        assert_eq!(o.new_level, Some(RenderLevel::Detailed));
    }

    #[test]
    fn action_output_with_event() {
        let o = ActionOutput::success("ok").with_event("test.event", "{}");
        assert_eq!(o.events.len(), 1);
        assert_eq!(o.events[0].0, "test.event");
    }

    #[test]
    fn handler_registry_debug() {
        let mut reg = HandlerRegistry::new();
        reg.register("x", Arc::new(TestHandler));
        let s = format!("{:?}", reg);
        assert!(s.contains("x"));
    }

    #[test]
    fn action_handler_ref_debug() {
        let r = ActionHandlerRef::Named("foo".into());
        let s = format!("{:?}", r);
        assert!(s.contains("foo"));
    }
}

//! 事件系统 —— 组件间松耦合通信。
//!
//! # 设计
//!
//! 组件通过事件总线发布和订阅事件，无需直接引用对方。
//! 订阅使用前缀通配模式：`component.*` 匹配所有以 `component.` 开头的事件。
//!
//! # 事件类型
//!
//! | 事件 kind          | source    | data (JSON)                            | 触发时机           |
//! |-------------------|-----------|----------------------------------------|-------------------|
//! | `registered`      | 组件 ID   | `{"id":"<id>"}`                        | `register()`      |
//! | `removed`         | 组件 ID   | `{"id":"<id>"}`                        | `remove()`        |
//! | `data_changed`    | 组件 ID   | 写入的原始数据（字符串）                  | `write()`         |
//! | `action_executed` | 组件 ID   | `{"action":"<name>","success":bool}`   | `component_action()` |
//!
//! # 使用
//!
//! ```ignore
//! // 订阅
//! ctx.on("component.updated", Box::new(|e| { ... }));
//!
//! // 发布
//! ctx.emit("component.updated", json_data);
//! ```

/// 组件事件。
#[derive(Clone, Debug)]
pub struct ComponentEvent {
    /// 来源组件 ID。
    pub source: String,
    /// 事件类型：`data_changed` | `action_executed` | `level_changed` | `registered` | `removed`
    pub kind: String,
    /// 事件数据（JSON 字符串）。
    pub data: String,
}

impl ComponentEvent {
    pub fn new(
        source: impl Into<String>,
        kind: impl Into<String>,
        data: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            kind: kind.into(),
            data: data.into(),
        }
    }

    /// 是否匹配订阅模式。
    ///
    /// 支持 `*` 通配符：
    /// - `dialogue.*` — 匹配 dialogue 组件的所有事件
    /// - `*.data_changed` — 匹配所有组件的 data_changed 事件
    /// - `*` — 匹配所有事件
    /// - `dialogue.data_changed` — 精确匹配
    fn matches_pattern(&self, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        let full_name = format!("{}.{}", self.source, self.kind);
        if let Some(source_pat) = pattern.strip_suffix(".*") {
            // "dialogue.*" — 匹配所有 kind
            self.source == source_pat
        } else if let Some(kind_pat) = pattern.strip_prefix("*.") {
            // "*.data_changed" — 匹配所有 source
            self.kind == kind_pat
        } else {
            full_name == pattern
        }
    }
}

/// 事件总线 —— 组件间松耦合通信。
pub(crate) trait EventBus {
    /// 发布事件。
    fn emit(&mut self, event: ComponentEvent);

    /// 订阅事件。
    ///
    /// `pattern` 格式：`"component.data_changed"`（精确）或 `"component.*"`（前缀通配）。
    fn on(&mut self, pattern: &str, handler: Box<dyn Fn(&ComponentEvent) + Send>);

    /// 发布事件（便捷方法，自动构造 ComponentEvent）。
    fn emit_raw(&mut self, source: &str, kind: &str, data: &str) {
        self.emit(ComponentEvent::new(source, kind, data));
    }
}

/// 事件总线的简单实现。
///
/// 订阅列表按注册顺序匹配，所有匹配的处理器均被调用。
pub(crate) struct SimpleEventBus {
    #[allow(clippy::type_complexity)]
    subscribers: Vec<(String, Box<dyn Fn(&ComponentEvent) + Send>)>,
}

impl SimpleEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }
}

impl EventBus for SimpleEventBus {
    fn emit(&mut self, event: ComponentEvent) {
        for (pattern, handler) in &self.subscribers {
            if event.matches_pattern(pattern) {
                handler(&event);
            }
        }
    }

    fn on(&mut self, pattern: &str, handler: Box<dyn Fn(&ComponentEvent) + Send>) {
        self.subscribers.push((pattern.to_string(), handler));
    }
}

impl Default for SimpleEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pattern_match() {
        let event = ComponentEvent::new("dialogue", "data_changed", "{}");
        assert!(event.matches_pattern("dialogue.data_changed"));
        assert!(!event.matches_pattern("dialogue.action_executed"));
        assert!(!event.matches_pattern("other.data_changed"));
    }

    #[test]
    fn wildcard_pattern_match() {
        let event = ComponentEvent::new("dialogue", "data_changed", "{}");
        assert!(event.matches_pattern("dialogue.*"));
        assert!(event.matches_pattern("*.data_changed"));
        assert!(!event.matches_pattern("other.*"));
    }

    #[test]
    fn emit_and_subscribe() {
        use std::sync::Mutex;
        let mut bus = SimpleEventBus::new();
        let received: std::sync::Arc<Mutex<Vec<String>>> = Default::default();
        let r = received.clone();

        bus.on(
            "dialogue.*",
            Box::new(move |e| {
                r.lock().unwrap().push(e.kind.clone());
            }),
        );

        bus.emit_raw("dialogue", "data_changed", "{}");
        bus.emit_raw("dialogue", "action_executed", "{}");
        bus.emit_raw("other", "data_changed", "{}");

        let events = received.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "data_changed");
        assert_eq!(events[1], "action_executed");
    }

    #[test]
    fn default_bus() {
        let mut bus = SimpleEventBus::default();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = called.clone();
        bus.on(
            "*",
            Box::new(move |_| {
                c.store(true, std::sync::atomic::Ordering::SeqCst);
            }),
        );
        bus.emit_raw("any", "any", "{}");
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }
}

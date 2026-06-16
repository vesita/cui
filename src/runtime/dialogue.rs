//! 对话管理 —— 消息缓冲和 DialogueOps 委托.
//!
//! 消息缓冲无上限，渲染窗口由 `VirtualView` 在 `DialogueSectionBase` 中管理。
//! 早期版本曾设 200 条硬上限，但截断会切断 tool_call↔tool_result 配对，
//! 导致 LLM API 400 错误。`VirtualView` 本身即提供无上限完整列表 + 可见窗口，
//! 此处无需重复截断。
//!
//! 读写分离：
//! - `read_hot_messages()` → 热窗消息（LLM 注入，默认路径）
//! - `read_all_messages()` → 全量历史（持久化/恢复，显式调用）

use std::sync::{Arc, Mutex};

use crate::action::DialogueOps;
use crate::component::ComponentTree;
use crate::data::DataMode;

/// 对话管理器 —— 管理消息缓冲和外部对话操作委托.
pub struct DialogueManager {
    messages: Vec<String>,
    shared: Option<Arc<Mutex<Box<dyn DialogueOps + Send>>>>,
}

impl DialogueManager {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            shared: None,
        }
    }

    /// 设置外部 DialogueOps 访问.
    pub fn set_shared(&mut self, ops: Arc<Mutex<Box<dyn DialogueOps + Send>>>) {
        self.shared = Some(ops);
    }

    /// 清空消息缓冲和外部 ops.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.shared = None;
    }

    /// 读取全量对话消息（持久化/恢复用，非 LLM 路径）。
    pub fn read_all_messages(&self) -> &[String] {
        &self.messages
    }

    /// 读取热窗消息（LLM 注入默认路径）。
    /// 若 DialogueOps 已注册则从 WindowState 热窗取，否则降级为全量。
    pub fn read_hot_messages(&self) -> Vec<String> {
        if let Some(ops) = &self.shared
            && let Ok(guard) = ops.lock()
        {
            let hot = guard.hot_messages_json();
            if !hot.is_empty() {
                return hot;
            }
        }
        self.messages.clone()
    }

    /// 推送 JSON 序列化的消息到对话.
    ///
    /// 同时写入 `dialogue` 组件的状态，用于渲染时展示.
    /// 消息缓冲无上限，`VirtualView` 在组件侧管理渲染窗口。
    pub fn push_message(&mut self, json: &str, tree: &mut ComponentTree) {
        self.messages.push(json.to_string());
        if !tree.write("dialogue", DataMode::Append, json) {
            tracing::warn!("警告: push_message 写入 dialogue 组件失败（可能未注册）");
        }
    }

    /// 通过共享 Arc 访问 DialogueOps.
    pub fn with_dialogue<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn DialogueOps) -> R,
    {
        let mut guard = self.shared.as_ref()?.lock().ok()?;
        Some(f(guard.as_mut()))
    }
}

impl Default for DialogueManager {
    fn default() -> Self {
        Self::new()
    }
}

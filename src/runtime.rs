//! 运行时服务 —— 事件总线、动作处理器、类型注册表、容量规划、对话管理。
//!
//! ## 子模块
//! - [`registry`] — 组件类型注册表（TypeRegistry）
//! - [`event`] — 事件总线
//! - [`handler`] — 动作处理器注册/分发
//! - [`capacity`] — 容量规划（token 预算）
//! - [`dialogue`] — 对话管理
//! - [`ordering`] — 组件排序策略

pub mod capacity;
pub mod dialogue;
pub mod event;
pub mod handler;
pub mod ordering;
pub mod registry;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

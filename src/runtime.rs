//! 运行时服务 —— 上下文管理、事件总线、动作处理器、类型注册表、容量规划、对话管理、渲染管线。
//!
//! ## 子模块
//! - [`context`] — 运行时上下文管理器（Context）
//! - [`capacity`] — 容量规划（token 预算）
//! - [`cycle`] — 渲染状态机（RenderCycle）
//! - [`schedule`] — 渲染调度（RenderPlan、辅助函数）
//! - [`output`] — 组件输出格式化
//! - [`event`] — 事件总线
//! - [`handler`] — 动作处理器注册/分发
//! - [`dialogue`] — 对话管理
//! - [`ordering`] — 组件排序策略
//! - [`registry`] — 组件类型注册表（TypeRegistry）

pub mod capacity;
pub mod context;
pub mod cycle;
pub(crate) mod dialogue;
pub(crate) mod event;
pub mod handler;
pub mod ordering;
pub mod output;
pub mod registry;
pub(crate) mod schedule;

pub use cycle::{Abort, CommitMsg, DoRenderPlan, Idle, Prepare, Preparing, RenderCycle, Rendering};
pub use schedule::{RenderPlan, RenderStats};

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

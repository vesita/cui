//! 渲染管线 —— 渲染状态机、容量规划与统计。
//!
//! ## 子模块
//! - `cycle` — RenderCycle 状态机（由 Context 驱动）
//! - `schedule` — RenderPlan、RenderStats、容量规划辅助函数

pub mod cycle;
pub mod schedule;

pub use cycle::{Abort, CommitMsg, DoRenderPlan, Idle, Prepare, Preparing, RenderCycle, Rendering};
pub use schedule::{RenderPlan, RenderStats};

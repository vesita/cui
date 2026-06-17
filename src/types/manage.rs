//! 生命周期管理事件 —— 组件在编排层中接收的系统事件。
//!
//! 对应 ESCDIR 的 `ManageTags` 语义，但直接挂在组件上。

/// 管理操作 —— 组件在收到事件时应执行的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ManageOp {
    /// 刷新数据（drain 后重建）。
    Refresh,
    /// 持久化当前状态。
    Persist,
    /// 压缩旧数据。
    Compress,
}

/// 生命周期事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManageEvent {
    /// 系统初始化。
    Init,
    /// Step 开始（新轮次）。
    StepStart,
    /// Step 结束。
    StepEnd,
    /// 进入新阶段。
    PhaseEnter(&'static str),
    /// 外部事件（如 MCP 工具变更）。
    External(&'static str),
}

impl ManageEvent {
    /// 是否为刷新类事件。
    pub fn is_refresh(&self) -> bool {
        matches!(self, ManageEvent::StepStart | ManageEvent::Init)
    }
}

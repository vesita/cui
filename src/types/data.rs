//! 数据更新系统 —— 组件数据写入模式与容量管理。
//!
//! [`DataMode`] 枚举定义写入语义，通过 `CuiComponent::write()` 转发到组件实现。
//! [`TruncatePolicy`] 控制容量超限时的截断策略。

/// 数据写入模式 —— 对应外部系统的编辑语义。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataMode {
    /// 整体覆写。
    Overwrite,
    /// 追加新数据。
    Append,
    /// 清空所有数据。
    Clear,
}

/// 容量超限截断策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TruncatePolicy {
    /// 保留尾部（丢弃旧数据）。
    KeepTail,
    /// 保留头部（丢弃新数据）。
    KeepHead,
}

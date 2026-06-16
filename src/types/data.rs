//! 数据更新系统 —— 组件数据写入模式。
//!
//! [`DataMode`] 枚举定义写入语义，通过 `BaseComponent::write()` 转发到组件实现。

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

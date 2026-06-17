//! 适配器模块 —— 将 .cui 模板文件转换为 CUI 组件节点。
//!
//! skill 和 tool 本质上是同一模式：加载 .cui 模板，用 CUI 原语构建节点。
//! 区别仅在于来源和分组方式。

pub mod skill;
pub mod tool;

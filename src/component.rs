//! 组件模型 —— CuiComponent 体系及组件树数据结构。
//!
//! ## 核心架构
//! - **base** — CuiComponent、ComponentLifecycle、Persistable trait
//! - **node** — ComponentSignal、NodeSchema、NodeInfo、ComponentNode
//! - **tree** — ComponentTree 数据模型 + StateEntry
//! - **iter** — AllNodes、AllNodesMut 全树迭代器
//! - **snapshot** — TreeSnapshot 序列化类型
//! - **builder** — CuiBuilder 声明式组装
//! - **builtin** — TextBlock、ConditionalBlock 等内置组件

pub mod base;
pub mod builtin;
pub(crate) mod iter;
pub mod node;
pub mod snapshot;
pub mod tree;

#[cfg(test)]
mod tests;

pub use base::{CuiComponent, ComponentLifecycle, Persistable};
pub use iter::{AllNodes, AllNodesMut};
pub use node::{ComponentNode, ComponentSignal, NodeInfo, NodeSchema};
pub use snapshot::{NodeSnapshot, TreeSnapshot, TreeStats};
pub use tree::{ComponentTree, StateEntry};

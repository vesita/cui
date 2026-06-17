//! 内置组件库 —— 框架级组件，覆盖 80% 场景无需手写 trait。
//!
//! 类比 Flutter 的 Text/Container/ListView 等内置 Widget。
//! 这些组件实现 [`BaseComponent`](crate::component::BaseComponent)，
//! 通过 `ComponentNode::leaf()` 即可使用。
//!
//! # 子模块
//!
//! - [`blocks`] — TextBlock、ConditionalBlock、ListBlock
//! - [`leaf`] — CuiFileLeaf（.cui 文件叶节点）
//! - [`group`] — GroupBuilder + GroupComponent（分组容器）
//! - [`primitives`] — Label、Body、Button、DataSlot 原子组件

mod blocks;
mod group;
mod leaf;
mod primitives;
mod toast;

pub use blocks::{
    ConditionalBlock, ListBlock, TextBlock, conditional_block, hidden_block, list_block, text_block,
};
pub use group::{GroupBuilder, group};
pub use leaf::{CuiFileLeaf, cui_file_leaf};
pub(crate) use leaf::leaf_apply_override;
pub use primitives::{Body, Button, DataSlot, Label, body, button, data_slot, label};
pub use toast::{Toast, toast};

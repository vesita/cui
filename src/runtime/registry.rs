//! 组件类型系统 —— 语义类型定义与类型解析。
//!
//! # 模块
//!
//! - [`registry`] — ComponentTypeDef、TypeRegistry、ResolvedComponent、resolve() 合并算法
//! - [`builtin`] — 内置类型定义（tool + section）
#![allow(clippy::module_inception)]

pub mod builtin;
pub mod registry;

pub use builtin::builtin_registry;
pub use registry::{ComponentTypeDef, ResolvedComponent, SlotDecl, TypeRegistry};

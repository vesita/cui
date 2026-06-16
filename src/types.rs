//! 核心基础类型 — CUI 框架共享的枚举、trait 和工具函数。
//!
//! 所有上层模块（text、tree、compiler 等）都依赖此模块中的类型。

pub mod action;
pub mod condition;
pub mod data;
pub mod keyword;
pub mod level;
pub mod manage;
pub mod output;
pub mod tokenizer;

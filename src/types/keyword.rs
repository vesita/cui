//! CUI 关键字系统 —— 编译器风格的关键字定义、验证与错误报告。
//!
//! # 设计原则
//!
//! - 所有关键字必须在此注册，未知关键字产生编译器风格错误
//! - 位置感知的错误消息：`file.cui:行:列`
//! - 向后兼容：新关键字都有默认值，现有 `.cui` 文件无需修改

pub mod def;
pub mod error;
pub mod types;

pub use def::{KeyPosition, KeywordCategory, KeywordDef, KeywordRegistry};
pub use error::{KeywordError, KeywordErrorKind, KeywordErrorSeverity};
pub use types::{ComponentKind, IoDef, IoType, PriorityLevel};

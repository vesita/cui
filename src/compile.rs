//! 编译管道 —— .cui 源码 → 模板填充 → 编译为组件树。
//!
//! 三个模块形成天然管道：
//! 1. [`file`] — 解析 .cui 文件格式（frontmatter + body）
//! 2. [`template`] — 模板引擎，填充槽位和指令
//! 3. [`compiler`] — 编译器，将多文档 .cui 源文件构建为 ComponentTree

pub mod compiler;
pub mod file;
pub mod template;

//! `.cui` 文件格式 —— 用 YAML frontmatter + Markdown body 声明 CUI 组件。
//!
//! # 格式
//!
//! ```markdown
//! ---
//! id: tools/read_file       # 可选，默认从文件路径推断
//! title: 📖 读文件           # 必需
//! priority: critical          # 可选，默认 normal
//! summary: 读取文件内容的工具  # 可选，无则取 body 首行
//! inert: true                # 可选，默认 false
//! static: true               # 可选，默认 false
//! actions:                   # 可选
//!   - {id: expand, label: 展开, target: detailed}
//! ---
//! 读取文件内容的工具。
//!
//! 用法：read_file(path: str)
//! ```
//!
//! # WYSIWYG
//!
//! `.cui` 文件的 body 直接等于 Normal 级渲染输出。
//! 开发者写入的内容 = AI 看到的内容，无额外转换层。
//!
//! # 目录加载
//!
//! ```ignore
//! cui/
//! ├── tools/
//! │   ├── read_file.cui
//! │   └── git_diff.cui
//! └── skills/
//!     └── rust.cui
//! ```
//!
//! 文件路径 → 组件 ID：`cui/tools/read_file.cui` → `tools/read_file`
//!
//! 加载时自动注入内置的 `_cui_introduction` 介绍组件，无需手动创建。

mod component;
mod directory;
mod frontmatter;
mod user;

pub use component::CuiFileComponent;
pub(crate) use component::PROMPT_ESCDIR;
pub use directory::CuiDirectory;
pub(crate) use user::load_user_overrides;

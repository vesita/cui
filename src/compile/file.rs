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

pub use component::CuiFileComponent;
pub(crate) use component::PROMPT_ESCDIR;
pub use directory::CuiDirectory;
pub use frontmatter::parse_frontmatter_body;

/// 解析多个 CUI 块（以 `\n---\n` 分隔的多个 frontmatter + body 文档）。
pub fn parse_multi_cui(content: &str) -> Vec<CuiFileComponent> {
    let content = content.trim();
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))
        .unwrap_or(content);

    let parts: Vec<&str> = rest.split("\n---\n").collect();
    let mut blocks = Vec::new();

    for pair in parts.chunks(2) {
        let fm = pair.first().map(|s| s.trim()).unwrap_or("");
        let body = pair.get(1).map(|s| s.trim()).unwrap_or("");
        if fm.is_empty() {
            continue;
        }
        let full = format!("---\n{}\n---\n{}", fm, body);
        match CuiFileComponent::from_str(&full, "__llm__") {
            Ok(comp) => blocks.push(comp),
            Err(e) => {
                tracing::warn!(
                    "parse_multi_cui: 跳过无法解析的块: {}",
                    if e.len() > 120 { &e[..120] } else { &e }
                );
            }
        }
    }
    blocks
}

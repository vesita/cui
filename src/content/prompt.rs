//! 提示词加载工具。
//!
//! ## 宏
//!
//! - [`prompt_path!`] —— 计算 workspace `prompt/` 根下的绝对路径（编译期）
//! - [`include_prompt!`] —— 返回相对于 workspace `prompt/` 的路径（编译期）
//!
//! 运行时加载统一使用 [`CuiFileComponent::from_file`]。
//!
//! ## 路径约定
//!
//! `prompt_path!` 及基于它的宏假设调用 crate 在 `crates/<name>/` 深度（workspace 标准布局）。

/// 计算 workspace `prompt/` 根下的绝对路径。
///
/// ```ignore
/// const GIT_USAGE_PATH: &str = prompt_path!("escdir/capability/git_usage.md");
/// ```
#[macro_export]
macro_rules! prompt_path {
    ($path:expr) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../prompt/", $path)
    };
}

/// 返回相对于 workspace `prompt/` 的路径字符串（编译期常量）。
///
/// 运行时通过 [`CuiFileComponent::from_file`] 加载。
///
/// ```ignore
/// const SKILLS_PATH: &str = include_prompt!("escdir/capability/skills.md");
/// ```
#[macro_export]
macro_rules! include_prompt {
    ($path:literal) => {
        $path
    };
}

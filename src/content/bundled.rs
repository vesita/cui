//! 编译时嵌入的 `.cui` 文件。
//!
//! `build.rs` 扫描 `cui/` 目录生成 `bundled_cui.rs`，
//! 此模块封装后供 `CuiDirectory::scan_root()` 使用。

/// 加载所有编译时嵌入的 `.cui` 文件（来自 CUI crate 的 `cui/` 目录）。
pub fn bundled_files() -> Vec<(&'static str, &'static str)> {
    include!(concat!(env!("OUT_DIR"), "/bundled_cui.rs"))
}

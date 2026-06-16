//! `.cui` 目录加载器 —— 递归扫描 `.cui` 文件并构建组件。

use std::fs;
use std::path::{Path, PathBuf};

use super::component::CuiFileComponent;

/// `cui/` 目录加载器的数据源。
enum CuiSource {
    Filesystem(PathBuf),
    Bundle,
}

/// `cui/` 目录加载器。
pub struct CuiDirectory {
    source: CuiSource,
}

impl CuiDirectory {
    /// 从文件系统 `cui/` 目录加载。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            source: CuiSource::Filesystem(root.into()),
        }
    }

    /// 从 CUI crate 自身编译时嵌入的 `cui/` 目录加载。
    pub fn scan_root() -> Self {
        Self {
            source: CuiSource::Bundle,
        }
    }

    /// 加载所有 `.cui` 文件对应的组件（单文档模式）。
    pub fn load(&self) -> Result<Vec<CuiFileComponent>, String> {
        match &self.source {
            CuiSource::Bundle => {
                let mut components = Vec::new();
                for (fname, content) in crate::content::bundled::bundled_files() {
                    let default_id = fname.trim_end_matches(".cui");
                    let comp = CuiFileComponent::from_str(content, default_id)?;
                    components.push(comp);
                }
                Ok(components)
            }
            CuiSource::Filesystem(root) => {
                let mut components = Vec::new();
                let builtin = CuiFileComponent::from_str(
                    include_str!("../../../cui/_cui_help.cui"),
                    "_cui_help",
                )
                .map_err(|e| format!("内置自描述组件解析失败: {e}"))?;
                components.push(builtin);
                if root.exists() {
                    visit_dir_inner(root, root, &mut components, &ScanStrategy::Single)?;
                }
                Ok(components)
            }
        }
    }

    /// 加载所有 `.cui` 文件，自动展开多文档格式。
    pub fn load_multi(&self) -> Result<Vec<CuiFileComponent>, String> {
        let mut expanded = Vec::new();
        match &self.source {
            CuiSource::Bundle => {
                for (fname, content) in crate::content::bundled::bundled_files() {
                    let default_id = fname.trim_end_matches(".cui");
                    if crate::compile::compiler::is_multi_document(content) {
                        match crate::compile::compiler::expand_multi_document(content, default_id) {
                            Ok(mut docs) => expanded.append(&mut docs),
                            Err(e) => {
                                tracing::warn!(
                                    "警告 [CUI 加载器]: 内嵌文件 '{}' 多文档展开失败: {:?}",
                                    default_id,
                                    e
                                );
                            }
                        }
                    } else {
                        let comp = CuiFileComponent::from_str(content, default_id)?;
                        expanded.push(comp);
                    }
                }
            }
            CuiSource::Filesystem(root) => {
                let builtin_yaml = include_str!("../../../cui/_cui_help.cui");
                let builtin = CuiFileComponent::from_str(builtin_yaml, "_cui_help")?;
                expanded.push(builtin);

                if root.exists() {
                    visit_dir_inner(root, root, &mut expanded, &ScanStrategy::Multi)?;
                }
            }
        }
        Ok(expanded)
    }
}

/// 目录扫描策略。
enum ScanStrategy {
    Single,
    Multi,
}

/// 递归扫描目录中的 `.cui` 文件，按策略解析并追加到 `out`。
fn visit_dir_inner(
    root: &Path,
    dir: &Path,
    out: &mut Vec<CuiFileComponent>,
    strategy: &ScanStrategy,
) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("读取目录失败 {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("目录项读取失败: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            visit_dir_inner(root, &path, out, strategy)?;
        } else if path.extension().is_some_and(|ext| ext == "cui") {
            let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if fname.starts_with('_') || fname.starts_with('.') {
                continue;
            }

            let relative = path.strip_prefix(root).unwrap_or(&path);
            let default_id = relative
                .with_extension("")
                .to_str()
                .unwrap_or("unknown")
                .replace('\\', "/");

            let content = fs::read_to_string(&path)
                .map_err(|e| format!("读取文件失败 {}: {}", path.display(), e))?;

            match strategy {
                ScanStrategy::Single => {
                    out.push(CuiFileComponent::from_str(&content, &default_id)?);
                }
                ScanStrategy::Multi => {
                    let components =
                        crate::compile::compiler::expand_multi_document(&content, &default_id)?;
                    out.extend(components);
                }
            }
        }
    }
    Ok(())
}

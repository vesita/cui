//! 用户覆盖 —— 从用户目录加载自定义组件配置。
//!
//! 用户文件与开发者文件结构相同，但额外支持 `pinned: true`。
//! 加载时按 id 匹配合并：用户值覆盖开发者值。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::keyword::IoDef;

use super::component::CuiFileComponent;

/// 用户覆盖条目 —— 代表一个用户自定义的组件覆盖。
#[derive(Debug, Clone)]
pub(crate) struct UserOverride {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub summary: Option<String>,
    pub priority: Option<crate::keyword::PriorityLevel>,
    pub inputs: Vec<(String, String)>,
    pub pinned: bool,
}

/// 加载指定目录中所有 `.cui` 文件作为用户覆盖。
pub(crate) fn load_user_overrides(dir: &Path) -> Vec<UserOverride> {
    if !dir.exists() {
        return vec![];
    }
    let mut overrides = Vec::new();
    visit_user_dir(dir, dir, &mut overrides);
    overrides
}

fn visit_user_dir(root: &Path, current: &Path, out: &mut Vec<UserOverride>) {
    let entries = match fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_user_dir(root, &path, out);
        } else if path.extension().map_or(false, |e| e == "cui") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('_') || name.starts_with('.') {
                    continue;
                }
            }
            let default_id = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.trim_end_matches(".cui").replace('/', "."))
                .unwrap_or_default();
            match load_user_file(&path, &default_id) {
                Ok(o) => out.push(o),
                Err(e) => tracing::warn!("跳过用户覆盖文件 {:?}: {}", path.display(), e),
            }
        }
    }
}

fn load_user_file(path: &Path, default_id: &str) -> Result<UserOverride, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{}", e))?;
    let comp = CuiFileComponent::from_str(&content, default_id)?;
    let fm_content = fs::read_to_string(path).map_err(|e| format!("{}", e))?;
    let pinned = extract_pinned(&fm_content);
    let inputs: Vec<(String, String)> = comp
        .input_values()
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .collect();
    Ok(UserOverride {
        id: comp.id().to_string(),
        title: Some(comp.title().to_string()),
        body: Some(comp.body().to_string()),
        summary: comp.summary().map(|s| s.to_string()),
        priority: Some(comp.priority()),
        inputs,
        pinned,
    })
}

/// 从 frontmatter YAML 中提取 `pinned` 值。
fn extract_pinned(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "pinned: true" || trimmed == "pinned: True" {
            return true;
        }
        if trimmed == "---" {
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("pinned:") {
            continue;
        }
    }
    false
}

/// 将用户覆盖合并到基础组件列表。
///
/// `base` 是开发者提供的组件列表，`overrides` 是用户自定义覆盖。
/// 按 `id` 匹配：找到同 id 的 base 组件，用用户值覆盖。
/// 用户独有的组件（新 id）追加到列表末尾。
pub(crate) fn merge_user_overrides(
    base: &mut Vec<CuiFileComponent>,
    overrides: &[UserOverride],
) {
    let mut override_map: HashMap<&str, &UserOverride> =
        overrides.iter().map(|o| (o.id.as_str(), o)).collect();

    for comp in base.iter_mut() {
        if let Some(o) = override_map.remove(comp.id()) {
            apply_override(comp, o);
        }
    }

    // 用户独有的新组件（基表中没有对应 id）也保留
    for (_, o) in override_map {
        let builder = UserOverrideComponent::from_override(o);
        if let Ok(c) = builder.to_cui_file_component() {
            base.push(c);
        }
    }
}

fn apply_override(comp: &mut CuiFileComponent, o: &UserOverride) {
    comp.set_pinned(o.pinned);
    if let Some(ref t) = o.title {
        comp.set_title(t);
    }
    if let Some(ref b) = o.body {
        comp.set_body(b);
    }
    if let Some(ref s) = o.summary {
        comp.set_summary(s);
    }
    if let Some(p) = o.priority {
        comp.set_priority(p);
    }
    for (name, val) in &o.inputs {
        comp.set_input(name, val);
    }
}

// ── UserOverrideComponent builder ───────────────────────────────

struct UserOverrideComponent {
    id: String,
    title: String,
    body: String,
    inputs: Vec<IoDef>,
}

impl UserOverrideComponent {
    fn from_override(o: &UserOverride) -> Self {
        let inputs: Vec<IoDef> = o
            .inputs
            .iter()
            .map(|(name, val)| IoDef {
                name: name.clone(),
                io_type: crate::keyword::IoType::String,
                required: false,
                description: None,
                default_value: Some(val.clone()),
            })
            .collect();
        Self {
            id: o.id.clone(),
            title: o.title.clone().unwrap_or_default(),
            body: o.body.clone().unwrap_or_default(),
            inputs,
        }
    }

    fn to_cui_file_component(self) -> Result<CuiFileComponent, String> {
        let mut yaml = format!("id: {}\ntitle: {}", self.id, self.title);
        for input in &self.inputs {
            if let Some(ref dv) = input.default_value {
                use std::fmt::Write;
                let _ = write!(yaml, "\n  - {{name: {}, default_value: {}}}", input.name, dv);
            }
        }
        let full = format!("---\n{}\n---\n{}", yaml, self.body);
        CuiFileComponent::from_str(&full, &self.id)
    }
}

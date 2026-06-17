//! YAML frontmatter 解析 —— 反序列化类型、可见性解析、input 合并。

use crate::condition::VisibilityCondition;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;

use serde::Deserialize;

// ── 优先级反序列化 ────────────────────────────────────────────────

pub(super) fn deserialize_priority<'de, D>(deserializer: D) -> Result<PriorityLevel, D::Error>
where
    D: serde::Deserializer<'de>,
{
    PriorityLevel::deserialize(deserializer)
}

// ── Frontmatter 反序列化 ─────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct CuiFrontmatter {
    pub(super) title: String,
    #[serde(
        deserialize_with = "deserialize_priority",
        default = "default_priority"
    )]
    pub(super) priority: PriorityLevel,
    pub(super) id: Option<String>,
    pub(super) summary: Option<String>,
    #[serde(default)]
    pub(super) inert: bool,
    #[serde(default, alias = "foldable")]
    pub(super) collapsible: bool,
    #[serde(default = "default_true")]
    pub(super) collapsed: bool,
    #[serde(rename = "static", default)]
    pub(super) is_static: bool,
    #[serde(default)]
    pub(super) kind: crate::keyword::ComponentKind,
    #[serde(rename = "type", default)]
    pub(super) component_type: Option<String>,
    #[serde(default)]
    pub(super) handler: Option<String>,
    #[serde(default)]
    pub(super) confidence: Option<f64>,
    #[serde(default)]
    pub(super) trigger: Option<String>,
    #[serde(default)]
    pub(super) inputs: Vec<crate::keyword::IoDef>,
    #[serde(default)]
    pub(super) outputs: Vec<crate::keyword::IoDef>,
    #[serde(default)]
    pub(super) actions: Vec<CuiActionDef>,
    #[serde(default)]
    pub(super) children: Vec<String>,
    #[serde(default)]
    pub(super) source: Option<String>,
    #[serde(default)]
    pub(super) persist: Option<String>,
    #[serde(default)]
    pub(super) entry: bool,
    #[serde(default)]
    pub(super) when: Option<String>,
    #[serde(default)]
    pub(super) visibility: Option<String>,
    #[serde(default)]
    pub(super) budget_ratio: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub(super) pinned: bool,
}

const fn default_priority() -> PriorityLevel {
    PriorityLevel::Normal
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
pub(super) struct CuiActionDef {
    pub(super) id: String,
    pub(super) label: String,
    #[serde(default)]
    pub(super) target: Option<String>,
    #[serde(default)]
    pub(super) show: Option<String>,
    #[serde(default)]
    pub(super) params: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub(super) handler: Option<String>,
}

// ── 可见性解析 ─────────────────────────────────────────────────────

pub(super) fn parse_level(s: &str) -> Option<RenderLevel> {
    match s {
        "hidden" => Some(RenderLevel::Hidden),
        "title" => Some(RenderLevel::Title),
        "summary" => Some(RenderLevel::Summary),
        "normal" | "standard" => Some(RenderLevel::Standard),
        "detailed" => Some(RenderLevel::Detailed),
        _ => None,
    }
}

pub(super) fn parse_show(s: &str) -> Option<crate::action::VisibilityRule> {
    if let Some(max) = s.strip_prefix('<') {
        parse_level(max).map(crate::action::VisibilityRule::LevelLessThan)
    } else if let Some(min) = s.strip_prefix('>') {
        parse_level(min).map(crate::action::VisibilityRule::LevelGreaterThan)
    } else {
        None
    }
}

/// 解析 `visibility` 字符串为 `VisibilityCondition`。
pub(super) fn parse_visibility(raw: &str) -> VisibilityCondition {
    let s = raw.trim();
    if s == "always" {
        return VisibilityCondition::Always;
    }
    if let Some(event) = s
        .strip_prefix("on_trigger(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let event = event.trim();
        if !event.is_empty() {
            return VisibilityCondition::OnTrigger(event.to_string());
        }
    }
    if let Some(value) = s
        .strip_prefix("when(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let value = value.trim();
        if !value.is_empty() {
            return VisibilityCondition::When(value.to_string());
        }
    }
    VisibilityCondition::Always
}

/// 解析 YAML frontmatter + Markdown body。
pub(crate) fn parse_frontmatter_body(content: &str) -> Result<(&str, &str), String> {
    let content = content.trim();
    if !content.starts_with("---") {
        return Err("`.cui` 文件必须以 `---` 开头".into());
    }
    let after_first = &content[3..];
    let end = after_first
        .find("\n---")
        .ok_or_else(|| "未找到 frontmatter 结束标记 `---`".to_string())?;
    let frontmatter = after_first[..end].trim();
    let body = after_first[end + 4..].trim();
    Ok((frontmatter, body))
}

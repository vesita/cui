/// 组件类型分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    /// 块级组件 —— `## [id] title` 标题 + body + actions。
    #[default]
    Block,
    /// 内联组件 —— 仅 body，无标题包装行。
    Inline,
    /// 行动组件 —— 仅 body（如 `[label]` 按钮），无标题包装行。
    Action,
    /// 组件分组（可包含子组件）。
    Group,
}

impl ComponentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentKind::Block => "block",
            ComponentKind::Inline => "inline",
            ComponentKind::Action => "action",
            ComponentKind::Group => "group",
        }
    }

    pub fn is_block_like(&self) -> bool {
        matches!(self, ComponentKind::Block)
    }

    pub fn is_inline(&self) -> bool {
        matches!(self, ComponentKind::Inline)
    }

    pub fn is_action(&self) -> bool {
        matches!(self, ComponentKind::Action)
    }
}

// ── 类型化接口系统 ─────────────────────────────────────────────────

/// 输入/输出的数据类型。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IoType {
    #[default]
    String,
    Integer,
    Float,
    Boolean,
    Path,
    Json,
    Custom(String),
}

/// 输入或输出的参数定义。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IoDef {
    pub name: String,
    #[serde(rename = "type", default)]
    pub io_type: IoType,
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
    #[serde(default)]
    pub default_value: Option<String>,
}

/// 优先级命名层级。
///
/// 变体按升序排列（Minimal < Low < Normal < High < Critical），
/// 使得 `#[derive(Ord)]` 自动给出正确的排序语义。
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLevel {
    Minimal,
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl PriorityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            PriorityLevel::Critical => "critical",
            PriorityLevel::High => "high",
            PriorityLevel::Normal => "normal",
            PriorityLevel::Low => "low",
            PriorityLevel::Minimal => "minimal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_kind_default() {
        assert_eq!(ComponentKind::default(), ComponentKind::Block);
    }

    #[test]
    fn component_kind_as_str() {
        assert_eq!(ComponentKind::Block.as_str(), "block");
        assert_eq!(ComponentKind::Inline.as_str(), "inline");
        assert_eq!(ComponentKind::Action.as_str(), "action");
        assert_eq!(ComponentKind::Group.as_str(), "group");
    }

    #[test]
    fn component_kind_is_block_like() {
        assert!(ComponentKind::Block.is_block_like());
        assert!(!ComponentKind::Inline.is_block_like());
        assert!(!ComponentKind::Action.is_block_like());
        assert!(!ComponentKind::Group.is_block_like());
    }

    #[test]
    fn component_kind_is_inline() {
        assert!(ComponentKind::Inline.is_inline());
        assert!(!ComponentKind::Block.is_inline());
        assert!(!ComponentKind::Action.is_inline());
    }

    #[test]
    fn component_kind_is_action() {
        assert!(ComponentKind::Action.is_action());
        assert!(!ComponentKind::Block.is_action());
        assert!(!ComponentKind::Inline.is_action());
    }

    #[test]
    fn io_def_deserialize_name_only() {
        let yaml = "name: query";
        let def: IoDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "query");
        assert_eq!(def.io_type, IoType::String);
        assert!(!def.required);
        assert!(def.description.is_none());
    }

    #[test]
    fn io_def_deserialize_full() {
        let yaml = concat!(
            "name: path\n",
            "type: path\n",
            "required: true\n",
            "description: 文件路径\n",
            "default_value: /tmp\n",
        );
        let def: IoDef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "path");
        assert_eq!(def.io_type, IoType::Path);
        assert!(def.required);
        assert_eq!(def.description.as_deref(), Some("文件路径"));
        assert_eq!(def.default_value.as_deref(), Some("/tmp"));
    }

    #[test]
    fn priority_level_default() {
        assert_eq!(PriorityLevel::default(), PriorityLevel::Normal);
    }

    #[test]
    fn priority_level_as_str() {
        assert_eq!(PriorityLevel::Critical.as_str(), "critical");
        assert_eq!(PriorityLevel::High.as_str(), "high");
        assert_eq!(PriorityLevel::Normal.as_str(), "normal");
        assert_eq!(PriorityLevel::Low.as_str(), "low");
        assert_eq!(PriorityLevel::Minimal.as_str(), "minimal");
    }

    #[test]
    fn priority_level_ordering() {
        assert!(PriorityLevel::Critical > PriorityLevel::High);
        assert!(PriorityLevel::High > PriorityLevel::Normal);
        assert!(PriorityLevel::Normal > PriorityLevel::Low);
        assert!(PriorityLevel::Low > PriorityLevel::Minimal);
    }

    #[test]
    fn priority_level_deserialize_named() {
        let v: PriorityLevel = serde_yaml::from_str("critical").unwrap();
        assert_eq!(v, PriorityLevel::Critical);
        let v: PriorityLevel = serde_yaml::from_str("low").unwrap();
        assert_eq!(v, PriorityLevel::Low);
    }

    #[test]
    fn priority_level_serialize() {
        let yaml = serde_yaml::to_string(&PriorityLevel::Critical).unwrap();
        assert_eq!(yaml.trim(), "critical");
    }
}

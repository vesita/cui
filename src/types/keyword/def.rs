use super::error::{KeywordError, KeywordErrorKind, KeywordErrorSeverity};

// ── 关键字类别 ─────────────────────────────────────────────────────

/// 关键字类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordCategory {
    /// 组件标识：`id`, `title`
    Identity,
    /// 组件类型：`kind`
    Structure,
    /// 渲染元数据：`priority`, `summary`, `inert`, `static`
    Metadata,
    /// 交互动作：`actions`
    Presentation,
    /// 类型化接口：`inputs`, `outputs`
    Interface,
    /// 框架内部保留（`_` 前缀）
    Internal,
    /// 为未来保留，当前仅警告
    Reserved,
}

// ── 关键字定义 ─────────────────────────────────────────────────────

/// 单个关键字的完整定义。
#[derive(Debug, Clone)]
pub struct KeywordDef {
    pub name: &'static str,
    pub category: KeywordCategory,
    pub required: bool,
    pub default_value: Option<&'static str>,
    pub description: &'static str,
    pub value_type: &'static str,
}

// ── 关键字注册表 ───────────────────────────────────────────────────

/// 关键字注册表 —— 类似编译器的符号表。
///
/// 持有所有已知和保留关键字的定义，提供验证查询。
#[derive(Debug)]
pub struct KeywordRegistry {
    known: &'static [&'static KeywordDef],
    reserved: &'static [&'static str],
}

impl KeywordRegistry {
    /// 创建包含所有当前关键字定义的注册表。
    pub const fn default() -> Self {
        Self {
            known: KNOWN_KEYWORDS,
            reserved: RESERVED_KEYWORDS,
        }
    }

    /// 查找关键字定义。
    pub fn lookup(&self, name: &str) -> Option<&'static KeywordDef> {
        self.known.iter().find(|k| k.name == name).copied()
    }

    /// 检查是否为保留关键字。
    pub fn is_reserved(&self, name: &str) -> bool {
        self.reserved.contains(&name)
    }

    /// 检查是否为内部关键字（`_` 前缀）。
    pub fn is_internal(&self, name: &str) -> bool {
        name.starts_with('_')
    }

    /// 验证关键字名称，返回定义或错误。
    pub fn validate(&self, name: &str) -> Result<&'static KeywordDef, KeywordErrorKind> {
        if let Some(def) = self.lookup(name) {
            Ok(def)
        } else if self.is_internal(name) {
            Err(KeywordErrorKind::Internal(name.to_string()))
        } else if self.is_reserved(name) {
            Err(KeywordErrorKind::Reserved(name.to_string()))
        } else {
            let help = self
                .known
                .iter()
                .map(|k| k.name)
                .collect::<Vec<_>>()
                .join(", ");
            Err(KeywordErrorKind::Unknown {
                name: name.to_string(),
                known: help,
            })
        }
    }

    /// 对 YAML frontmatter 源码进行关键字验证。
    pub fn validate_yaml(
        &self,
        yaml_source: &str,
        line_offset: usize,
    ) -> Result<(), Vec<KeywordError>> {
        let keys = locate_top_level_keys(yaml_source, line_offset);
        let mut errors = Vec::new();

        for (name, pos) in &keys {
            match self.validate(name) {
                Ok(_) => {}
                Err(kind) => {
                    let severity = match &kind {
                        KeywordErrorKind::Unknown { .. } => KeywordErrorSeverity::Error,
                        KeywordErrorKind::Reserved(_) => KeywordErrorSeverity::Warning,
                        KeywordErrorKind::Internal(_) => KeywordErrorSeverity::Warning,
                    };
                    let message = kind.message();
                    let source_line = yaml_source
                        .lines()
                        .nth(pos.line.saturating_sub(line_offset))
                        .map(|s| s.to_string());

                    errors.push(KeywordError {
                        line: pos.line,
                        column: pos.column,
                        severity,
                        message,
                        source_line,
                    });
                }
            }
        }

        let has_error = errors
            .iter()
            .any(|e| e.severity == KeywordErrorSeverity::Error);
        if has_error { Err(errors) } else { Ok(()) }
    }
}

// ── 关键字定义表（编译期常量） ─────────────────────────────────────

const KW_ID: KeywordDef = KeywordDef {
    name: "id",
    category: KeywordCategory::Identity,
    required: false,
    default_value: None,
    description: "组件标识符，默认从文件路径推断",
    value_type: "string",
};

const KW_TITLE: KeywordDef = KeywordDef {
    name: "title",
    category: KeywordCategory::Identity,
    required: true,
    default_value: None,
    description: "组件显示标题",
    value_type: "string",
};

const KW_TYPE: KeywordDef = KeywordDef {
    name: "type",
    category: KeywordCategory::Structure,
    required: false,
    default_value: None,
    description: "语义组件类型（内置: tool、section；项目可通过 TypeRegistry 扩展自定义类型）",
    value_type: "enum",
};

const KW_KIND: KeywordDef = KeywordDef {
    name: "kind",
    category: KeywordCategory::Structure,
    required: false,
    default_value: Some("block"),
    description: "渲染类别覆盖（通常由 type 自动提供）：block、inline、action、group",
    value_type: "enum",
};

const KW_PRIORITY: KeywordDef = KeywordDef {
    name: "priority",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: Some("normal"),
    description: "容量规划优先级：critical、high、normal、low、minimal",
    value_type: "enum",
};

const KW_SUMMARY: KeywordDef = KeywordDef {
    name: "summary",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: None,
    description: "摘要文本，默认取 body 首行",
    value_type: "string",
};

const KW_INERT: KeywordDef = KeywordDef {
    name: "inert",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: Some("false"),
    description: "惰性标记，内容永不变化时设为 true",
    value_type: "boolean",
};

const KW_STATIC: KeywordDef = KeywordDef {
    name: "static",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: Some("false"),
    description: "静态标记，设为 true 即使在预算不足时也保留",
    value_type: "boolean",
};

const KW_ENTRY: KeywordDef = KeywordDef {
    name: "entry",
    category: KeywordCategory::Structure,
    required: false,
    default_value: Some("false"),
    description: "入口标记，设为 true 表示该组件为组件树的根节点",
    value_type: "boolean",
};

const KW_ACTIONS: KeywordDef = KeywordDef {
    name: "actions",
    category: KeywordCategory::Presentation,
    required: false,
    default_value: None,
    description: "交互动作列表",
    value_type: "list",
};

const KW_INPUTS: KeywordDef = KeywordDef {
    name: "inputs",
    category: KeywordCategory::Interface,
    required: false,
    default_value: None,
    description: "组件输入参数定义",
    value_type: "list",
};

const KW_OUTPUTS: KeywordDef = KeywordDef {
    name: "outputs",
    category: KeywordCategory::Interface,
    required: false,
    default_value: None,
    description: "组件输出参数定义",
    value_type: "list",
};

const KW_CHILDREN: KeywordDef = KeywordDef {
    name: "children",
    category: KeywordCategory::Structure,
    required: false,
    default_value: None,
    description: "子组件 ID 引用列表，用于组装组件树",
    value_type: "list",
};

const KW_SOURCE: KeywordDef = KeywordDef {
    name: "source",
    category: KeywordCategory::Structure,
    required: false,
    default_value: None,
    description: "从外部 `.md` 或 `.cui` 文件导入内容作为 body",
    value_type: "string",
};

const KW_PERSIST: KeywordDef = KeywordDef {
    name: "persist",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: None,
    description: "持久化键名，跨会话保留组件数据",
    value_type: "string",
};

const KW_WHEN: KeywordDef = KeywordDef {
    name: "when",
    category: KeywordCategory::Structure,
    required: false,
    default_value: None,
    description: "运行时可见性条件，格式为 {key: xxx, value: yyy}",
    value_type: "object",
};

const KW_VISIBILITY: KeywordDef = KeywordDef {
    name: "visibility",
    category: KeywordCategory::Structure,
    required: false,
    default_value: Some("always"),
    description: "可见性快捷语法：always、on_trigger(event)、when(key=val)",
    value_type: "string",
};

const KW_BUDGET_RATIO: KeywordDef = KeywordDef {
    name: "budget_ratio",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: None,
    description: "分组内子节点预算权重比例（0.0-1.0），仅 kind: group 有效",
    value_type: "float",
};

const KW_SLOTS: KeywordDef = KeywordDef {
    name: "slots",
    category: KeywordCategory::Interface,
    required: false,
    default_value: None,
    description: "body 中 {{var:name}} 的声明列表，加载时验证",
    value_type: "list",
};

const KW_HANDLER: KeywordDef = KeywordDef {
    name: "handler",
    category: KeywordCategory::Presentation,
    required: false,
    default_value: None,
    description: "工具处理器名称（如 tool.read_file），用于 type: tool 的执行动作",
    value_type: "string",
};

const KW_CONFIDENCE: KeywordDef = KeywordDef {
    name: "confidence",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: None,
    description: "置信度（0.0-1.0），用于 LLM 生成的结构化输出",
    value_type: "float",
};

const KW_FOLDABLE: KeywordDef = KeywordDef {
    name: "foldable",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: Some("false"),
    description: "（已弃用，请使用 collapsible）可折叠标记，设为 true 时冷态自动收起为摘要，AI 可手动展开",
    value_type: "boolean",
};

const KW_COLLAPSIBLE: KeywordDef = KeywordDef {
    name: "collapsible",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: Some("false"),
    description: "可折叠标记，设为 true 时冷态自动收起为摘要，AI 可手动展开",
    value_type: "boolean",
};

const KW_TRIGGER: KeywordDef = KeywordDef {
    name: "trigger",
    category: KeywordCategory::Metadata,
    required: false,
    default_value: None,
    description: "触发条件描述，用于程序性记忆的匹配",
    value_type: "string",
};

static KNOWN_KEYWORDS: &[&KeywordDef] = &[
    &KW_ID,
    &KW_TITLE,
    &KW_TYPE,
    &KW_KIND,
    &KW_PRIORITY,
    &KW_SUMMARY,
    &KW_INERT,
    &KW_FOLDABLE,
    &KW_COLLAPSIBLE,
    &KW_STATIC,
    &KW_ENTRY,
    &KW_ACTIONS,
    &KW_INPUTS,
    &KW_OUTPUTS,
    &KW_CHILDREN,
    &KW_SOURCE,
    &KW_PERSIST,
    &KW_WHEN,
    &KW_VISIBILITY,
    &KW_BUDGET_RATIO,
    &KW_SLOTS,
    &KW_HANDLER,
    &KW_CONFIDENCE,
    &KW_TRIGGER,
];

static RESERVED_KEYWORDS: &[&str] = &[
    "version",
    "author",
    "license",
    "tags",
    "category",
    "deprecated",
    "since",
    "until",
    "example",
    "see",
    "aliases",
    "note",
    "env",
];

// ── 源码位置 ───────────────────────────────────────────────────────

/// YAML key 在源码中的位置。
#[derive(Debug, Clone, Copy)]
pub struct KeyPosition {
    pub line: usize,
    pub column: usize,
}

/// 在 YAML 源代码中定位顶层 key 的位置（文本扫描方式）。
fn locate_top_level_keys(yaml_source: &str, line_offset: usize) -> Vec<(String, KeyPosition)> {
    let mut keys = Vec::new();

    for (i, line) in yaml_source.lines().enumerate() {
        let line_number = line_offset + i;

        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        if trimmed.starts_with("- ") {
            continue;
        }

        if let Some(colon_pos) = trimmed.find(':') {
            let key_name = trimmed[..colon_pos].trim();

            if key_name.starts_with('%') || key_name == "---" || key_name == "..." {
                continue;
            }

            if key_name.starts_with('"') || key_name.starts_with('\'') {
                let unquoted = key_name.trim_matches(|c| c == '"' || c == '\'');
                if !unquoted.is_empty() && !unquoted.contains(' ') {
                    let col = line.find(unquoted).unwrap_or(0);
                    keys.push((
                        unquoted.to_string(),
                        KeyPosition {
                            line: line_number,
                            column: col + 1,
                        },
                    ));
                }
                continue;
            }

            if !key_name.is_empty() && !key_name.contains(' ') && !key_name.contains('\t') {
                let col = line.find(key_name).unwrap_or(0);
                keys.push((
                    key_name.to_string(),
                    KeyPosition {
                        line: line_number,
                        column: col + 1,
                    },
                ));
            }
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> KeywordRegistry {
        KeywordRegistry::default()
    }

    #[test]
    fn known_keywords_resolve() {
        let r = registry();
        assert!(r.lookup("id").is_some());
        assert!(r.lookup("title").is_some());
        assert!(r.lookup("kind").is_some());
        assert!(r.lookup("priority").is_some());
        assert!(r.lookup("summary").is_some());
        assert!(r.lookup("inert").is_some());
        assert!(r.lookup("static").is_some());
        assert!(r.lookup("actions").is_some());
        assert!(r.lookup("inputs").is_some());
        assert!(r.lookup("outputs").is_some());
        assert!(r.lookup("children").is_some());
        assert!(r.lookup("source").is_some());
    }

    #[test]
    fn unknown_keyword_returns_none() {
        assert!(registry().lookup("foobar").is_none());
    }

    #[test]
    fn validate_known_keyword_ok() {
        assert!(registry().validate("title").is_ok());
    }

    #[test]
    fn validate_unknown_keyword_error() {
        let err = registry().validate("foobar").unwrap_err();
        match err {
            KeywordErrorKind::Unknown { name, known } => {
                assert_eq!(name, "foobar");
                assert!(known.contains("title"));
            }
            _ => panic!("expected Unknown variant"),
        }
    }

    #[test]
    fn validate_reserved_keyword() {
        let err = registry().validate("version").unwrap_err();
        match err {
            KeywordErrorKind::Reserved(name) => assert_eq!(name, "version"),
            _ => panic!("expected Reserved variant"),
        }
    }

    #[test]
    fn validate_internal_keyword() {
        let err = registry().validate("_internal").unwrap_err();
        match err {
            KeywordErrorKind::Internal(name) => assert_eq!(name, "_internal"),
            _ => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn validate_yaml_no_errors_for_known_keys() {
        let yaml = "title: Hello\npriority: 100\ninert: true";
        let result = registry().validate_yaml(yaml, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_yaml_detects_unknown_key() {
        let yaml = "title: Hello\nfoobar: value";
        let result = registry().validate_yaml(yaml, 2);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, KeywordErrorSeverity::Error);
        assert!(errors[0].message.contains("foobar"));
        assert_eq!(errors[0].line, 3);
    }

    #[test]
    fn validate_yaml_reserved_keyword_warning() {
        let yaml = "title: Hello\nversion: 1.0";
        let result = registry().validate_yaml(yaml, 2);
        assert!(
            result.is_ok(),
            "reserved keywords should not produce errors"
        );
    }

    #[test]
    fn validate_yaml_internal_prefix_warning() {
        let yaml = "title: Hello\n_internal: debug";
        let result = registry().validate_yaml(yaml, 2);
        assert!(
            result.is_ok(),
            "internal keywords should not produce errors"
        );
    }

    #[test]
    fn keyword_defs_have_descriptions() {
        let r = registry();
        for def in r.known {
            assert!(
                !def.description.is_empty(),
                "keyword '{}' missing description",
                def.name
            );
        }
    }

    #[test]
    fn title_is_required() {
        let r = registry();
        let title = r.lookup("title").unwrap();
        assert!(title.required);
    }

    #[test]
    fn id_is_not_required() {
        let r = registry();
        let id = r.lookup("id").unwrap();
        assert!(!id.required);
    }

    #[test]
    fn locate_keys_basic() {
        let yaml = "title: Hello\npriority: 100\n";
        let keys = locate_top_level_keys(yaml, 1);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].0, "title");
        assert_eq!(keys[1].0, "priority");
    }

    #[test]
    fn locate_keys_skips_indented() {
        let yaml = "title: Hello\n  nested: key\nactions:\n  - {id: expand}";
        let keys = locate_top_level_keys(yaml, 1);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].0, "title");
        assert_eq!(keys[1].0, "actions");
    }

    #[test]
    fn locate_keys_with_line_offset() {
        let yaml = "title: Hello\n";
        let keys = locate_top_level_keys(yaml, 10);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].1.line, 10);
    }
}

//! 组件类型注册表 —— 类型定义、合并解析、槽位填充。

use crate::action::ActionDef;
use crate::keyword::{ComponentKind, PriorityLevel};
use crate::runtime::handler::ActionHandlerRef;
use std::collections::HashMap;

/// 组件类型定义的槽位声明。
#[derive(Debug, Clone)]
pub struct SlotDecl {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

/// 语义组件类型定义。
///
/// 每种类型预设默认的渲染类别、动作列表、body 模板等。
/// 实例 `.cui` 文件通过 `type:` 引用类型，实例字段覆盖默认值。
#[derive(Debug, Clone)]
pub struct ComponentTypeDef {
    pub name: String,
    /// 默认渲染类别（实例可通过 `kind:` 覆盖）。
    pub default_kind: ComponentKind,
    /// 类型默认动作列表，与实例动作合并（默认在前，实例在后）。
    pub default_actions: Vec<ActionDef>,
    /// body 模板（可选）。`{{var:name}}` 占位符从实例字段填充。
    pub body_template: Option<String>,
    /// 槽位声明列表。
    pub slots: Vec<SlotDecl>,
    /// 默认优先级（实例可通过 `priority:` 覆盖）。
    pub default_priority: Option<PriorityLevel>,
    /// 默认惰性标记。
    pub default_inert: bool,
    /// 默认静态标记。
    pub default_static: bool,
    /// 类型用途说明。
    pub description: String,
}

/// 类型解析后的完整组件规格。
#[derive(Debug, Clone)]
pub struct ResolvedComponent {
    pub id: String,
    pub title: String,
    pub kind: ComponentKind,
    pub priority: PriorityLevel,
    pub summary: Option<String>,
    pub inert: bool,
    pub is_static: bool,
    pub actions: Vec<ActionDef>,
    pub body: String,
    pub children: Vec<String>,
    pub source: Option<String>,
    pub persist: Option<String>,
    pub entry: bool,
    pub budget_ratio: Option<f32>,
    /// 层级类型的子类标签。如 `type: tool.bash` → subtype = "bash"。
    pub subtype: Option<String>,
}

/// 类型注册表 —— 名称 → ComponentTypeDef 的映射。
#[derive(Debug, Default)]
pub struct TypeRegistry {
    types: HashMap<String, ComponentTypeDef>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    /// 注册一个类型定义。
    pub fn register(&mut self, def: ComponentTypeDef) {
        self.types.insert(def.name.clone(), def);
    }

    /// 按名称查找类型，支持层级回退。
    ///
    /// `tool.bash` → 先查 `tool.bash`，未找到则回退到 `tool`。
    pub fn lookup(&self, name: &str) -> Option<&ComponentTypeDef> {
        if let Some(def) = self.types.get(name) {
            return Some(def);
        }
        // 层级回退：tool.bash → tool
        if let Some(dot) = name.rfind('.') {
            return self.types.get(&name[..dot]);
        }
        None
    }

    /// 列出所有已知类型名。
    pub fn type_names(&self) -> Vec<&str> {
        self.types.keys().map(|s| s.as_str()).collect()
    }

    /// 消费注册表，返回迭代器（用于合并到另一个注册表）。
    pub fn into_types(self) -> impl Iterator<Item = (String, ComponentTypeDef)> {
        self.types.into_iter()
    }

    /// 合并类型默认值与实例字段，产出 ResolvedComponent。
    ///
    /// 合并规则：
    /// - kind: 实例优先，无则用类型 default_kind
    /// - priority: 实例优先，无则用类型 default_priority
    /// - actions: 类型默认在前，实例追加在后
    /// - body: 若类型有 body_template，用实例字段填充 `{{var:name}}`
    /// - inert/static: 实例优先，无则用类型默认
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        &self,
        type_name: &str,
        id: &str,
        title: &str,
        instance_kind: Option<ComponentKind>,
        instance_priority: Option<PriorityLevel>,
        instance_actions: &[ActionDef],
        instance_body: &str,
        instance_summary: Option<&str>,
        instance_inert: Option<bool>,
        instance_static: Option<bool>,
        instance_handler: Option<&str>,
        instance_children: &[String],
        instance_source: Option<&str>,
        instance_persist: Option<&str>,
        instance_entry: bool,
        instance_budget_ratio: Option<f32>,
    ) -> Result<ResolvedComponent, String> {
        let typedef = self.lookup(type_name).ok_or_else(|| {
            format!(
                "未知组件类型 '{}'，已知类型：{}",
                type_name,
                self.type_names().join(", ")
            )
        })?;

        // 提取子类型：tool.bash → subtype = "act"
        let subtype = type_name
            .rfind('.')
            .map(|dot| type_name[dot + 1..].to_string());

        // kind: 实例优先
        let kind = instance_kind.unwrap_or(typedef.default_kind);

        // priority: 实例优先
        let priority = instance_priority
            .or(typedef.default_priority)
            .unwrap_or_default();

        // inert/static: 实例优先
        let inert = instance_inert.unwrap_or(typedef.default_inert);
        let is_static = instance_static.unwrap_or(typedef.default_static);

        // actions: 类型默认在前，实例在后
        let mut actions = typedef.default_actions.clone();

        // 解析默认动作中的 Unresolved handler：从实例字段查找对应值
        for action in &mut actions {
            if let Some(ActionHandlerRef::Unresolved(slot_name)) = action.handler() {
                let resolved = match slot_name.as_str() {
                    "handler" => instance_handler.map(|h| h.to_string()),
                    // 未来扩展：其他 slot 名可映射到实例字段
                    _ => None,
                };
                if let Some(h) = resolved {
                    action.set_handler(ActionHandlerRef::Named(h));
                }
            }
        }

        // 追加实例自定义动作（去重：相同 id 的跳过）
        for inst_action in instance_actions {
            if !actions.iter().any(|a| a.id() == inst_action.id()) {
                actions.push(inst_action.clone());
            }
        }

        // body: 模板填充
        let body = if let Some(ref template) = typedef.body_template {
            fill_template(template, instance_body, instance_handler)
        } else {
            instance_body.to_string()
        };

        Ok(ResolvedComponent {
            id: id.to_string(),
            title: title.to_string(),
            kind,
            priority,
            summary: instance_summary.map(|s| s.to_string()),
            inert,
            is_static,
            actions,
            body,
            children: instance_children.to_vec(),
            source: instance_source.map(|s| s.to_string()),
            persist: instance_persist.map(|s| s.to_string()),
            entry: instance_entry,
            budget_ratio: instance_budget_ratio,
            subtype,
        })
    }
}

/// 简单的键值对模板填充。
///
/// 支持 `{{var:body}}` 和 `{{var:handler}}` 等占位符。
fn fill_template(template: &str, body: &str, handler: Option<&str>) -> String {
    let mut result = template.to_string();
    // {{var:body}}
    result = result.replace("{{var:body}}", body);
    // {{var:handler}}
    if let Some(h) = handler {
        result = result.replace("{{var:handler}}", h);
    } else {
        result = result.replace("{{var:handler}}", "");
    }
    // {{var:title}} — 暂不支持，title 直接从实例取
    result = result.replace("{{var:title}}", "");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_actions() -> Vec<ActionDef> {
        vec![]
    }

    fn make_registry() -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        let mut default_actions = vec![
            ActionDef::new("expand", "展开").with_target_level(crate::level::RenderLevel::Detailed),
            ActionDef::new("execute", "执行")
                .with_handler(ActionHandlerRef::Unresolved("handler".into())),
        ];
        // Remove execute action for test simplicity if not testing slots
        default_actions[1].set_handler(ActionHandlerRef::Unresolved("handler".into()));

        reg.register(ComponentTypeDef {
            name: "tool".into(),
            default_kind: ComponentKind::Block,
            default_actions,
            body_template: Some("{{var:body}}".into()),
            slots: vec![
                SlotDecl {
                    name: "handler".into(),
                    description: "工具处理器".into(),
                    required: true,
                    default: None,
                },
                SlotDecl {
                    name: "body".into(),
                    description: "工具描述".into(),
                    required: true,
                    default: None,
                },
            ],
            default_priority: None,
            default_inert: false,
            default_static: false,
            description: "可执行工具".into(),
        });

        reg.register(ComponentTypeDef {
            name: "section".into(),
            default_kind: ComponentKind::Block,
            default_actions: vec![
                ActionDef::new("expand", "展开")
                    .with_target_level(crate::level::RenderLevel::Detailed),
            ],
            body_template: None,
            slots: vec![],
            default_priority: Some(PriorityLevel::High),
            default_inert: false,
            default_static: false,
            description: "信息分区".into(),
        });

        reg
    }

    #[test]
    fn lookup_known_type() {
        let reg = make_registry();
        assert!(reg.lookup("tool").is_some());
        assert!(reg.lookup("section").is_some());
        assert!(reg.lookup("unknown").is_none());
    }

    #[test]
    fn resolve_tool_fills_handler_slot() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "tool",
                "read",
                "读取文件",
                None,
                None,
                &empty_actions(),
                "读取文件内容",
                None,
                None,
                None,
                Some("tool.read_file"),
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();

        assert_eq!(resolved.kind, ComponentKind::Block);
        assert_eq!(resolved.body, "读取文件内容");
        // 默认动作 + handler slot 已解析
        let execute = resolved
            .actions
            .iter()
            .find(|a| a.id() == "execute")
            .unwrap();
        assert!(
            matches!(execute.handler(), Some(ActionHandlerRef::Named(n)) if n == "tool.read_file")
        );
    }

    #[test]
    fn resolve_instance_actions_appended() {
        let reg = make_registry();
        let custom = vec![ActionDef::new("custom", "自定义")];
        let resolved = reg
            .resolve(
                "tool",
                "x",
                "X",
                None,
                None,
                &custom,
                "body",
                None,
                None,
                None,
                Some("tool.x"),
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        // 默认 expand + execute + 自定义 custom
        let ids: Vec<&str> = resolved.actions.iter().map(|a| a.id()).collect();
        assert_eq!(ids, vec!["expand", "execute", "custom"]);
    }

    #[test]
    fn resolve_instance_kind_overrides() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "tool",
                "x",
                "X",
                Some(ComponentKind::Inline),
                None,
                &empty_actions(),
                "body",
                None,
                None,
                None,
                Some("tool.x"),
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        assert_eq!(resolved.kind, ComponentKind::Inline);
    }

    #[test]
    fn resolve_unknown_type_errors() {
        let reg = make_registry();
        let err = reg
            .resolve(
                "nonexistent",
                "x",
                "X",
                None,
                None,
                &empty_actions(),
                "body",
                None,
                None,
                None,
                None,
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap_err();
        assert!(err.contains("未知组件类型"));
        assert!(err.contains("tool"));
    }

    #[test]
    fn resolve_section_uses_default_priority() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "section",
                "s",
                "分区",
                None,
                None,
                &empty_actions(),
                "内容",
                None,
                None,
                None,
                None,
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        assert_eq!(resolved.priority, PriorityLevel::High);
    }

    #[test]
    fn resolve_section_no_handler_slot() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "section",
                "s",
                "分区",
                None,
                None,
                &empty_actions(),
                "内容",
                None,
                None,
                None,
                None,
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        // section 的 expand 动作没有 handler
        let expand = resolved
            .actions
            .iter()
            .find(|a| a.id() == "expand")
            .unwrap();
        assert!(expand.handler().is_none());
    }

    #[test]
    fn type_names_lists_all() {
        let reg = make_registry();
        let mut names = reg.type_names();
        names.sort();
        assert_eq!(names, vec!["section", "tool"]);
    }

    // ── 层级类型解析 ──────────────────────────────────────────

    #[test]
    fn lookup_hierarchical_fallback() {
        let reg = make_registry();
        // "tool" 精确匹配
        assert!(reg.lookup("tool").is_some());
        // "tool.subtype" → 回退到 "tool"
        let def = reg.lookup("tool.subtype").unwrap();
        assert_eq!(def.name, "tool");
        // 不存在的类型 → None
        assert!(reg.lookup("nonexistent.sub").is_none());
    }

    #[test]
    fn resolve_extracts_subtype_from_dotted_type() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "tool.read_file",
                "read",
                "读取",
                None,
                None,
                &empty_actions(),
                "body",
                None,
                None,
                None,
                Some("tool.read_file"),
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        // 回退到 tool 类型的默认 kind
        assert_eq!(resolved.kind, ComponentKind::Block);
        // subtype 应为 "read_file"
        assert_eq!(resolved.subtype.as_deref(), Some("read_file"));
    }

    #[test]
    fn resolve_no_subtype_for_plain_type() {
        let reg = make_registry();
        let resolved = reg
            .resolve(
                "tool",
                "read",
                "读取",
                None,
                None,
                &empty_actions(),
                "body",
                None,
                None,
                None,
                Some("tool.read_file"),
                &[],
                None,
                None,
                false,
                None,
            )
            .unwrap();
        assert_eq!(resolved.subtype, None);
    }
}

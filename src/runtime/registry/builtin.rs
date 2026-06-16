//! 内置组件类型定义 —— 框架级的通用语义类型。
//!
//! `tool` 和 `section` 是任何 CUI 项目都适用的基础类型。
//! 项目特有类型应通过 `TypeRegistry::register()` 外部注入。

use super::registry::{ComponentTypeDef, SlotDecl, TypeRegistry};
use crate::action::ActionDef;
use crate::keyword::ComponentKind;
use crate::level::RenderLevel;
use crate::runtime::handler::ActionHandlerRef;

/// 创建框架内置的 TypeRegistry（仅 tool + section）。
///
/// 项目特有类型请通过 `registry.register(...)` 扩展后再注入编译器。
pub fn builtin_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();

    // ── tool ──────────────────────────────────────────
    reg.register(ComponentTypeDef {
        name: "tool".into(),
        default_kind: ComponentKind::Block,
        default_actions: vec![
            ActionDef::new("expand", "展开完整描述").with_target_level(RenderLevel::Detailed),
            ActionDef::new("execute", "执行")
                .with_handler(ActionHandlerRef::Unresolved("handler".into())),
            ActionDef::new("collapse", "折叠")
                .with_target_level(RenderLevel::Summary)
                .with_show_when(crate::action::VisibilityRule::LevelGreaterThan(
                    RenderLevel::Summary,
                )),
        ],
        body_template: Some("{{var:body}}".into()),
        slots: vec![
            SlotDecl {
                name: "handler".into(),
                description: "工具处理器名称（如 tool.read_file）".into(),
                required: true,
                default: None,
            },
            SlotDecl {
                name: "body".into(),
                description: "工具描述正文".into(),
                required: true,
                default: None,
            },
        ],
        default_priority: None,
        default_inert: false,
        default_static: false,
        description: "可执行工具组件 —— 提供展开/执行/折叠三个标准动作".into(),
    });

    // ── section ───────────────────────────────────────
    reg.register(ComponentTypeDef {
        name: "section".into(),
        default_kind: ComponentKind::Block,
        default_actions: vec![
            ActionDef::new("expand", "展开").with_target_level(RenderLevel::Detailed),
            ActionDef::new("collapse", "折叠")
                .with_target_level(RenderLevel::Summary)
                .with_show_when(crate::action::VisibilityRule::LevelGreaterThan(
                    RenderLevel::Summary,
                )),
        ],
        body_template: None,
        slots: vec![],
        default_priority: Some(crate::keyword::PriorityLevel::High),
        default_inert: false,
        default_static: false,
        description: "信息分区组件 —— 无执行动作，仅展示内容".into(),
    });

    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_has_tool_and_section() {
        let reg = builtin_registry();
        assert_eq!(reg.type_names().len(), 2);
        assert!(reg.lookup("tool").is_some());
        assert!(reg.lookup("section").is_some());
    }

    #[test]
    fn tool_has_three_default_actions() {
        let reg = builtin_registry();
        let tool = reg.lookup("tool").unwrap();
        assert_eq!(tool.default_actions.len(), 3);
        let ids: Vec<&str> = tool.default_actions.iter().map(|a| a.id()).collect();
        assert_eq!(ids, vec!["expand", "execute", "collapse"]);
    }

    #[test]
    fn tool_execute_has_slot_handler() {
        let reg = builtin_registry();
        let tool = reg.lookup("tool").unwrap();
        let execute = tool
            .default_actions
            .iter()
            .find(|a| a.id() == "execute")
            .unwrap();
        assert!(
            matches!(execute.handler(), Some(ActionHandlerRef::Unresolved(s)) if s == "handler")
        );
    }

    #[test]
    fn section_has_expand_collapse() {
        let reg = builtin_registry();
        let section = reg.lookup("section").unwrap();
        let ids: Vec<&str> = section.default_actions.iter().map(|a| a.id()).collect();
        assert_eq!(ids, vec!["expand", "collapse"]);
    }
}

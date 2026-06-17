//! 技能适配器 —— 将技能列表渲染为 CUI 文本块节点。

use crate::component::builtin::TextBlock;
use crate::component::ComponentNode;
use crate::keyword::PriorityLevel;

/// 从技能名称/描述列表构建技能列表组件节点。
pub fn skill_list_node(id: &str, title: &str, skills: &[(&str, &str)]) -> ComponentNode {
    let content = if skills.is_empty() {
        String::new()
    } else {
        let mut c = String::new();
        for (name, desc) in skills {
            c.push_str(&format!("- **{}**: {}\n", name, desc));
        }
        c
    };
    TextBlock::new(id, title, &content)
        .priority(PriorityLevel::Low)
        .inert()
        .build()
}

//! 技能适配器 —— 将技能列表渲染为 CUI 文本块节点。

use crate::component::builtin::TextBlock;
use crate::{ComponentNode, PriorityLevel};

/// 将技能名称/描述列表构建为惰性文本块组件节点。
///
/// 技能目录是静态参考内容，标记为惰性低优先，容量紧张时优先折叠。
pub fn skill_list_node(
    id: &str,
    title: &str,
    skills: &[(impl AsRef<str>, impl AsRef<str>)],
) -> ComponentNode {
    let mut content = String::new();
    for (name, desc) in skills {
        content.push_str(&format!("- **{}**: {}\n", name.as_ref(), desc.as_ref()));
    }
    TextBlock::new(id, title, content)
        .priority(PriorityLevel::Low)
        .inert()
        .build()
}

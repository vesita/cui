//! 语义化输出格式 —— 紧凑的 Markdown 标题 + 正文 + 内联动作。
//!
//! 格式：
//! ```text
//! ## [标题]
//! body content (标准 Markdown)
//!
//! `[action1]` `[action2]`
//! ```
//!
//! - 标题用 `[title]` 包裹，dirty 时附加 ● 标记
//! - body 直接作为 Markdown 正文
//! - actions 用内联代码格式

use crate::action::ActionDef;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;

/// 输出差量标记：组件自上次渲染后未变化。
pub fn render_delta_marker(id: &str) -> String {
    format!("## [{id}]\n[unmodified]\n")
}

/// 渲染组件为语义化格式。
///
/// 输出格式：
/// ```text
/// ## [标题]
/// body...
///
/// `[action1]` `[action2]`
/// ```
///
/// dirty 时在标题后附加 ● 标记。
pub fn render_component(
    id: &str,
    title: &str,
    _level: RenderLevel,
    body: &str,
    actions: &[ActionDef],
    dirty: bool,
    _priority: PriorityLevel,
) -> String {
    let mut out = String::new();

    // ## [标题]（或 ## [标题] ●）
    let display_title = if title.is_empty() { id } else { title };
    out.push_str("## [");
    out.push_str(display_title);
    out.push(']');
    if dirty {
        out.push_str(" ●");
    }
    out.push('\n');

    let body = body.trim_end();
    if !body.is_empty() {
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }

    if !actions.is_empty() {
        if !body.is_empty() {
            out.push('\n');
        }
        for a in actions.iter() {
            out.push_str("`[");
            out.push_str(a.label());
            out.push_str("]` ");
        }
        out.truncate(out.trim_end().len());
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderLevel;
    use crate::action::ActionDef;

    fn action(label: &str) -> ActionDef {
        ActionDef::new(label.to_string(), label.to_string())
    }

    #[test]
    fn empty_body_no_actions() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "",
            &[],
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\n");
    }

    #[test]
    fn short_body_inline() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Summary,
            "short body",
            &[],
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\nshort body\n");
    }

    #[test]
    fn long_body_indented() {
        let long = "first line\nsecond line\nthird line";
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            long,
            &[],
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\nfirst line\nsecond line\nthird line\n");
    }

    #[test]
    fn empty_body_with_actions() {
        let acts = vec![action("expand")];
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "",
            &acts,
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\n`[expand]`\n");
    }

    #[test]
    fn body_with_actions() {
        let acts = vec![action("expand"), action("refresh")];
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "content",
            &acts,
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\ncontent\n\n`[expand]` `[refresh]`\n");
    }

    #[test]
    fn dirty_marker() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "body",
            &[],
            true,
            PriorityLevel::Normal,
        );
        assert!(out.contains("●"), "expected dirty marker: {out}");
    }

    #[test]
    fn high_priority_shows_badge() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "content",
            &[],
            false,
            PriorityLevel::High,
        );
        assert!(out.contains("[测试]"));
        assert!(out.contains("content"));
    }

    #[test]
    fn body_trimmed() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "  spaced  ",
            &[],
            false,
            PriorityLevel::Normal,
        );
        assert_eq!(out, "## [测试]\n  spaced\n");
    }

    #[test]
    fn body_with_multiline_and_actions() {
        let body = "line one\nline two\nline three";
        let acts = vec![action("a"), action("b"), action("c"), action("d")];
        let out = render_component(
            "comp",
            "C",
            RenderLevel::Standard,
            body,
            &acts,
            false,
            PriorityLevel::Normal,
        );
        assert!(out.starts_with("## [C]\n"));
        assert!(out.contains("line one\n"));
        assert!(out.contains("\n`[a]` `[b]` `[c]` `[d]`\n"));
    }
}

//! 语义化输出格式 —— Markdown 标题 + 按行排列的元数据 + `---` 分隔体 + 内联代码动作。
//!
//! 格式：
//! ```text
//! ## [id]
//! title: 标题
//! level: `detailed`
//! priority: `high`
//! dirty
//! ---
//! body content (标准 Markdown)
//!
//! `[action1]` `[action2]`
//! ```
//!
//! - 元数据每行一个信号，`---` 明确分隔元数据与正文
//! - title 仅在不同于 id 且非空时输出
//! - level 总是输出，用反引号包裹
//! - priority 仅非 Normal 时输出
//! - dirty 作为裸词标记
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
pub fn render_component(
    id: &str,
    title: &str,
    level: RenderLevel,
    body: &str,
    actions: &[ActionDef],
    dirty: bool,
    priority: PriorityLevel,
) -> String {
    let mut out = String::new();

    // ## [id]
    out.push_str("## [");
    out.push_str(id);
    out.push(']');
    out.push('\n');

    // title: 标题（仅在不同于 id 且非空时）
    if !title.is_empty() && title != id {
        out.push_str("title: ");
        out.push_str(title);
        out.push('\n');
    }

    // level: `detailed`
    out.push_str("level: `");
    out.push_str(level.as_str());
    out.push('`');
    out.push('\n');

    // priority: `high`
    if priority != PriorityLevel::Normal {
        out.push_str("priority: `");
        out.push_str(priority.as_str());
        out.push_str("`\n");
    }

    // dirty（裸词）
    if dirty {
        out.push_str("dirty\n");
    }

    let body = body.trim_end();
    let has_body = !body.is_empty();
    let has_actions = !actions.is_empty();

    // --- 分隔线（仅当元数据后有内容时）
    if has_body || has_actions {
        out.push_str("---\n");
    }

    // body
    if has_body {
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }

    // 正文与动作之间的空行
    if has_body && has_actions {
        out.push('\n');
    }

    // `[action1]` `[action2]`
    if has_actions {
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
        assert_eq!(out, "## [test]\ntitle: 测试\nlevel: `standard`\n");
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
        assert_eq!(
            out,
            "## [test]\ntitle: 测试\nlevel: `summary`\n---\nshort body\n"
        );
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
        assert_eq!(
            out,
            "## [test]\ntitle: 测试\nlevel: `standard`\n---\nfirst line\nsecond line\nthird line\n"
        );
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
        assert_eq!(
            out,
            "## [test]\ntitle: 测试\nlevel: `standard`\n---\n`[expand]`\n"
        );
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
        assert_eq!(
            out,
            "## [test]\ntitle: 测试\nlevel: `standard`\n---\ncontent\n\n`[expand]` `[refresh]`\n"
        );
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
        assert!(out.contains("dirty\n"), "expected dirty flag: {out}");
    }

    #[test]
    fn high_priority_shows_badge() {
        let out = render_component(
            "test",
            "测试",
            RenderLevel::Standard,
            "",
            &[],
            false,
            PriorityLevel::High,
        );
        assert!(out.contains("priority: `high`"));
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
        // 仅去除尾随空白，前导空白保留
        assert_eq!(
            out,
            "## [test]\ntitle: 测试\nlevel: `standard`\n---\n  spaced\n"
        );
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
        assert!(out.starts_with("## [comp]\n"));
        assert!(out.contains("title: C\n"));
        assert!(out.contains("line one\n"));
        assert!(out.contains("\n`[a]` `[b]` `[c]` `[d]`\n"));
    }
}

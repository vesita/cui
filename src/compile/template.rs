//! 模板引擎 —— 组合 CUI 组件支持可定制视图。
//!
//! Markdown 模板中嵌入 `{{component id="..." mode=...}}` 指令，
//! 将 CUI 组件按需组合输出。每个组件绑定其前导 prose 为条件块：
//! 空内容 → 整块（prose + component）不输出。
//!
//! # 语法
//!
//! ```markdown
//! ### 目标
//! {{component id="目标" mode=Full}}
//!
//! ### 文件树
//! {{component id="文件树" mode=Truncated(1000)}}
//! ```

/// 读取模式 —— 控制从组件读取内容的量。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ReadMode {
    /// 完整内容。
    #[default]
    Full,
    /// 截断到指定字符数（无省略号）。
    Truncated(usize),
    /// 截断到指定字符数（首尾保留，中间省略）。
    Trimmed(usize),
}

/// 解析后的模板节点。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TemplateNode {
    /// 首部文本（第一个 component 之前），始终输出。
    Lead(String),
    /// 条件块：component 内容为空时整块不输出。
    Block {
        preamble: String,
        directive: ComponentDirective,
    },
    /// 尾部文本（最后一个 component 之后），始终输出。
    Trail(String),
}

/// 一个 `{{component id="..." mode=...}}` 指令。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComponentDirective {
    pub id: String,
    pub mode: ReadMode,
}

/// 模板内容解析器 —— 由 Context 实现。
pub trait TemplateResolver {
    /// 按 component id 读取渲染内容。
    fn read_component(&self, id: &str, mode: ReadMode) -> String;
}

/// 模板引擎 —— 纯函数入口。
pub(crate) struct TemplateEngine;

impl TemplateEngine {
    /// 解析模板字符串为节点列表。
#[cfg(test)]
    pub fn parse(template: &str) -> Vec<TemplateNode> {
        let mut nodes: Vec<TemplateNode> = Vec::new();
        let mut cursor = 0usize;
        let mut had_directive = false;

        while let Some(directive_start) = template[cursor..].find("{{component") {
            let ds = cursor + directive_start;

            let before = &template[cursor..ds];

            let last_newline = before.rfind('\n');
            let line_start = match last_newline {
                Some(nl) => {
                    let prev_line_start = before[..nl].rfind('\n').map(|p| p + 1).unwrap_or(0);
                    cursor + prev_line_start
                }
                None => cursor,
            };
            let preamble = &template[line_start..ds];
            let prose = &template[cursor..line_start];

            let Some(end) = template[ds..].find("}}") else {
                tracing::warn!(
                    "警告 [模板引擎]: 在 {}.. 处发现未闭合的 {{{{component 指令，跳过后续所有指令",
                    &template[ds..ds.saturating_add(40)]
                );
                break;
            };
            let directive_end = ds + end + 2;

            let body = template[ds + "{{component".len()..ds + end].trim();
            let directive = Self::parse_directive(body);

            if let Some(dir) = directive {
                had_directive = true;
                if nodes.is_empty() {
                    nodes.push(TemplateNode::Lead(prose.to_string()));
                } else {
                    if !prose.is_empty() {
                        nodes.push(TemplateNode::Trail(prose.to_string()));
                    }
                }
                nodes.push(TemplateNode::Block {
                    preamble: preamble.to_string(),
                    directive: dir,
                });
            } else {
                let fallback = format!("{}{}{}", prose, preamble, &template[ds..directive_end]);
                if nodes.is_empty() {
                    nodes.push(TemplateNode::Lead(fallback));
                } else {
                    let fb = fallback;
                    match nodes.last_mut() {
                        Some(TemplateNode::Trail(t)) => t.push_str(&fb),
                        _ => nodes.push(TemplateNode::Trail(fb)),
                    }
                }
            }

            cursor = directive_end;
        }

        let remaining = &template[cursor..];
        if !remaining.is_empty() {
            if had_directive {
                match nodes.last_mut() {
                    Some(TemplateNode::Trail(t)) => t.push_str(remaining),
                    _ => nodes.push(TemplateNode::Trail(remaining.to_string())),
                }
            } else {
                nodes.push(TemplateNode::Lead(remaining.to_string()));
            }
        }

        nodes
    }

    /// 渲染解析后的节点列表。
#[cfg(test)]
    pub fn render_nodes(nodes: &[TemplateNode], resolver: &dyn TemplateResolver) -> String {
        let mut out = String::new();
        for node in nodes {
            match node {
                TemplateNode::Lead(text) | TemplateNode::Trail(text) => {
                    out.push_str(text);
                }
                TemplateNode::Block {
                    preamble,
                    directive,
                } => {
                    let content = resolver.read_component(&directive.id, directive.mode);
                    if !content.trim().is_empty() {
                        out.push_str(preamble);
                        out.push_str(content.trim_end());
                        out.push('\n');
                    }
                }
            }
        }
        out
    }

    /// 从 `prompt/escdir/views/{view_name}.cui` 加载模板并渲染。
    #[cfg(feature = "prompts")]
#[cfg(test)]
    pub fn render_view(view_name: &str, resolver: &dyn TemplateResolver) -> String {
        let path = format!(
            "{}/views/{view_name}.cui",
            crate::compile::file::PROMPT_ESCDIR
        );
        let template = crate::CuiFileComponent::from_file(&path)
            .map(|c| c.body().to_string())
            .unwrap_or_default();
        if template.is_empty() {
            return String::new();
        }
        let nodes = Self::parse(&template);
        Self::render_nodes(&nodes, resolver)
    }

    /// 填充 `{{input:name}}` 占位符，将运行时数据注入渲染后的输出。
    ///
    /// 每个输入条目格式为 `(name, value)`。
    /// 单次扫描 O(L)，避免了逐 key 调用 `String::replace()` 的二次方行为。
    /// 填充后仍残留的 `{{input:...}}` 会被清空，防止模板语法泄漏到 AI 输出。
    ///
    /// ```ignore
    /// let output = TemplateEngine::fill_slots(&body, &[("branch", "main"), ("status", "clean")]);
    /// ```
    pub fn fill_slots(output: &str, values: &[(&str, &str)]) -> String {
        let lookup = |name: &str| values.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
        let mut result = String::with_capacity(output.len());
        let mut rest = output;
        let marker = "{{input:";
        while let Some(start) = rest.find(marker) {
            result.push_str(&rest[..start]);
            let after_marker = &rest[start + marker.len()..];
            if let Some(end) = after_marker.find("}}") {
                let name = after_marker[..end].trim();
                if let Some(value) = lookup(name) {
                    result.push_str(value);
                }
                rest = &after_marker[end + 2..];
            } else {
                result.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
        result.push_str(rest);
        result
    }

    /// 解析 `label="xxx" mode=Truncated(200)` 指令体。
#[cfg(test)]
    fn parse_directive(body: &str) -> Option<ComponentDirective> {
        let mut id: Option<String> = None;
        let mut mode = ReadMode::Full;

        let mut rest = body.trim();
        while !rest.is_empty() {
            if let Some(eq) = rest.find('=') {
                let key = rest[..eq].trim();
                let value_start = eq + 1;
                let value_end;
                let raw_value;

                if rest[value_start..].starts_with('"') {
                    let inner = &rest[value_start + 1..];
                    value_end = inner.find('"').map(|q| value_start + 1 + q + 1)?;
                    raw_value = &rest[value_start + 1..value_end - 1];
                } else if rest[value_start..].starts_with(|c: char| c.is_alphabetic()) {
                    let inner = &rest[value_start..];
                    let after_key = if let Some(paren) = inner.find('(') {
                        if let Some(closing) = inner[paren..].find(')') {
                            paren + closing + 1
                        } else {
                            inner.len()
                        }
                    } else {
                        inner
                            .find(|c: char| c.is_whitespace())
                            .unwrap_or(inner.len())
                    };
                    value_end = value_start + after_key;
                    raw_value = &rest[value_start..value_end];
                } else {
                    break;
                }

                match key {
                    "id" => id = Some(raw_value.to_string()),
                    "mode" => mode = parse_mode(raw_value),
                    _ => {}
                }

                rest = rest[value_end..].trim();
            } else {
                break;
            }
        }

        id.map(|l| ComponentDirective { id: l, mode })
    }
}

/// 解析 mode 字符串为 ReadMode。
#[cfg(test)]
fn parse_mode(s: &str) -> ReadMode {
    let s = s.trim();
    if s.eq_ignore_ascii_case("Full") {
        return ReadMode::Full;
    }
    if let Some(rest) = s
        .strip_prefix("Truncated(")
        .or_else(|| s.strip_prefix("truncated("))
        && let Some(num_str) = rest.strip_suffix(')')
        && let Ok(n) = num_str.parse::<usize>()
    {
        return ReadMode::Truncated(n);
    }
    if let Some(rest) = s
        .strip_prefix("Trimmed(")
        .or_else(|| s.strip_prefix("trimmed("))
        && let Some(num_str) = rest.strip_suffix(')')
        && let Ok(n) = num_str.parse::<usize>()
    {
        return ReadMode::Trimmed(n);
    }
    ReadMode::Full
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestResolver(&'static str);

    impl TemplateResolver for TestResolver {
        fn read_component(&self, _id: &str, _mode: ReadMode) -> String {
            self.0.to_string()
        }
    }

    #[test]
    fn empty_template() {
        let nodes = TemplateEngine::parse("");
        assert_eq!(nodes, vec![]);
    }

    #[test]
    fn plain_text_only() {
        let nodes = TemplateEngine::parse("hello world");
        assert_eq!(nodes, vec![TemplateNode::Lead("hello world".into())]);
    }

    #[test]
    fn single_directive() {
        let nodes = TemplateEngine::parse("{{component id=\"target\"}}");
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], TemplateNode::Lead(_)));
        assert!(matches!(&nodes[1], TemplateNode::Block { .. }));
    }

    #[test]
    fn directive_with_mode() {
        let nodes = TemplateEngine::parse("{{component id=\"X\" mode=Truncated(200)}}");
        if let Some(TemplateNode::Block { directive, .. }) = nodes.into_iter().nth(1) {
            assert_eq!(directive.id, "X");
            assert_eq!(directive.mode, ReadMode::Truncated(200));
        } else {
            panic!("expected Block node");
        }
    }

    #[test]
    fn non_empty_component_includes_preamble() {
        let resolver = TestResolver("target content");
        let template = "### 目标\n{{component id=\"target\"}}";
        let nodes = TemplateEngine::parse(template);
        let out = TemplateEngine::render_nodes(&nodes, &resolver);
        assert!(out.contains("### 目标"), "preamble 应输出");
        assert!(out.contains("target content"), "内容应输出");
    }

    #[test]
    fn empty_component_suppresses_block() {
        let resolver = TestResolver("");
        let template = "### 文件树\n{{component id=\"file_tree\"}}";
        let nodes = TemplateEngine::parse(template);
        let out = TemplateEngine::render_nodes(&nodes, &resolver);
        assert!(!out.contains("### 文件树"), "空内容的 preamble 不应输出");
        assert_eq!(out.trim(), "", "空内容的整块应完全移除");
    }

    #[test]
    fn mixed_empty_and_non_empty() {
        let _resolver = TestResolver("");
        let template =
            "头\n### 目标\n{{component id=\"目标\"}}\n### 文件树\n{{component id=\"文件树\"}}\n尾";
        let nodes = TemplateEngine::parse(template);
        // Override per-component resolution
        struct MultiResolver;
        impl TemplateResolver for MultiResolver {
            fn read_component(&self, id: &str, _mode: ReadMode) -> String {
                match id {
                    "目标" => "目标内容".into(),
                    _ => "".into(),
                }
            }
        }
        let out = TemplateEngine::render_nodes(&nodes, &MultiResolver);
        assert!(out.contains("头"), "Lead 应输出");
        assert!(out.contains("### 目标"), "非空内容的 preamble 应输出");
        assert!(out.contains("目标内容"), "非空内容应输出");
        assert!(!out.contains("### 文件树"), "空内容的整块应移除");
        assert!(out.contains("尾"), "Trail 应输出");
    }

    #[test]
    fn directive_unparseable_keeps_as_text() {
        let nodes = TemplateEngine::parse("前 {{component broken=1}} 后");
        let has_lead = nodes.iter().any(|n| matches!(n, TemplateNode::Lead(_)));
        assert!(has_lead, "无法解析的指令应作为文本保留");
    }

    #[test]
    fn read_mode_default_is_full() {
        assert_eq!(ReadMode::default(), ReadMode::Full);
    }

    #[test]
    fn read_mode_truncated_applies() {
        let _resolver = TestResolver("hello world");
        let template = "{{component id=\"x\" mode=Truncated(5)}}";
        let nodes = TemplateEngine::parse(template);
        if let Some(TemplateNode::Block { directive, .. }) = nodes.into_iter().nth(1) {
            assert_eq!(directive.mode, ReadMode::Truncated(5));
        } else {
            panic!("expected Block");
        }
    }

    #[test]
    fn mode_parsing_various_forms() {
        assert_eq!(parse_mode("Full"), ReadMode::Full);
        assert_eq!(parse_mode("Truncated(100)"), ReadMode::Truncated(100));
        assert_eq!(parse_mode("Trimmed(50)"), ReadMode::Trimmed(50));
        assert_eq!(parse_mode("unknown"), ReadMode::Full);
        assert_eq!(parse_mode("Truncated(invalid)"), ReadMode::Full);
    }
}

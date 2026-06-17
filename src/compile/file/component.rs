//! `.cui` 文件组件 —— 解析 frontmatter + body 为 CuiFileComponent。

use std::fs;
use std::path::Path;

/// 内部使用的 escdir 路径（仅框架内部 `render_view` 等少数地方需要）。
pub(crate) const PROMPT_ESCDIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../prompt/escdir");

use crate::action::ActionDef;
use crate::condition::VisibilityCondition;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;
use crate::runtime::handler::ActionHandlerRef;

use super::frontmatter::{
    CuiFrontmatter, parse_frontmatter_body, parse_level, parse_show, parse_visibility,
};

// ── 从 CuiFrontmatter 构建 CuiFileComponent ─────────────────────

impl CuiFileComponent {
    fn from_frontmatter(
        fm: CuiFrontmatter,
        body: String,
        id: String,
        visibility_cond: VisibilityCondition,
    ) -> Self {
        let CuiFrontmatter {
            title,
            priority,
            id: _fm_id,
            summary,
            inert,
            collapsible,
            collapsed,
            is_static,
            kind,
            component_type,
            handler,
            confidence,
            trigger,
            inputs,
            outputs,
            actions: fm_actions,
            children,
            source,
            persist,
            entry,
            when: _when,
            visibility: _visibility,
            budget_ratio,
        } = fm;
        let actions: Vec<ActionDef> = fm_actions
            .into_iter()
            .map(|a| {
                let target_level = a.target.as_deref().and_then(parse_level);
                let show_when = a.show.as_deref().and_then(parse_show);
                let handler = a.handler.map(ActionHandlerRef::Named);
                let mut def = ActionDef::new(a.id, a.label);
                if let Some(tl) = target_level {
                    def = def.with_target_level(tl);
                }
                if let Some(sw) = show_when {
                    def = def.with_show_when(sw);
                }
                if let Some(p) = a.params {
                    def = def.with_params(p);
                }
                if let Some(h) = handler {
                    def = def.with_handler(h);
                }
                def
            })
            .collect();

        Self {
            id,
            title,
            priority,
            summary,
            inert,
            collapsible,
            collapsed,
            is_static,
            kind,
            component_type,
            handler,
            confidence,
            trigger,
            inputs,
            outputs,
            actions,
            body,
            children,
            source,
            persist,
            entry,
            visibility_cond,
            budget_ratio,
        }
    }
}

/// 从 `.cui` 文件解析得到的组件。
#[derive(Debug, Clone)]
pub struct CuiFileComponent {
    id: String,
    title: String,
    priority: PriorityLevel,
    pub(crate) summary: Option<String>,
    inert: bool,
    collapsible: bool,
    collapsed: bool,
    is_static: bool,
    kind: crate::keyword::ComponentKind,
    component_type: Option<String>,
    handler: Option<String>,
    confidence: Option<f64>,
    trigger: Option<String>,
    inputs: Vec<crate::keyword::IoDef>,
    outputs: Vec<crate::keyword::IoDef>,
    actions: Vec<ActionDef>,
    body: String,
    children: Vec<String>,
    source: Option<String>,
    persist: Option<String>,
    entry: bool,
    visibility_cond: VisibilityCondition,
    budget_ratio: Option<f32>,
}

impl CuiFileComponent {
    /// 从 `.cui` 格式字符串解析。
    pub fn from_str(content: &str, default_id: &str) -> Result<Self, String> {
        let (fm_str, body) = parse_frontmatter_body(content)?;
        Self::from_parts(fm_str, body, default_id, None)
    }

    /// 从 `.cui` 文件读取并解析。返回第一个文档。
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取文件失败 {}: {}", path.display(), e))?;
        let default_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let (fm_str, body) = parse_frontmatter_body(&content)?;
        Self::from_parts(fm_str, body, default_id, Some(&path.to_string_lossy()))
    }

    /// 从 `.cui` 文件读取多文档（`---` 分隔），返回全部组件。
    pub fn from_file_multi(path: impl AsRef<Path>) -> Result<Vec<Self>, String> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取文件失败 {}: {}", path.display(), e))?;
        let default_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        crate::compile::compiler::expand_multi_document(&content, default_id)
    }

    /// 从多文档 `.cui` 文件中按 id 查找组件。
    pub fn from_file_find(path: impl AsRef<Path>, id: &str) -> Result<Self, String> {
        Self::from_file_multi(path)?
            .into_iter()
            .find(|c| c.id() == id)
            .ok_or_else(|| format!("多文档文件中未找到 id='{id}' 的组件"))
    }

    fn from_parts(
        fm_str: &str,
        body: &str,
        default_id: &str,
        file_path: Option<&str>,
    ) -> Result<Self, String> {
        let registry = crate::keyword::KeywordRegistry::default();
        if let Err(errors) = registry.validate_yaml(fm_str, 2) {
            let msg = errors
                .iter()
                .map(|e| e.format(file_path))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(msg);
        }

        let fm: CuiFrontmatter = serde_yaml::from_str(fm_str).map_err(|e| {
            let mut msg = format!("frontmatter YAML 解析失败: {}", e);
            if let Some(fp) = file_path {
                msg = format!("{}: {}", fp, msg);
            }
            msg
        })?;

        let id = fm.id.clone().unwrap_or_else(|| default_id.to_string());

        for ch in &['[', ']', '{', '}', '`'] {
            if id.contains(*ch) {
                let mut msg = format!("组件 id '{}' 包含非法字符 '{}'（会破坏输出格式）", id, ch);
                if let Some(fp) = file_path {
                    msg = format!("{}: {}", fp, msg);
                }
                return Err(msg);
            }
        }

        let visibility_cond = if let Some(ref when) = fm.when {
            VisibilityCondition::When(when.clone())
        } else if let Some(ref vis) = fm.visibility {
            parse_visibility(vis)
        } else {
            VisibilityCondition::Always
        };

        Ok(Self::from_frontmatter(
            fm,
            body.to_string(),
            id,
            visibility_cond,
        ))
    }
}

// ── 访问器 ─────────────────────────────────────────────────

impl CuiFileComponent {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn priority(&self) -> PriorityLevel {
        self.priority
    }
    pub fn is_inert(&self) -> bool {
        self.inert
    }
    pub fn collapsible(&self) -> bool {
        self.collapsible
    }
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }
    pub fn is_static(&self) -> bool {
        self.is_static
    }
    pub fn visibility_condition(&self) -> VisibilityCondition {
        self.visibility_cond.clone()
    }
    pub fn component_kind(&self) -> crate::keyword::ComponentKind {
        self.kind
    }
    pub fn component_type(&self) -> Option<&str> {
        self.component_type.as_deref()
    }
    pub fn handler(&self) -> Option<&str> {
        self.handler.as_deref()
    }
    pub fn confidence(&self) -> Option<f64> {
        self.confidence
    }
    pub fn trigger(&self) -> Option<&str> {
        self.trigger.as_deref()
    }
    pub fn inputs(&self) -> &[crate::keyword::IoDef] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[crate::keyword::IoDef] {
        &self.outputs
    }
    pub fn component_children(&self) -> &[String] {
        &self.children
    }
    pub fn component_source(&self) -> Option<&str> {
        self.source.as_deref()
    }
    pub fn persist_key(&self) -> Option<&str> {
        self.persist.as_deref()
    }
    pub fn is_entry(&self) -> bool {
        self.entry
    }
    pub fn actions(&self) -> Vec<ActionDef> {
        self.actions.clone()
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn template(&self, replacements: &[(&str, &str)]) -> String {
        let mut body = self.body.clone();
        for (key, value) in replacements {
            body = body.replace(&format!("{{{}}}", key), value);
        }
        body
    }

    pub fn render_body(&self, level: RenderLevel) -> String {
        match level {
            RenderLevel::Hidden => String::new(),
            RenderLevel::Title => String::new(),
            RenderLevel::Summary => self
                .summary
                .clone()
                .unwrap_or_else(|| self.body.lines().next().unwrap_or("").to_string()),
            RenderLevel::Standard | RenderLevel::Detailed => self.body.clone(),
        }
    }

    pub fn input_values(&self) -> Vec<(String, String)> {
        self.inputs
            .iter()
            .map(|io| {
                let val = io.default_value.as_deref().unwrap_or("");
                (io.name.clone(), val.to_string())
            })
            .collect()
    }

    pub fn budget_ratio(&self) -> Option<f32> {
        self.budget_ratio
    }

    pub fn render_to_string(&self, level: RenderLevel) -> String {
        let body = self.render_body(level);
        if body.is_empty() {
            return String::new();
        }
        crate::types::output::render_component(
            &self.id,
            &self.title,
            level,
            &body,
            &self.actions,
            false,
            self.priority,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyword::ComponentKind;

    #[test]
    fn parse_minimal() {
        let src = "---\ntitle: Hello\n---\nWorld";
        let comp = CuiFileComponent::from_str(src, "test").unwrap();
        assert_eq!(comp.id(), "test");
        assert_eq!(comp.title(), "Hello");
        assert_eq!(comp.priority(), PriorityLevel::Normal);
        assert!(!comp.is_inert());
        assert!(!comp.is_static());
    }

    #[test]
    fn parse_all_fields() {
        let src = concat!(
            "---\n",
            "id: tools/demo\n",
            "title: Demo\n",
            "priority: critical\n",
            "summary: 演示组件\n",
            "inert: true\n",
            "static: true\n",
            "---\n",
            "演示内容\n",
        );
        let comp = CuiFileComponent::from_str(src, "fallback").unwrap();
        assert_eq!(comp.id(), "tools/demo");
        assert_eq!(comp.priority(), PriorityLevel::Critical);
        assert!(comp.is_inert());
        assert!(comp.is_static());
    }

    #[test]
    fn id_fallback_to_default() {
        let src = "---\ntitle: NoId\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "fallback_id").unwrap();
        assert_eq!(comp.id(), "fallback_id");
    }

    #[test]
    fn render_levels() {
        let src = concat!(
            "---\n",
            "id: r\n",
            "title: RenderTest\n",
            "summary: 摘要\n",
            "---\n",
            "完整内容\n第二行\n",
        );
        let comp = CuiFileComponent::from_str(src, "x").unwrap();

        assert_eq!(comp.render_body(RenderLevel::Hidden), "");
        assert_eq!(comp.render_body(RenderLevel::Title), "");
        assert_eq!(comp.render_body(RenderLevel::Summary), "摘要");
        assert_eq!(comp.render_body(RenderLevel::Standard), "完整内容\n第二行");
        assert_eq!(comp.render_body(RenderLevel::Detailed), "完整内容\n第二行");
    }

    #[test]
    fn summary_auto_from_first_line() {
        let src = "---\ntitle: Auto\n---\n首行\n第二行\n";
        let comp = CuiFileComponent::from_str(src, "a").unwrap();
        assert_eq!(comp.render_body(RenderLevel::Summary), "首行");
    }

    #[test]
    fn parse_actions() {
        let src = concat!(
            "---\n",
            "title: WithActions\n",
            "actions:\n",
            "  - {id: expand, label: 展开, target: detailed}\n",
            "  - {id: fold, label: 折叠, target: summary, show: '>summary'}\n",
            "---\n",
            "Body\n",
        );
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.actions().len(), 2);
        assert_eq!(comp.actions()[0].id(), "expand");
        assert_eq!(
            comp.actions()[0].target_level(),
            Some(RenderLevel::Detailed)
        );
        assert!(comp.actions()[0].show_when().is_none());

        assert_eq!(comp.actions()[1].id(), "fold");
        assert_eq!(comp.actions()[1].target_level(), Some(RenderLevel::Summary));
        assert!(comp.actions()[1].show_when().is_some());
    }

    #[test]
    fn missing_frontmatter_error() {
        let err = CuiFileComponent::from_str("not starting with ---", "x").unwrap_err();
        assert!(err.contains("---"), "错误消息应提示 `---`");
    }

    #[test]
    fn missing_closing_error() {
        let err =
            CuiFileComponent::from_str("---\ntitle: T\nbody without closing", "x").unwrap_err();
        assert!(err.contains("---"), "错误消息应提示结束标记");
    }

    #[test]
    fn view_roundtrip_wysiwyg() {
        let src = concat!(
            "---\n",
            "id: tools/read_file\n",
            "title: 📖 读文件\n",
            "---\n",
            "读取文件内容的工具。\n",
            "\n",
            "用法：read_file(path: str)\n",
        );
        let comp = CuiFileComponent::from_str(src, "x").unwrap();

        let normal_body = comp.render_body(RenderLevel::Standard);
        assert_eq!(
            normal_body,
            "读取文件内容的工具。\n\n用法：read_file(path: str)"
        );

        let rendered = comp.render_to_string(RenderLevel::Standard);
        assert!(rendered.starts_with("## ["));
        assert!(rendered.contains("读取文件内容的工具"));
    }

    #[test]
    fn parse_kind_block() {
        let src = "---\ntitle: T\nkind: block\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.component_kind(), ComponentKind::Block);
    }

    #[test]
    fn parse_kind_group() {
        let src = "---\ntitle: T\nkind: group\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.component_kind(), ComponentKind::Group);
    }

    #[test]
    fn parse_kind_inline() {
        let src = "---\ntitle: T\nkind: inline\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.component_kind(), ComponentKind::Inline);
    }

    #[test]
    fn parse_kind_defaults_to_component() {
        let src = "---\ntitle: T\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.component_kind(), ComponentKind::Block);
    }

    #[test]
    fn parse_inputs() {
        let src = concat!(
            "---\n",
            "title: T\n",
            "inputs:\n",
            "  - {name: path, type: path, required: true, description: 文件路径}\n",
            "  - {name: encoding, type: string}\n",
            "---\n",
            "Body\n",
        );
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.inputs().len(), 2);
        assert_eq!(comp.inputs()[0].name, "path");
        assert_eq!(comp.inputs()[0].required, true);
        assert_eq!(comp.inputs()[1].name, "encoding");
        assert!(!comp.inputs()[1].required);
    }

    #[test]
    fn parse_outputs() {
        let src = concat!(
            "---\n",
            "title: T\n",
            "outputs:\n",
            "  - {name: result, type: json, description: 查询结果}\n",
            "---\n",
            "Body\n",
        );
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.outputs().len(), 1);
        assert_eq!(comp.outputs()[0].name, "result");
    }

    #[test]
    fn parse_unknown_keyword_rejected() {
        let src = "---\ntitle: T\nfoobar: value\n---\nBody";
        let err = CuiFileComponent::from_str(src, "x").unwrap_err();
        assert!(err.contains("未知关键字"), "应包含未知关键字错误消息");
        assert!(err.contains("foobar"), "应包含具体关键字名");
    }

    #[test]
    fn parse_reserved_keyword_allowed() {
        let src = "---\ntitle: T\nversion: 1.0\nauthor: me\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.title(), "T", "保留关键字不阻止加载");
    }

    #[test]
    fn parse_internal_keyword_allowed() {
        let src = "---\ntitle: T\n_internal: debug\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        assert_eq!(comp.title(), "T", "内部关键字不阻止加载");
    }

    #[test]
    fn render_to_string_starts_with_hash_bracket() {
        let src = "---\ntitle: T\nkind: group\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        let rendered = comp.render_to_string(RenderLevel::Standard);
        assert!(rendered.starts_with("## "), "新格式应以 ## 开头");
    }

    #[test]
    fn render_to_string_contains_body() {
        let src = "---\ntitle: T\n---\nBody";
        let comp = CuiFileComponent::from_str(src, "x").unwrap();
        let rendered = comp.render_to_string(RenderLevel::Standard);
        assert!(rendered.contains("Body"), "渲染输出应包含 body 内容");
    }
}

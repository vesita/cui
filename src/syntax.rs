//! 代码语法高亮模块 —— 基于 Syntect。
//!
//! 内置 500+ 语言语法定义和多种配色主题，
//! 提供 HTML inline-style 和 CSS class 两种高亮输出。
//!
//! # 用法
//!
//! ```ignore
//! use cui::syntax;
//!
//! // HTML inline-style 输出（用于 Markdown 渲染）
//! let html = syntax::highlight_to_html("fn main() {}", "rust");
//!
//! // token 序列输出（用于编辑器叠加层）
//! for token in syntax::highlight_tokens("let x = 1;", "rust") {
//!     println!("{:?}", token); // (class, text)
//! }
//! ```

use std::sync::OnceLock;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::highlighted_html_for_string;
use syntect::parsing::{SyntaxReference, SyntaxSet};

// ── 公开类型 ────────────────────────────────────────────────

/// 一个高亮 token：`(CSS class name, text)`。
///
/// class name 对应：
/// - `hl-k` — keyword
/// - `hl-s` — string
/// - `hl-c` — comment
/// - `hl-f` — function
/// - `hl-t` — type
/// - `hl-n` — number
/// - `hl-o` — operator
/// - `hl-b` — builtin
/// - `hl-v` — variable
/// - `hl-p` — punctuation
/// - `hl-pl` — plain
pub type Token = (&'static str, String);

// ── 全局单例 ────────────────────────────────────────────────

fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static TS: OnceLock<ThemeSet> = OnceLock::new();
    TS.get_or_init(ThemeSet::load_defaults)
}

static CURRENT_THEME: OnceLock<&'static str> = OnceLock::new();

fn current_theme_name() -> &'static str {
    CURRENT_THEME.get().copied().unwrap_or("base16-ocean.dark")
}

fn current_theme() -> &'static Theme {
    let name = current_theme_name();
    theme_set().themes.get(name).unwrap_or_else(|| {
        theme_set()
            .themes
            .values()
            .next()
            .expect("至少有一个内置主题")
    })
}

// ── 可用主题 ────────────────────────────────────────────────

/// 可用主题列表。
pub static THEMES: &[&str] = &[
    "base16-ocean.dark",
    "base16-ocean.light",
    "InspiredGitHub",
    "Solarized (dark)",
    "Solarized (light)",
    "base16-eighties.dark",
    "base16-mocha.dark",
    "base16-mountain.dark",
    "base16-google.dark",
    "base16-google.light",
];

// ── 公开 API ────────────────────────────────────────────────

/// 切换当前高亮主题。
///
/// 主题名须为 [`THEMES`] 中的一项。不在列表中时静默忽略。
pub fn set_theme(name: &str) {
    if theme_set().themes.contains_key(name) {
        let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
        let _ = CURRENT_THEME.set(leaked);
    }
}

/// 将代码高亮为 HTML `<span style="color:...">` 格式。
///
/// 若 `lang` 为空或不受支持，返回 HTML 转义后的纯文本（无高亮）。
pub fn highlight_to_html(code: &str, lang: &str) -> String {
    let ss = syntax_set();
    let theme = current_theme();

    match find_syntax(ss, lang) {
        Some(syn) => {
            highlighted_html_for_string(code, ss, syn, theme).unwrap_or_else(|_| html_escape(code))
        }
        None => html_escape(code),
    }
}

/// 将代码高亮为 token 序列。
///
/// 若 `lang` 为空或不受支持，返回 `("hl-pl", text)` 单一项。
pub fn highlight_tokens(code: &str, lang: &str) -> Vec<Token> {
    use syntect::util::LinesWithEndings;

    let ss = syntax_set();
    let theme = current_theme();

    let Some(syntax) = find_syntax(ss, lang) else {
        return vec![("hl-pl", code.to_string())];
    };

    let mut hl = syntect::easy::HighlightLines::new(syntax, theme);
    let mut tokens: Vec<Token> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let Ok(ranges) = hl.highlight_line(line, ss) else {
            continue;
        };
        for (style, text) in &ranges {
            let class = style_to_class(style.foreground);
            tokens.push((class, text.to_string()));
        }
    }

    tokens
}

/// 查询 lang 是否受支持。
pub fn is_supported(lang: &str) -> bool {
    find_syntax(syntax_set(), lang).is_some()
}

/// 返回所有受支持的语言名列表。
pub fn list_languages() -> Vec<&'static str> {
    syntax_set()
        .syntaxes()
        .iter()
        .map(|s| s.name.as_str())
        .chain(
            syntax_set()
                .syntaxes()
                .iter()
                .flat_map(|s| &s.file_extensions)
                .map(|ext| ext.as_str()),
        )
        .collect()
}

// ── 内部函数 ────────────────────────────────────────────────

fn find_syntax<'a>(ss: &'a SyntaxSet, lang: &str) -> Option<&'a SyntaxReference> {
    if lang.is_empty() {
        return None;
    }
    ss.find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_extension(lang))
        .or_else(|| {
            let lower = lang.to_ascii_lowercase();
            // 常见别名
            match lower.as_str() {
                "js" | "javascript" | "ecmascript" | "node" | "mjs" => {
                    ss.find_syntax_by_extension("js")
                }
                "ts" | "typescript" => ss.find_syntax_by_extension("ts"),
                "py" | "python" | "python3" => ss.find_syntax_by_extension("py"),
                "rs" | "rust" => ss.find_syntax_by_extension("rs"),
                "html" | "htm" | "xhtml" => ss.find_syntax_by_extension("html"),
                "css" | "scss" | "sass" | "less" => ss.find_syntax_by_extension("css"),
                "json" | "jsonc" => ss.find_syntax_by_extension("json"),
                "yaml" | "yml" => ss.find_syntax_by_extension("yaml"),
                "md" | "markdown" | "mdown" => ss.find_syntax_by_extension("md"),
                "sh" | "bash" | "zsh" | "shell" | "posix" => ss.find_syntax_by_extension("sh"),
                "toml" => ss.find_syntax_by_extension("toml"),
                "xml" | "svg" | "plist" | "xsd" | "xslt" => ss.find_syntax_by_extension("xml"),
                "go" | "golang" => ss.find_syntax_by_extension("go"),
                "rb" | "ruby" => ss.find_syntax_by_extension("rb"),
                "kt" | "kotlin" => ss.find_syntax_by_extension("kt"),
                "swift" => ss.find_syntax_by_extension("swift"),
                "rsx" | "dioxus" => ss.find_syntax_by_extension("rs"),
                "tsx" => ss.find_syntax_by_extension("tsx"),
                "jsx" => ss.find_syntax_by_extension("jsx"),
                "c" => ss.find_syntax_by_extension("c"),
                "cpp" | "c++" | "cc" => ss.find_syntax_by_extension("cpp"),
                "h" | "hpp" => ss.find_syntax_by_extension("h"),
                "dart" => ss.find_syntax_by_extension("dart"),
                "sql" => ss.find_syntax_by_extension("sql"),
                _ => None,
            }
        })
}

/// 将 syntect 颜色映射到 CSS class。
fn style_to_class(fg: syntect::highlighting::Color) -> &'static str {
    // 对 syntect 的颜色做简化分类。
    // 完整的 scope→class 映射需要引入 scope 解析，
    // 这里用一个轻量的启发式：根据颜色明度+色相粗分。
    let luminance = 0.299 * fg.r as f32 + 0.587 * fg.g as f32 + 0.114 * fg.b as f32;

    // 高饱和度颜色通常对应关键字或特殊 token
    let max = fg.r.max(fg.g.max(fg.b)) as f32;
    let min = fg.r.min(fg.g.min(fg.b)) as f32;
    let saturation = if max == 0.0 { 0.0 } else { (max - min) / max };

    match () {
        _ if saturation < 0.15 && luminance > 180.0 => "hl-pl",
        _ if saturation < 0.15 => "hl-c",
        _ if fg.g > 180 && fg.r < 100 && fg.b < 100 => "hl-s",
        _ if fg.b > 150 && fg.r < 150 && fg.g < 150 => "hl-k",
        _ if fg.r > 200 && fg.g < 120 => "hl-f",
        _ if fg.r > 150 && fg.g > 150 && fg.b < 100 => "hl-n",
        _ if saturation > 0.7 && luminance > 100.0 => "hl-k",
        _ => "hl-pl",
    }
}

/// HTML 转义。
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_fn() {
        let html = highlight_to_html("fn main() {}", "rust");
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
        assert!(html.contains("span"));
    }

    #[test]
    fn highlight_unknown_lang_returns_escaped() {
        let html = highlight_to_html("<hello>", "nonexistent_lang_xyz");
        assert!(html.contains("&lt;"));
        assert!(html.contains("&gt;"));
        assert!(!html.contains("<span"));
    }

    #[test]
    fn highlight_empty_lang_returns_escaped() {
        let html = highlight_to_html("test", "");
        assert_eq!(html, "test");
    }

    #[test]
    fn highlight_empty_code() {
        let html = highlight_to_html("", "rust");
        // syntect returns pre-wrapped content; just don't crash
        assert!(!html.contains("fn main"));
    }

    #[test]
    fn tokens_rust() {
        let tokens = highlight_tokens("let x = 1;", "rust");
        assert!(!tokens.is_empty());
        let joined: String = tokens.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(joined, "let x = 1;");
    }

    #[test]
    fn tokens_unknown_lang() {
        let tokens = highlight_tokens("test", "xyz");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].0, "hl-pl");
    }

    #[test]
    fn tokens_empty_code() {
        let tokens = highlight_tokens("", "rust");
        assert!(tokens.is_empty());
    }

    #[test]
    fn is_supported_common_langs() {
        assert!(is_supported("rust"));
        assert!(is_supported("python"));
        assert!(is_supported("javascript"));
        assert!(is_supported("html"));
        assert!(is_supported("css"));
        assert!(!is_supported(""));
    }

    #[test]
    fn list_languages_returns_non_empty() {
        let langs = list_languages();
        assert!(!langs.is_empty());
        assert!(&langs.contains(&"Rust"));
    }

    #[test]
    fn theme_switch_valid() {
        set_theme("InspiredGitHub");
    }

    #[test]
    fn html_escape_basic() {
        assert_eq!(html_escape("<>&"), "&lt;&gt;&amp;");
    }

    #[test]
    fn html_escape_no_change() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    #[test]
    fn tokens_simple_expression() {
        let tokens = highlight_tokens("x + y", "rust");
        assert!(!tokens.is_empty());
    }
}

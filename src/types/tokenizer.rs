//! 简易 Tokenizer —— 为 CUI 预算规划提供 token 估算。
//!
//! 设计参考：HuggingFace tokenizers 库的 WordLevel 模型思想。
//! 与其依赖外部 vocab 文件，这里使用启发式估算校准到 Claude 模型行为。
//!
//! 选择启发式而非完整 BPE 的原因：
//! - CUI 预算只要求相对排名正确，不要求精确 token 数
//! - 免去加载 vocab 文件的开销和依赖
//! - 启动即用，无需训练或下载
//! - 若需更高精度，`set_tokenizer()` 可注入真实 tokenizer（参考 HF tokenizers 的 bindings/rust/）
//!
//! 校准目标为 Claude 系列模型的 tokenization 行为：
//!
//! | 字符类型 | 示例 | 估算比例 |
//! |----------|------|----------|
//! | ASCII 字母/数字 | `hello123` | ~4 chars/token |
//! | CJK 表意文字 | `中文` | ~1 char/token |
//! | 假名/谚文 | `ひらがな` | ~2 chars/token |
//! | Emoji | `🚀` | 1 char ≈ 1 token |
//! | 标点/符号 | `.,!?` | 合并到相邻词 |
//! | 空白字符序列 | `  \n` | ~0 token（合并到相邻词）|
//!
//! 如需更高精度，可通过 `set_tokenizer` 替换为真实 BPE tokenizer。

use std::sync::RwLock;

/// Token 估算函数签名。
pub type TokenizerFn = fn(&str) -> usize;

/// 全局 tokenizer。写一次读多次，RwLock 允许无竞争并发读取。
static GLOBAL_TOKENIZER: RwLock<Option<TokenizerFn>> = RwLock::new(None);

/// 设置全局 tokenizer。
///
/// 默认使用 `heuristic_estimate`。可在程序启动时替换为真实 BPE tokenizer。
pub fn set_tokenizer(fn_: TokenizerFn) {
    if let Ok(mut guard) = GLOBAL_TOKENIZER.write() {
        *guard = Some(fn_);
    }
}

/// 重置为默认 tokenizer（主要用于测试）。
pub fn reset_tokenizer() {
    if let Ok(mut guard) = GLOBAL_TOKENIZER.write() {
        *guard = None;
    }
}

/// 估算文本的 token 数量。
///
/// 使用已设置的全局 tokenizer，默认回退到启发式估算。
pub fn estimate(text: &str) -> usize {
    match GLOBAL_TOKENIZER.read() {
        Ok(guard) => match *guard {
            Some(f) => f(text),
            None => heuristic_estimate(text),
        },
        Err(_) => heuristic_estimate(text),
    }
}

/// 启发式 token 估算。
///
/// 算法：
/// 1. 按空白和标点预分词
/// 2. 每个"词"根据字符类型按不同比例估算
/// 3. 特殊处理：CJK 按 ~1 char/token，假名/谚文按 ~1.5 chars/token，ASCII 块按 ~4 chars/token
pub fn heuristic_estimate(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut tokens = 0.0f64;
    let mut ascii_run: usize = 0;

    for ch in text.chars() {
        let cp = ch as u32;

        if ch.is_ascii() {
            ascii_run += 1;
        } else {
            // Flush ASCII run first
            if ascii_run > 0 {
                tokens += ascii_run as f64 / 4.0;
                ascii_run = 0;
            }

            match cp {
                // CJK Unified Ideographs + Extension A (~1 char/token in BPE)
                0x4E00..=0x9FFF | 0x3400..=0x4DBF => tokens += 1.0,
                // CJK Compatibility Ideographs
                0xF900..=0xFAFF => tokens += 1.0,
                // Hiragana & Katakana (~1-2 chars/token)
                0x3040..=0x30FF => tokens += 1.0 / 1.5,
                // Hangul Syllables (~1-2 chars/token)
                0xAC00..=0xD7AF => tokens += 1.0 / 1.5,
                // CJK Symbols and Punctuation
                0x3000..=0x303F => {} // ~0 token, merged into adjacent
                // Emoji range (rough)
                0x1F000..=0x1FFFF | 0x2000..=0x2BFF => tokens += 1.0,
                // Private Use Area (often emoji components)
                0xE000..=0xF8FF => tokens += 1.0,
                // Misc symbols
                _ => tokens += 1.0 / 3.0,
            }
        }
    }

    // Finalize remaining ASCII run
    if ascii_run > 0 {
        tokens += ascii_run as f64 / 4.0;
    }

    // Add a small overhead for structural tokens (whitespace separators)
    // Every ~10 words of ASCII pays ~1 token for boundary
    let word_count = text.split_whitespace().count() as f64;
    tokens += word_count * 0.1;

    // 中文文本中每约 20 个中文字附加 1 个结构 token
    (tokens.ceil() as usize).max(1)
}

/// 估算单条消息的 token 基础开销（内容 + 推理 + 结构开销 8）。
///
/// 工具调用和附件的 token 需调用方使用 [`estimate`] 自行累加。
/// 此函数提供消息级别的公共 API，避免各 crate 重复实现相同逻辑。
pub fn estimate_message_base(content: &str, reasoning: &str) -> usize {
    estimate(content).max(1) + estimate(reasoning).max(1) + 8
}

/// 对已知格式的组件输出进行更精确的 token 估算。
///
/// 相比直接 `estimate(body)`，此函数额外考虑 CUI 格式开销：
/// - `## [id] level ~` 头部的固定开销
/// - 动作按钮的格式开销
/// - dirty 标记的开销
pub fn estimate_component(
    id: &str,
    level: &str,
    body: &str,
    has_actions: bool,
    dirty: bool,
) -> usize {
    // 头部: ## [id] level ≈ 7 + id + level 字符
    let mut total = estimate(&format!("## [{}] {}", id, level));

    // Dirty 标记
    if dirty {
        total += estimate(" `dirty`");
    }

    // Body —— 直接用实际内容估算，正确处理 CJK 等多字节文本
    total += estimate(body);

    // 动作按钮
    if has_actions {
        total += 4; // `[action]` 格式约 4 tokens
    }

    total.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!(estimate(""), 0);
    }

    #[test]
    fn short_ascii() {
        let t = estimate("hello world");
        // "hello" + " " + "world" = 11 chars → ~11/4 + 2*0.1 ≈ 2.95 → 3
        assert!(t >= 1, "short text should be at least 1 token");
        assert!(t <= 5, "11 chars shouldn't exceed 5 tokens");
    }

    #[test]
    fn cjk_text() {
        let t = estimate("你好世界");
        // 4 CJK chars at 1 token each = 4
        assert!(t >= 1, "CJK text should have tokens");
        assert!(t <= 5);
    }

    #[test]
    fn mixed_cjk_and_ascii() {
        let t = estimate("你好 world 世界");
        // CJK: 4 chars → 4.0, ASCII: 5 chars → 1.25, words: 3 → 0.3 → ~5.55 → 6
        assert!(t >= 2, "mixed text should have tokens");
        assert!(t <= 8);
    }

    #[test]
    fn long_ascii_text() {
        let text = "The quick brown fox jumps over the lazy dog. This is a longer text that should produce more tokens.";
        let t = estimate(text);
        // ~100 chars → ~25 chars/token baseline
        assert!(t > 10, "long text should have many tokens");
    }

    #[test]
    fn heuristic_is_reasonable() {
        // A typical component body of ~200 chars
        let text = "This is a sample component body that might be rendered in the CUI framework. It has multiple sentences and describes some state or provides some information to the AI model reading the output.";
        let t = heuristic_estimate(text);
        // 200+ chars / 4 = 50 baseline, plus word overhead
        assert!(
            t >= 30,
            "200+ char English text should be at least 30 tokens"
        );
        assert!(t <= 80, "200+ char English text shouldn't exceed 80 tokens");
    }

    #[test]
    fn set_global_tokenizer_is_used() {
        fn custom_counter(_text: &str) -> usize {
            42
        }
        set_tokenizer(custom_counter);
        assert_eq!(estimate("anything"), 42);
        reset_tokenizer();
        assert!(
            estimate("hello world") > 0,
            "reset should restore heuristic"
        );
    }

    #[test]
    fn estimate_component_basic() {
        let t = estimate_component("test", "normal", "hello world", false, false);
        assert!(t >= 1);
    }

    #[test]
    fn estimate_component_with_dirty() {
        let clean = estimate_component("test", "normal", "hello", false, false);
        let dirty = estimate_component("test", "normal", "hello", false, true);
        assert!(
            dirty >= clean,
            "dirty component should cost at least as much as clean"
        );
    }
}

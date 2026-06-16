/// 关键字验证错误的内部类型（不含位置信息）。
#[derive(Debug, Clone)]
pub enum KeywordErrorKind {
    /// 未知关键字。
    Unknown { name: String, known: String },
    /// 保留关键字。
    Reserved(String),
    /// 内部关键字（`_` 前缀，仅警告）。
    Internal(String),
}

impl KeywordErrorKind {
    pub(crate) fn message(&self) -> String {
        match self {
            KeywordErrorKind::Unknown { name, known } => {
                format!("未知关键字 '{name}'，可用关键字：{known}")
            }
            KeywordErrorKind::Reserved(name) => {
                format!("关键字 '{name}' 已保留，暂未启用，当前版本会忽略此字段")
            }
            KeywordErrorKind::Internal(name) => {
                format!("关键字 '{name}' 以下划线开头，为框架内部保留")
            }
        }
    }
}

/// 错误严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordErrorSeverity {
    /// 阻止加载。
    Error,
    /// 仅警告，不阻止加载。
    Warning,
}

/// 位置感知的关键字验证错误。
#[derive(Debug, Clone)]
pub struct KeywordError {
    pub line: usize,
    pub column: usize,
    pub severity: KeywordErrorSeverity,
    pub message: String,
    pub source_line: Option<String>,
}

impl KeywordError {
    /// 格式化为编译器风格错误消息。
    ///
    /// 示例：
    /// ```text
    /// error[KW001]: 未知关键字 'foo'
    ///  --> tools/read_file.cui:3:5
    ///   |
    /// 3 | foo: bar
    ///   | ^^^ 此关键字未定义
    /// ```
    pub fn format(&self, file_path: Option<&str>) -> String {
        let severity_tag = match self.severity {
            KeywordErrorSeverity::Error => "error",
            KeywordErrorSeverity::Warning => "warning",
        };

        let code = match self.severity {
            KeywordErrorSeverity::Error => "KW001",
            KeywordErrorSeverity::Warning => "KW002",
        };

        let mut out = String::new();

        out.push_str(&format!("{severity_tag}[{code}]: {}\n", self.message));

        if let Some(fp) = file_path {
            out.push_str(&format!(" --> {fp}:{}:{}\n", self.line, self.column));
        } else {
            out.push_str(&format!(" --> {}:{}\n", self.line, self.column));
        }
        out.push_str("  |\n");

        if let Some(ref source) = self.source_line {
            out.push_str(&format!(" {} | {source}\n", self.line));
            out.push_str(&format!(
                "  | {}\n",
                " ".repeat(self.column.saturating_sub(1))
                    + "^"
                        .repeat(source.len().saturating_sub(self.column.saturating_sub(1)))
                        .as_str()
            ));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_format_contains_position() {
        let err = KeywordError {
            line: 3,
            column: 5,
            severity: KeywordErrorSeverity::Error,
            message: "未知关键字 'foo'".into(),
            source_line: Some("foo: bar".into()),
        };
        let formatted = err.format(Some("test.cui"));
        assert!(formatted.contains("test.cui:3:5"));
        assert!(formatted.contains("error[KW001]"));
        assert!(formatted.contains("foo: bar"));
    }

    #[test]
    fn error_format_without_file() {
        let err = KeywordError {
            line: 5,
            column: 1,
            severity: KeywordErrorSeverity::Warning,
            message: "保留关键字".into(),
            source_line: None,
        };
        let formatted = err.format(None);
        assert!(formatted.contains("5:1"));
        assert!(formatted.contains("warning[KW002]"));
    }
}

//! 渲染级别 —— 控制组件在不同容量压力下的展示粒度。

/// 组件的渲染级别，按详细程度升序排列。
///
/// 布局引擎使用此枚举的偏序关系来决定降级/升级顺序。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderLevel {
    /// 完全隐藏，不渲染任何内容。
    Hidden = 0,
    /// 仅渲染标题。
    Title = 1,
    /// 一句话摘要。
    Summary = 2,
    /// 标准渲染（默认）。
    Standard = 3,
    /// 完整详情。
    Detailed = 4,
}

impl RenderLevel {
    /// 渲染级别总数（用于数组大小常量）。
    pub const VARIANT_COUNT: usize = 5;

    /// 降一级（取更简略的级别），最低到 `Hidden`。
    pub fn degrade(self) -> Self {
        match self {
            Self::Detailed => Self::Standard,
            Self::Standard => Self::Summary,
            Self::Summary => Self::Title,
            Self::Title => Self::Hidden,
            Self::Hidden => Self::Hidden,
        }
    }

    /// 升一级（取更详细的级别），最高到 `Detailed`。
    pub fn upgrade(self) -> Self {
        match self {
            Self::Hidden => Self::Title,
            Self::Title => Self::Summary,
            Self::Summary => Self::Standard,
            Self::Standard => Self::Detailed,
            Self::Detailed => Self::Detailed,
        }
    }

    /// 返回级别的简短字符串标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Title => "title",
            Self::Summary => "summary",
            Self::Standard => "standard",
            Self::Detailed => "detailed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrade_from_detailed() {
        assert_eq!(RenderLevel::Detailed.degrade(), RenderLevel::Standard);
    }

    #[test]
    fn degrade_from_normal() {
        assert_eq!(RenderLevel::Standard.degrade(), RenderLevel::Summary);
    }

    #[test]
    fn degrade_from_title() {
        assert_eq!(RenderLevel::Title.degrade(), RenderLevel::Hidden);
    }

    #[test]
    fn degrade_clamps_at_hidden() {
        assert_eq!(RenderLevel::Hidden.degrade(), RenderLevel::Hidden);
    }

    #[test]
    fn upgrade_from_hidden() {
        assert_eq!(RenderLevel::Hidden.upgrade(), RenderLevel::Title);
    }

    #[test]
    fn upgrade_from_summary() {
        assert_eq!(RenderLevel::Summary.upgrade(), RenderLevel::Standard);
    }

    #[test]
    fn upgrade_clamps_at_detailed() {
        assert_eq!(RenderLevel::Detailed.upgrade(), RenderLevel::Detailed);
    }

    #[test]
    fn as_str_values() {
        assert_eq!(RenderLevel::Hidden.as_str(), "hidden");
        assert_eq!(RenderLevel::Title.as_str(), "title");
        assert_eq!(RenderLevel::Summary.as_str(), "summary");
        assert_eq!(RenderLevel::Standard.as_str(), "standard");
        assert_eq!(RenderLevel::Detailed.as_str(), "detailed");
    }

    #[test]
    fn ordering() {
        assert!(RenderLevel::Hidden < RenderLevel::Title);
        assert!(RenderLevel::Standard < RenderLevel::Detailed);
        assert!(RenderLevel::Detailed > RenderLevel::Summary);
    }
}

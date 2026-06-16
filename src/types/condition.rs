//! 条件渲染系统 —— 组件声明可见性条件，系统在渲染前评估。
//!
//! 对应 UI 框架的 `v-if` / conditional render 概念。
//! 组件注册时声明满足什么条件才渲染，在每次 `render()` 前自动评估。

use std::collections::HashSet;

/// 组件可见性条件。
///
/// 默认是 `Always`（始终可见）。需要条件渲染的组件覆盖
/// `BaseComponent::visibility_condition()` 返回具体条件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VisibilityCondition {
    /// 始终可见（默认）。
    Always,
    /// 当前活跃条件集中包含此值时可见。
    ///
    /// 框架维护一个活跃条件集合（`HashSet<String>`），
    /// 例如同时激活 `"plan"` 和 `"skill_debug"` 两个条件。
    /// `When("plan")` 在条件集中存在 `"plan"` 时可见。
    When(String),
    /// 外部事件触发后可见。
    ///
    /// 系统调用 `ctx.trigger(event)` 时激活，组件变为可见。
    /// 适用于 MCP 工具变更等异步通知场景。
    OnTrigger(String),
}

impl VisibilityCondition {
    /// 便捷构造器：创建基于条件的可见性。
    ///
    /// ```ignore
    /// VisibilityCondition::when("plan")
    /// ```
    pub fn when(value: impl Into<String>) -> Self {
        VisibilityCondition::When(value.into())
    }

    /// 评估此条件在给定系统状态下是否满足。
    ///
    /// - `is_triggered`: 指定外部事件是否已被触发
    /// - `active_conditions`: 当前活跃的条件集合（多个条件可同时活跃）
    pub fn evaluate(
        &self,
        is_triggered: &dyn Fn(&str) -> bool,
        active_conditions: &HashSet<String>,
    ) -> bool {
        match self {
            VisibilityCondition::Always => true,
            VisibilityCondition::When(expected) => active_conditions.contains(expected),
            VisibilityCondition::OnTrigger(event) => is_triggered(event),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_set() -> HashSet<String> {
        HashSet::new()
    }

    fn set_of(values: &[&str]) -> HashSet<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn always_true() {
        assert!(VisibilityCondition::Always.evaluate(&|_| false, &empty_set()));
    }

    #[test]
    fn when_condition_matches() {
        let cond = VisibilityCondition::when("act");
        assert!(cond.evaluate(&|_| false, &set_of(&["act"])));
        assert!(!cond.evaluate(&|_| false, &set_of(&["review"])));
        assert!(!cond.evaluate(&|_| false, &empty_set()));
    }

    #[test]
    fn multi_condition_set() {
        let cond = VisibilityCondition::when("plan");
        let active = set_of(&["plan", "skill_debug"]);
        assert!(cond.evaluate(&|_| false, &active));
    }

    #[test]
    fn on_trigger_when_triggered() {
        let cond = VisibilityCondition::OnTrigger("config_changed".into());
        assert!(cond.evaluate(&|e| e == "config_changed", &empty_set()));
        assert!(!cond.evaluate(&|_| false, &empty_set()));
    }
}

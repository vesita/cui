//! 渲染调度 —— RenderPlan、RenderStats、辅助函数。

use crate::level::RenderLevel;

use crate::component::node::ComponentNode;

/// 动作记录。
#[derive(Clone, Debug)]
pub(crate) struct ActionRecord {
    pub component_title: String,
    pub action: String,
    pub target_level: Option<RenderLevel>,
    pub success: bool,
}

pub(crate) fn render_recent_actions(actions: &[ActionRecord]) -> Option<String> {
    if actions.is_empty() {
        return None;
    }
    let mut out = String::from("## [_recent]\n");
    for r in actions {
        let level_str = r
            .target_level
            .map(|l| format!(" → {}", l.as_str()))
            .unwrap_or_default();
        let status = if r.success { " ✓" } else { " ✗" };
        out.push_str(&format!(
            "  ·[{}] {}{}{}\n",
            r.component_title, r.action, level_str, status
        ));
    }
    Some(out)
}

/// 递归衰减节点的活跃度信号。
pub(crate) fn cool_node_signal(node: &mut ComponentNode) {
    let current_hash = node.info().content_hash.get();
    let info = node.info_mut();
    info.signal.cool();
    if current_hash != info.signal.last_content_hash {
        info.signal.volatility = info.signal.volatility.saturating_add(32);
    } else {
        info.signal.volatility = info.signal.volatility.saturating_sub(4);
    }
    info.signal.last_content_hash = current_hash;
    if let ComponentNode::Composite { children, .. } = node {
        for child in children.iter_mut() {
            cool_node_signal(child);
        }
    }
}

/// 递归规划 Composite 子节点的渲染级别。
///
/// 子预算按优先级权重分配，Critical 子节点获得更多预算。
/// 可折叠组件在折叠态（level < Standard）下子节点强制 Hidden。
/// Leaf 节点无子节点，折叠态通过 render(Summary) 仅返回标题+首行。
pub(crate) fn plan_composite_children(node: &mut ComponentNode, budget: usize) {
    let parent_level = node.level();
    let is_collapsible = node.is_collapsible();
    if let ComponentNode::Composite { children, .. } = node {
        if children.is_empty() {
            return;
        }

        // 可折叠复合组件：折叠态下子节点强制 Hidden
        if is_collapsible && parent_level < RenderLevel::Standard {
            for child in children.iter_mut() {
                child.set_level(RenderLevel::Hidden);
            }
            // 递归处理子 Composite（可能嵌套可折叠组件）
            for child in children.iter_mut() {
                plan_composite_children(child, budget);
            }
            return;
        }

        let heatmap: Vec<u8> = children.iter().map(|c| c.heat()).collect();
        let minimums: Vec<RenderLevel> = children
            .iter()
            .map(|c| crate::runtime::capacity::tier_minimum(c.priority()))
            .collect();
        let child_refs: Vec<&dyn crate::component::base::BaseComponent> =
            children.iter().map(|c| c.component_ref()).collect();

        let child_plan =
            crate::runtime::capacity::plan_tree(&child_refs, budget, &heatmap, &minimums);

        for (ci, ca) in child_plan.assignments.iter().enumerate() {
            if let Some(child) = children.get_mut(ci) {
                let mut level = ca.level;
                if child.is_static() && level < RenderLevel::Summary {
                    level = RenderLevel::Summary;
                }
                child.set_level(level);
            }
        }

        // 冷态可折叠子节点：plan_tree 可能因预算宽松将其升级到 Standard，
        // 但折叠态（Summary）是有意"休息"状态，不自动升级。
        for child in children.iter_mut() {
            clamp_cold_foldable(child);
        }

        // 递归处理子 Composite（优先级加权）
        let weights: Vec<usize> = children
            .iter()
            .map(|c| crate::runtime::capacity::priority_weight(c.priority()).max(1))
            .collect();
        let total_weight: usize = weights.iter().sum::<usize>().max(1);
        for (j, child) in children.iter_mut().enumerate() {
            let sub_budget = (weights.get(j).copied().unwrap_or(1) * budget / total_weight).max(1);
            plan_composite_children(child, sub_budget);
        }
    }
}

/// 冷态可折叠组件钳制：不应被 plan_tree 自动升级到 Standard 以上。
///
/// 可折叠组件的 Summary 是"休息"状态 —— 仅 AI 显式展开（触发 fire() 变热）
/// 后才应保持展开。此函数将 heat==0 的可折叠节点从 > Summary 钳回 Summary。
pub(crate) fn clamp_cold_foldable(node: &mut ComponentNode) {
    if node.is_collapsible()
        && node.is_collapsed()
        && node.heat() == 0
        && node.level() > RenderLevel::Summary
    {
        node.set_level(RenderLevel::Summary);
    }
}

/// 递归对节点及其子节点应用可见性条件。
///
/// 容量规划完成后调用，确保 VisibilityCondition 优先于容量决策。
/// 条件不满足的节点强制设为 Hidden。
pub(crate) fn apply_visibility_to_tree(
    node: &mut ComponentNode,
    triggered: &std::collections::HashSet<String>,
    active_conditions: &std::collections::HashSet<String>,
) {
    let cond = node.visibility_condition();
    if !cond.evaluate(&|e| triggered.contains(e), active_conditions) {
        node.set_level(RenderLevel::Hidden);
    }
    if let ComponentNode::Composite { children, .. } = node {
        for child in children.iter_mut() {
            apply_visibility_to_tree(child, triggered, active_conditions);
        }
    }
}

/// 递归对节点应用 temp_expand 提升。
///
/// 容量规划和可见性评估后调用，确保 temp_expand 对非根节点也生效。
pub(crate) fn apply_temp_expand_to_tree(node: &mut ComponentNode, te_id: &str) {
    if node.id() == te_id && node.level() < RenderLevel::Summary {
        node.set_level(RenderLevel::Summary);
    }
    if let ComponentNode::Composite { children, .. } = node {
        for child in children.iter_mut() {
            apply_temp_expand_to_tree(child, te_id);
        }
    }
}

/// 为 Composite 根节点计算子节点预算。
///
/// 若 Composite 设置了 `budget_ratio`，子预算按比例分配；
/// 否则使用全部预算。
pub(crate) fn composite_child_budget(total_budget: usize, root: &ComponentNode) -> usize {
    if let ComponentNode::Composite {
        budget_ratio: Some(ratio),
        ..
    } = root
    {
        return ((total_budget as f64 * *ratio as f64) as usize).max(1);
    }
    total_budget
}

/// 渲染统计 —— render 后的预算使用反馈。
#[derive(Clone, Debug)]
pub struct RenderStats {
    /// 预算上限（token）。
    pub budget: usize,
    /// 预估实际使用 token 数。
    pub total_estimated: usize,
    /// 预算使用率（0.0 ~ 1.0）。
    pub usage_pct: f64,
    /// 根组件总数。
    pub component_count: usize,
    /// 被隐藏的组件数。
    pub hidden_count: usize,
    /// 各级别组件分布。
    pub level_distribution: Vec<(&'static str, usize)>,
}

/// 低预算阈值：effective budget 低于此值时仅渲染 Critical 的 Title 级别。
pub(crate) const MIN_RENDER_BUDGET: usize = 128;

/// 渲染计划 —— `prepare()` 的输出，`render_plan()` 的输入。
///
/// 包含渲染所需的所有预计算数据，使 `render_plan()` 成为纯函数。
pub struct RenderPlan {
    pub condition_checks: Vec<bool>,
    pub assignments: Vec<crate::runtime::capacity::ComponentAssignment>,
    pub recent_rendered: Option<String>,
    pub hidden_info: Vec<(String, bool, bool, bool)>, // (id, is_inert, is_dirty, is_composite)
    pub overview_expanded: bool,
    pub effective_budget: usize,
    pub total_estimated: usize,
}

impl RenderPlan {
    /// 测试用空 RenderPlan。
    #[cfg(test)]
    pub fn empty_for_test() -> Self {
        Self {
            condition_checks: Vec::new(),
            assignments: Vec::new(),
            recent_rendered: None,
            hidden_info: Vec::new(),
            overview_expanded: false,
            effective_budget: 0,
            total_estimated: 0,
        }
    }

    /// 计算渲染统计信息。
    pub fn stats(&self) -> RenderStats {
        let mut level_counts: std::collections::BTreeMap<RenderLevel, usize> =
            std::collections::BTreeMap::new();
        for a in &self.assignments {
            *level_counts.entry(a.level).or_default() += 1;
        }
        let component_count = self.assignments.len();
        let hidden_count = self
            .assignments
            .iter()
            .filter(|a| a.level == RenderLevel::Hidden)
            .count();

        let level_distribution: Vec<_> = level_counts
            .into_iter()
            .map(|(l, c)| (l.as_str(), c))
            .collect();

        let usage_pct = if self.effective_budget > 0 {
            (self.total_estimated as f64 / self.effective_budget as f64).min(1.0)
        } else {
            0.0
        };

        RenderStats {
            budget: self.effective_budget,
            total_estimated: self.total_estimated,
            usage_pct,
            component_count,
            hidden_count,
            level_distribution,
        }
    }
}

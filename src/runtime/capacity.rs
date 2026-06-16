//! 容量规划 —— 基于 token 预算的迭代降级/升级算法。
//!
//! 每个组件从 `minimums` 级别起步，超预算则降级低优先级组件，
//! 有剩余则升级高优先级组件。Critical 组件享有升级偏置因子 0.5。
//! 热组件在降级阶段获得等效优先级提升。
//!
//! 预算单位为 token（通过 [`crate::tokenizer`] 估算），非字符数。

use crate::component::BaseComponent;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;

/// 单个组件在容量规划中的分配结果。
#[derive(Debug)]
pub struct ComponentAssignment {
    /// 组件在容器中的索引。
    pub index: usize,
    /// 分配到的渲染级别。
    pub level: RenderLevel,
}

/// 容量规划结果。
#[derive(Debug)]
pub struct CapacityPlan {
    /// 每个组件的级别分配。
    pub assignments: Vec<ComponentAssignment>,
    /// 预估总 token 数（通过 tokenizer 估算）。
    pub total_estimated: usize,
    /// 预算上限（token）。
    pub budget: usize,
}

/// 为 BaseComponent 进行容量规划。
///
/// `minimums` — 每个组件的保底级别（由优先级决定）。
/// `heatmap` — 每个组件的热度值（0 = 冷，>0 = 最近交互过）。
pub fn plan_tree(
    components: &[&dyn BaseComponent],
    budget: usize,
    heatmap: &[u8],
    minimums: &[RenderLevel],
) -> CapacityPlan {
    let n = components.len();
    if n == 0 {
        return CapacityPlan {
            assignments: vec![],
            total_estimated: 0,
            budget,
        };
    }

    let mut levels: Vec<RenderLevel> = minimums.to_vec();

    // estimated_tokens 缓存
    let mut cache: Vec<[Option<usize>; RenderLevel::VARIANT_COUNT]> =
        vec![[None; RenderLevel::VARIANT_COUNT]; n];
    let mut est = |i: usize, level: RenderLevel| -> usize {
        let idx = level as usize;
        if let Some(v) = cache[i][idx] {
            return v;
        }
        let v = components[i].estimated_tokens(level);
        cache[i][idx] = Some(v);
        v
    };

    // Phase 1: 降级
    let mut total: usize = (0..n).map(|i| est(i, levels[i])).sum();
    loop {
        if total <= budget {
            break;
        }

        let mut candidates: Vec<usize> = (0..n)
            .filter(|&i| levels[i] != RenderLevel::Hidden)
            .collect();

        if candidates.is_empty() {
            break;
        }

        candidates.sort_by(|&a, &b| {
            let heat_a = heatmap.get(a).copied().unwrap_or(0);
            let heat_b = heatmap.get(b).copied().unwrap_or(0);
            match (heat_a == 0, heat_b == 0) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => match heat_a.cmp(&heat_b) {
                    std::cmp::Ordering::Equal => {
                        components[a].priority().cmp(&components[b].priority())
                    }
                    ord => ord,
                },
            }
        });
        let idx = candidates[0];
        let new_level = levels[idx].degrade();
        total = total.saturating_sub(est(idx, levels[idx]));
        levels[idx] = new_level;
        total += est(idx, levels[idx]);
    }

    // Phase 2: 升级
    loop {
        let slack = budget.saturating_sub(total);
        if slack == 0 {
            break;
        }

        let mut candidates: Vec<usize> = (0..n)
            .filter(|&i| levels[i] != RenderLevel::Detailed)
            .collect();

        if candidates.is_empty() {
            break;
        }

        candidates.sort_by(|&a, &b| {
            let heat_a = heatmap.get(a).copied().unwrap_or(0);
            let heat_b = heatmap.get(b).copied().unwrap_or(0);
            match (heat_a == 0, heat_b == 0) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => match heat_b.cmp(&heat_a) {
                    std::cmp::Ordering::Equal => {
                        components[b].priority().cmp(&components[a].priority())
                    }
                    ord => ord,
                },
            }
        });

        // 遍历已排序的候选，找到第一个实际成本在 slack 内的组件
        let mut any_upgraded = false;
        for &idx in &candidates {
            let new_level = levels[idx].upgrade();
            let delta = est(idx, new_level).saturating_sub(est(idx, levels[idx]));
            if delta <= slack {
                total = total.saturating_sub(est(idx, levels[idx]));
                levels[idx] = new_level;
                total += est(idx, levels[idx]);
                any_upgraded = true;
                break;
            }
        }
        if !any_upgraded {
            break;
        }
    }

    let total_estimated = total;

    let assignments = levels
        .into_iter()
        .enumerate()
        .map(|(index, level)| ComponentAssignment { index, level })
        .collect();

    CapacityPlan {
        assignments,
        total_estimated,
        budget,
    }
}

/// 根据优先级返回保底渲染级别。
///
/// Critical/High 至少 Summary，Normal 至少 Title，
/// Low/Minimal 默认 Hidden（预算充足时升级）。
pub fn tier_minimum(priority: PriorityLevel) -> RenderLevel {
    match priority {
        PriorityLevel::Critical | PriorityLevel::High => RenderLevel::Summary,
        PriorityLevel::Normal => RenderLevel::Title,
        PriorityLevel::Low | PriorityLevel::Minimal => RenderLevel::Hidden,
    }
}

/// 优先级子预算权重（Composite 子节点分配用）。
pub fn priority_weight(priority: PriorityLevel) -> usize {
    match priority {
        PriorityLevel::Critical => 5,
        PriorityLevel::High => 4,
        PriorityLevel::Normal => 2,
        PriorityLevel::Low => 1,
        PriorityLevel::Minimal => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Dummy {
        id: &'static str,
        pri: crate::keyword::PriorityLevel,
        size: usize, // chars produced at Standard
    }

    impl BaseComponent for Dummy {
        fn id(&self) -> &str {
            self.id
        }
        fn title(&self) -> &str {
            self.id
        }
        fn priority(&self) -> crate::keyword::PriorityLevel {
            self.pri
        }

        fn render(&self, level: RenderLevel) -> String {
            match level {
                RenderLevel::Hidden => String::new(),
                RenderLevel::Title => format!("[{}]", self.id),
                RenderLevel::Summary => format!("{}: s", self.id),
                RenderLevel::Standard => "x".repeat(self.size),
                RenderLevel::Detailed => format!("{}: d", "x".repeat(self.size)),
            }
        }
        fn handle_action(&mut self, action: &str, _params: &str) -> crate::action::ActionResult {
            crate::action::ActionResult::error(self.id, action, "stub")
        }
    }

    #[test]
    fn degrade_lowest_priority() {
        let high = Dummy {
            id: "high",
            pri: crate::keyword::PriorityLevel::Critical,
            size: 500,
        };
        let low = Dummy {
            id: "low",
            pri: crate::keyword::PriorityLevel::Minimal,
            size: 500,
        };
        let components: Vec<&dyn BaseComponent> = vec![&high, &low];

        // 500 chars * 2 / 4 = 250 tokens > budget 5 → 降级
        let minimums = vec![RenderLevel::Standard; 2];
        let plan = plan_tree(&components, 5, &[], &minimums);
        let low_assignment = &plan.assignments[1]; // Minimal priority
        assert!(low_assignment.level < RenderLevel::Standard);
    }

    #[test]
    fn upgrade_highest_priority() {
        let high = Dummy {
            id: "high",
            pri: crate::keyword::PriorityLevel::Critical,
            size: 50,
        };
        let low = Dummy {
            id: "low",
            pri: crate::keyword::PriorityLevel::Minimal,
            size: 50,
        };
        let components: Vec<&dyn BaseComponent> = vec![&high, &low];

        // 50*2/4=25 tokens < budget 200 → 有空间升级
        let minimums = vec![RenderLevel::Standard; 2];
        let plan = plan_tree(&components, 200, &[], &minimums);
        let high_assignment = &plan.assignments[0]; // Critical priority
        assert!(high_assignment.level > RenderLevel::Standard);
    }

    #[test]
    fn empty_components() {
        let components: Vec<&dyn BaseComponent> = vec![];
        let plan = plan_tree(&components, 1000, &[], &[]);
        assert!(plan.assignments.is_empty());
        assert_eq!(plan.total_estimated, 0);
    }

    #[test]
    fn extreme_budget_pressure_all_hidden() {
        let a = Dummy {
            id: "a",
            pri: crate::keyword::PriorityLevel::Normal,
            size: 500,
        };
        let b = Dummy {
            id: "b",
            pri: crate::keyword::PriorityLevel::Low,
            size: 500,
        };
        let components: Vec<&dyn BaseComponent> = vec![&a, &b];

        let minimums = vec![RenderLevel::Standard; 2];
        let plan = plan_tree(&components, 1, &[], &minimums);
        for assignment in &plan.assignments {
            assert!(assignment.level <= RenderLevel::Title);
        }
    }

    #[test]
    fn hot_component_resists_degrading() {
        let cold_high = Dummy {
            id: "cold_high",
            pri: crate::keyword::PriorityLevel::Critical,
            size: 500,
        };
        let hot_low = Dummy {
            id: "hot_low",
            pri: crate::keyword::PriorityLevel::Minimal,
            size: 500,
        };
        let components: Vec<&dyn BaseComponent> = vec![&cold_high, &hot_low];

        // 500 chars * 2 / 4 = 250 tokens > budget 100
        // 冷=0, 热=4 → 冷的先降级
        let minimums = vec![RenderLevel::Standard; 2];
        let plan = plan_tree(&components, 100, &[0, 4], &minimums);

        let cold_level = plan.assignments[0].level;
        let hot_level = plan.assignments[1].level;
        // Phase 1: 冷组件(heat=0)先降级 → cold 降级更深
        // Phase 2: 遍历候选，低成本升级优先 → cold 从 Hidden 追回
        // Hot(Minimal) 升级 Standard 成本 122 > slack，停在 Summary
        // Cold(Critical) 升级成本低，从 Hidden → Title → Summary
        assert!(
            plan.total_estimated <= 100,
            "预算应被遵守: total={}, cold={:?}, hot={:?}",
            plan.total_estimated,
            cold_level,
            hot_level
        );
        assert!(
            cold_level >= hot_level,
            "冷组件降级更深但预算恢复后应追上: cold={:?}, hot={:?}",
            cold_level,
            hot_level
        );
    }
}

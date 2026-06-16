//! 组件排序与缓存优化。
//!
//! # 缓存优化原理
//!
//! LLM API（Claude 等）的 prompt caching 基于**前缀匹配**——
//! 如果渲染输出的前缀部分在两轮之间不变，则命中缓存。
//! 前缀之后的变化不影响已缓存的 prefix。
//!
//! [`CacheOptimized`] 策略将稳定组件（static、inert、低频 dirty）排在前面，
//! 频繁变化的组件（对话、git status）排在后面。这样稳定组件的输出始终在同一位置，
//! 形成稳定的缓存前缀。

use crate::component::ComponentNode;

/// 组件排序策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderingStrategy {
    /// 按优先级降序（默认行为）。
    #[default]
    ByPriority,
    /// 按 ID 字典序稳定排序。新增/移除只影响局部。
    Stable,
    /// 缓存优化排序：稳定内容在前，频繁变化内容在后。
    ///
    /// 排序键（贪心：以实际内容波动率为首要信号）：
    /// 1. `volatility` — 低优先（内容变化越少越靠前 → 缓存前缀）
    /// 2. `is_collapsible` — false 优先（不可折叠 = 稳定结构骨架）
    /// 3. `priority` — 高优先（关键指令始终在前，不受 heat 干扰）
    /// 4. `is_inert` — false 优先（惰性参考材料最后）
    /// 5. `is_static` — true 优先
    /// 6. `heat` — 高热优先（AI 刚交互过的紧随关键指令）
    /// 7. `dirty_count` — 低优先
    /// 8. `id` — 字典序确定最终顺序
    CacheOptimized,
}

/// 手动重排的目标位置。
#[derive(Debug, Clone)]
pub enum OrderPosition {
    First,
    Last,
    Before(String),
    After(String),
    At(usize),
}

/// 按策略排序一组组件引用。
///
/// 返回重排后的索引列表。不修改原切片。
pub(crate) fn sort_indices(nodes: &[&ComponentNode], strategy: OrderingStrategy) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..nodes.len()).collect();
    match strategy {
        OrderingStrategy::ByPriority => {
            indices.sort_by(|&a, &b| {
                nodes[b]
                    .priority()
                    .cmp(&nodes[a].priority())
                    .then_with(|| nodes[a].id().cmp(nodes[b].id()))
            });
        }
        OrderingStrategy::Stable => {
            indices.sort_by(|&a, &b| nodes[a].id().cmp(nodes[b].id()));
        }
        OrderingStrategy::CacheOptimized => {
            indices.sort_by(|&a, &b| {
                // volatility: low first — 内容稳定的组件形成缓存前缀
                let a_vol = nodes[a].volatility();
                let b_vol = nodes[b].volatility();
                a_vol
                    .cmp(&b_vol)
                    // is_collapsible: false first — 不可折叠组件是稳定结构骨架
                    .then_with(|| {
                        let a_foldable = nodes[a].is_collapsible();
                        let b_foldable = nodes[b].is_collapsible();
                        a_foldable.cmp(&b_foldable)
                    })
                    // priority: high first — 关键指令始终在前
                    .then_with(|| nodes[b].priority().cmp(&nodes[a].priority()))
                    // is_inert: false first — 惰性参考材料放到最后
                    .then_with(|| {
                        let a_inert = nodes[a].is_inert();
                        let b_inert = nodes[b].is_inert();
                        a_inert.cmp(&b_inert)
                    })
                    // is_static: true first
                    .then_with(|| {
                        let a_static = nodes[a].is_static();
                        let b_static = nodes[b].is_static();
                        b_static.cmp(&a_static)
                    })
                    // heat: high first — AI 刚交互过的紧随关键指令
                    .then_with(|| {
                        let a_heat = nodes[a].heat();
                        let b_heat = nodes[b].heat();
                        b_heat.cmp(&a_heat)
                    })
                    // dirty_count: low first
                    .then_with(|| {
                        let a_dc = nodes[a].dirty_count();
                        let b_dc = nodes[b].dirty_count();
                        a_dc.cmp(&b_dc)
                    })
                    // id: alphabetical
                    .then_with(|| nodes[a].id().cmp(nodes[b].id()))
            });
        }
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::builtin::TextBlock;

    #[test]
    fn sort_by_priority() {
        let a = TextBlock::new("a", "A", "low")
            .priority(crate::keyword::PriorityLevel::Low)
            .build();
        let b = TextBlock::new("b", "B", "high")
            .priority(crate::keyword::PriorityLevel::High)
            .build();
        let nodes = [&a, &b];
        let indices = sort_indices(&nodes, OrderingStrategy::ByPriority);
        // b (High) should come before a (Low)
        assert_eq!(indices[0], 1); // b
        assert_eq!(indices[1], 0); // a
    }

    #[test]
    fn sort_stable() {
        let z = TextBlock::new("z", "Z", "").build();
        let a = TextBlock::new("a", "A", "").build();
        let m = TextBlock::new("m", "M", "").build();
        let nodes = [&z, &a, &m];
        let indices = sort_indices(&nodes, OrderingStrategy::Stable);
        assert_eq!(indices[0], 1); // a
        assert_eq!(indices[1], 2); // m
        assert_eq!(indices[2], 0); // z
    }

    #[test]
    fn sort_cache_optimized_non_foldable_first() {
        // non-collapsible nodes go before collapsible ones
        let normal = TextBlock::new("normal", "Normal", "").build();
        let collapsible = TextBlock::new("collapsible", "Foldable", "")
            .collapsible()
            .collapsed(true)
            .build();
        // Ensure both have heat=0, same priority
        let nodes: [&ComponentNode; 2] = [&collapsible, &normal];
        let indices = sort_indices(&nodes, OrderingStrategy::CacheOptimized);
        // non-collapsible (normal) should come first
        assert_eq!(indices[0], 1); // normal (non-collapsible)
        assert_eq!(indices[1], 0); // collapsible
    }

    #[test]
    fn sort_cache_optimized_heat_before_cold() {
        let mut hot = TextBlock::new("hot", "Hot", "").build();
        hot.mark_dirty(); // sets heat=4
        let cold = TextBlock::new("cold", "Cold", "").build();
        let nodes: [&ComponentNode; 2] = [&cold, &hot];
        let indices = sort_indices(&nodes, OrderingStrategy::CacheOptimized);
        // hot should come first
        assert_eq!(indices[0], 1); // hot
        assert_eq!(indices[1], 0); // cold
    }

    #[test]
    fn sort_cache_optimized_inert_last() {
        let inert = TextBlock::new("inert", "Inert", "").inert().build();
        let normal = TextBlock::new("normal", "Normal", "").build();
        let nodes: [&ComponentNode; 2] = [&inert, &normal];
        let indices = sort_indices(&nodes, OrderingStrategy::CacheOptimized);
        // non-inert (normal) should come first, inert last
        assert_eq!(indices[0], 1); // normal
        assert_eq!(indices[1], 0); // inert
    }

    #[test]
    fn sort_cache_optimized_volatility_first() {
        // Low volatility = stable content → should go first for cache prefix
        let mut stable = TextBlock::new("stable", "Stable", "").build();
        stable.set_volatility(8);
        let mut volatile = TextBlock::new("volatile", "Volatile", "").build();
        volatile.set_volatility(200);
        let nodes: [&ComponentNode; 2] = [&volatile, &stable];
        let indices = sort_indices(&nodes, OrderingStrategy::CacheOptimized);
        assert_eq!(indices[0], 1); // stable (low volatility)
        assert_eq!(indices[1], 0); // volatile (high volatility)
    }

    #[test]
    fn default_strategy_is_by_priority() {
        assert_eq!(OrderingStrategy::default(), OrderingStrategy::ByPriority);
    }
}

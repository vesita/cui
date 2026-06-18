//! 组件树 —— ComponentTree + StateEntry。

use std::collections::{HashMap, HashSet};

use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;
use crate::runtime::ordering::{self, OrderPosition, OrderingStrategy};

use super::base::CuiComponent;
use super::iter::{AllNodes, AllNodesMut};
use super::node::ComponentNode;
use super::snapshot::{NodeSnapshot, TreeSnapshot, TreeStats};
use crate::runtime::schedule::{
    self, ActionRecord, MIN_RENDER_BUDGET, RenderPlan, apply_temp_expand_to_tree,
    apply_visibility_to_tree, cool_node_signal, plan_composite_children,
    render_recent_actions,
};

/// 带 TTL 的组件状态条目。
#[derive(Clone, Debug)]
pub struct StateEntry {
    /// 状态值。
    pub value: String,
    /// 最后访问时间（wall-clock），用于 TTL 清理。
    pub last_accessed: std::time::Instant,
}

impl StateEntry {
    /// 创建新条目，记录当前时间。
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            last_accessed: std::time::Instant::now(),
        }
    }

    /// 触摸（更新访问时间）。
    pub fn touch(&mut self) {
        self.last_accessed = std::time::Instant::now();
    }

    /// 检查条目是否已过期（超过 `max_age` 未被访问）。
    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        self.last_accessed.elapsed() > max_age
    }
}

/// 组件树 —— 顶层容器，管理一组 ComponentNode。
pub struct ComponentTree {
    roots: Vec<ComponentNode>,
    /// 组件排序策略。
    ordering: OrderingStrategy,
    /// 全局状态（跨组件共享的键值对状态）。
    global_state: HashMap<String, String>,
    /// 按组件 ID 命名的组件级状态。
    component_state: HashMap<String, HashMap<String, StateEntry>>,
    /// 已触发的异步事件。
    triggered: HashSet<String>,
    /// 当前活跃的条件集合。`When(val)` 组件在 `val` 属于此集合时可见。
    active_conditions: HashSet<String>,
    /// Toast 式临时展开：(id, expires_at)。
    /// `expires_at` 为绝对 tick 值，当 `tick >= expires_at` 时自动失效。
    temp_expand: Option<(String, u64)>,
    recent: Vec<ActionRecord>,
    recent_ticks: u8,
    overview_expanded: bool,
    delta_mode: bool,
}

impl Default for ComponentTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentTree {
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            ordering: OrderingStrategy::default(),
            global_state: HashMap::new(),
            component_state: HashMap::new(),
            triggered: HashSet::new(),
            active_conditions: HashSet::new(),
            temp_expand: None,
            recent: Vec::new(),
            recent_ticks: 0,
            overview_expanded: false,
            delta_mode: false,
        }
    }

    pub fn push(&mut self, node: ComponentNode) {
        debug_assert!(
            self.roots.iter().all(|n| n.id() != node.id()),
            "重复组件 ID: {} 已存在于树中",
            node.id()
        );
        self.roots.push(node);
        let last = self.roots.last_mut().expect("刚 push 的节点应存在");
        if let Some(lc) = &mut last.info_mut().lifecycle {
            lc.on_mount();
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<ComponentNode> {
        // 先查根级别
        if let Some(idx) = self.roots.iter().position(|n| n.id() == id) {
            if let Some(lc) = &mut self.roots[idx].info_mut().lifecycle {
                lc.on_unmount();
            }
            if self
                .temp_expand
                .as_ref()
                .is_some_and(|(te_id, _)| te_id == id)
            {
                self.temp_expand = None;
            }
            return Some(self.roots.remove(idx));
        }
        // 递归搜索 Composite 子节点
        for root in self.roots.iter_mut() {
            if let Some(removed) = root.remove_child(id) {
                if self
                    .temp_expand
                    .as_ref()
                    .is_some_and(|(te_id, _)| te_id == id)
                {
                    self.temp_expand = None;
                }
                return Some(removed);
            }
        }
        None
    }

    pub fn clear(&mut self) {
        self.roots.clear();
        self.temp_expand = None;
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// 设置组件排序策略。
    pub fn set_ordering(&mut self, strategy: OrderingStrategy) {
        self.ordering = strategy;
    }

    /// 获取当前排序策略。
    pub fn ordering(&self) -> OrderingStrategy {
        self.ordering
    }

    /// 物理重排根组件顺序以优化 LLM 缓存命中率。
    ///
    /// 按缓存优化键（volatility↗ / collapsible↗ / priority↘ / inert↗ /
    /// static↘ / heat↘ / dirty_count↗ / id↗）排序 `self.roots`。
    ///
    /// 确保 `_cui_recent` / `_cui_overview` 元数据组件已注册，并将
    /// `_cui_recent`（volatility 最高）推到末尾。
    pub fn reorder_roots(&mut self) {
        self.ensure_meta_components();
        self.roots
            .sort_by(|a, b| ordering::cache_optimized_cmp(a, b));
    }

    /// 确保元数据组件（`_cui_recent` / `_cui_overview`）存在。
    fn ensure_meta_components(&mut self) {
        let has_recent = self.roots.iter().any(|n| n.id() == "_cui_recent");
        let has_overview = self.roots.iter().any(|n| n.id() == "_cui_overview");

        if !has_recent {
            let node = crate::component::builtin::CuiFileLeaf::new(
                "_cui_recent", "_recent", "",
            )
            .kind(crate::keyword::ComponentKind::Inline)
            .priority(PriorityLevel::Low)
            .inert()
            .build();
            self.roots.push(node);
        }
        if !has_overview {
            let node = crate::component::builtin::CuiFileLeaf::new(
                "_cui_overview", "概述", "",
            )
            .kind(crate::keyword::ComponentKind::Inline)
            .priority(PriorityLevel::Low)
            .inert()
            .build();
            self.roots.push(node);
        }
    }

    /// 将 `_cui_recent` / `_cui_overview` 与最新数据同步。
    ///
    /// `recent_rendered` 为 `render_recent_actions` 的输出（clone 后的值）。
    /// `hidden_info` 为隐藏组件信息列表。
    fn sync_meta_components(&mut self, recent_rendered: Option<&str>, hidden_info: &[(String, bool, bool, bool)]) {
        use crate::component::builtin::CuiFileLeaf;

        // 更新 _cui_recent 身体内容
        if let Some(node) = self.find_mut("_cui_recent") {
            let new_body = recent_rendered.unwrap_or("").to_string();
            let changed = if let Some(leaf) = node
                .component_mut()
                .as_any_mut()
                .and_then(|a| a.downcast_mut::<CuiFileLeaf>())
            {
                let changed = leaf.body != new_body;
                leaf.body = new_body;
                changed
            } else {
                false
            };
            if changed {
                node.mark_dirty();
            }
        }
        // 更新 _cui_overview 身体内容
        if let Some(node) = self.find_mut("_cui_overview") {
            let new_body = if hidden_info.is_empty() {
                String::new()
            } else {
                let mut overview = String::from("## [_overview]\n  ");
                let ids: Vec<String> = hidden_info
                    .iter()
                    .map(|(id, _, dirty, _)| {
                        if *dirty {
                            format!("`{id}`●")
                        } else {
                            format!("`{id}`")
                        }
                    })
                    .collect();
                overview.push_str(&ids.join(" "));
                overview.push_str(" `[expand_hidden]`\n");
                overview
            };
            let changed = if let Some(leaf) = node
                .component_mut()
                .as_any_mut()
                .and_then(|a| a.downcast_mut::<CuiFileLeaf>())
            {
                let changed = leaf.body != new_body;
                leaf.body = new_body;
                changed
            } else {
                false
            };
            if changed {
                node.mark_dirty();
            }
        }
    }

    /// 检查是否已注册元数据组件（`_cui_recent` 存在即视为已启用 reorder 模式）。
    pub fn has_meta_components(&self) -> bool {
        self.roots.iter().any(|n| n.id() == "_cui_recent")
    }

    /// 手动将组件移到指定位置。
    ///
    /// 返回 `true` 如果找到了指定 ID 的组件并成功移动。
    pub fn reorder(&mut self, id: &str, position: OrderPosition) -> bool {
        let idx = self.roots.iter().position(|n| n.id() == id);
        let idx = match idx {
            Some(i) => i,
            None => return false,
        };
        let node = self.roots.remove(idx);
        match position {
            OrderPosition::First => self.roots.insert(0, node),
            OrderPosition::Last => self.roots.push(node),
            OrderPosition::Before(ref target_id) => {
                let target_idx = self.roots.iter().position(|n| n.id() == target_id);
                match target_idx {
                    Some(ti) => self.roots.insert(ti, node),
                    None => self.roots.push(node),
                }
            }
            OrderPosition::After(ref target_id) => {
                let target_idx = self.roots.iter().position(|n| n.id() == target_id);
                match target_idx {
                    Some(ti) => self.roots.insert((ti + 1).min(self.roots.len()), node),
                    None => self.roots.push(node),
                }
            }
            OrderPosition::At(pos) => {
                self.roots.insert(pos.min(self.roots.len()), node);
            }
        }
        true
    }

    pub fn find(&self, id: &str) -> Option<&ComponentNode> {
        self.roots.iter().find_map(|n| n.find(id))
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut ComponentNode> {
        self.roots.iter_mut().find_map(|n| n.find_mut(id))
    }

    /// 设置全局状态（跨组件共享的键值对状态）。
    pub fn set_global_state(&mut self, key: &str, value: &str) {
        self.global_state.insert(key.to_string(), value.to_string());
    }

    /// 读取全局状态。
    pub fn get_global_state(&self, key: &str) -> Option<&str> {
        self.global_state.get(key).map(|s| s.as_str())
    }

    /// 设置指定组件的命名空间状态。
    pub fn set_component_state(&mut self, component_id: &str, key: &str, value: &str) {
        self.component_state
            .entry(component_id.to_string())
            .or_default()
            .insert(key.to_string(), StateEntry::new(value));
    }

    /// 读取指定组件的命名空间状态（自动更新访问时间）。
    pub fn get_component_state(&mut self, component_id: &str, key: &str) -> Option<&str> {
        let entry = self.component_state.get_mut(component_id)?.get_mut(key)?;
        entry.touch();
        Some(entry.value.as_str())
    }

    /// 清理超过 `max_age` 未访问的组件状态条目。
    ///
    /// 返回被清理的条目数。建议在每个 Cycle 结束时调用（如 `compress` 中）。
    pub fn cleanup_stale_component_state(&mut self, max_age: std::time::Duration) -> usize {
        let mut removed = 0;
        self.component_state.retain(|_, entries| {
            let before = entries.len();
            entries.retain(|_, entry| !entry.is_stale(max_age));
            removed += before - entries.len();
            !entries.is_empty()
        });
        removed
    }

    /// `set_global_state` 的便捷别名。注意与 `set_component_state`（组件级命名空间状态）区分。
    pub fn set_state(&mut self, key: &str, value: &str) {
        self.set_global_state(key, value);
    }

    pub fn trigger(&mut self, event: &'static str) {
        self.triggered.insert(event.to_string());
    }

    /// 保存 triggered 和 conditions 快照，用于 render_volatile 恢复。
    pub fn triggered_snapshot(&self) -> HashSet<String> {
        self.triggered.clone()
    }

    /// 恢复 triggered 状态（render_volatile 使用）。
    pub fn restore_triggered(&mut self, saved: HashSet<String>) {
        self.triggered = saved;
    }

    /// 保存 active_conditions 快照，用于 render_volatile 恢复。
    pub fn conditions_snapshot(&self) -> HashSet<String> {
        self.active_conditions.clone()
    }

    /// 恢复 active_conditions 状态（render_volatile 使用）。
    pub fn restore_conditions(&mut self, saved: HashSet<String>) {
        self.active_conditions = saved;
    }

    // ── 条件管理 ───────────────────────────────────────

    /// 添加一条活跃条件。`When(val)` 组件在 `val` 属于此集合时可见。
    pub fn add_condition(&mut self, value: &str) {
        self.active_conditions.insert(value.to_string());
    }

    /// 移除一条活跃条件。
    pub fn remove_condition(&mut self, value: &str) {
        self.active_conditions.remove(value);
    }

    /// 清空所有活跃条件。
    pub fn clear_conditions(&mut self) {
        self.active_conditions.clear();
    }

    /// 替换所有活跃条件（原子替换）。
    pub fn set_conditions(&mut self, values: &[&str]) {
        self.active_conditions.clear();
        for v in values {
            self.active_conditions.insert(v.to_string());
        }
    }

    /// 检查指定条件是否活跃。
    pub fn has_condition(&self, value: &str) -> bool {
        self.active_conditions.contains(value)
    }

    /// 获取活跃条件集合的只读引用。
    pub fn active_conditions(&self) -> &HashSet<String> {
        &self.active_conditions
    }

    pub fn mark_dirty(&mut self, id: &str) {
        if let Some(node) = self.find_mut(id) {
            node.mark_dirty();
        }
    }

    /// 展开/收起概述区（显示所有隐藏组件的详细信息）。
    pub fn set_overview_expanded(&mut self, expanded: bool) {
        self.overview_expanded = expanded;
    }

    /// 概述区当前状态。
    pub fn overview_expanded(&self) -> bool {
        self.overview_expanded
    }

    /// 启用/关闭差量渲染模式。启用后未变化的组件输出 `[unmodified]` 标记。
    pub fn set_delta_mode(&mut self, enabled: bool) {
        self.delta_mode = enabled;
    }

    /// 当前差量渲染模式状态。
    pub fn delta_mode(&self) -> bool {
        self.delta_mode
    }

    /// 当前渲染 tick —— 每次实际渲染完成（commit）后递增。
    /// 虚拟渲染（render_volatile）不推进 tick。
    /// Toast 式临时展开：将组件在接下来 N 个 tick 内强制至少 Summary 级别。
    /// 类似 UI 框架中的 toast 通知 —— 自动出现，到期后自动消失。
    /// `current_tick` 由 Context 提供，用于计算过期时间。
    pub fn set_temp_expand(&mut self, id: &str, ticks: u8, current_tick: u64) {
        let expires_at = current_tick + ticks.max(1) as u64;
        self.temp_expand = Some((id.to_string(), expires_at));
    }

    /// 获取原始 temp_expand 数据：(id, expires_at)。
    /// 供 Context 用于过期清理判断。
    pub fn temp_expand_raw(&self) -> Option<(&str, u64)> {
        self.temp_expand
            .as_ref()
            .map(|(id, expires_at)| (id.as_str(), *expires_at))
    }

    /// 清除 temp_expand（由 Context 在过期时调用）。
    pub fn clear_temp_expand(&mut self) {
        self.temp_expand = None;
    }

    /// 获取当前临时展开信息：(id, remaining_ticks)。
    /// 返回 None 如果无活跃的 temp_expand。
    /// `current_tick` 由 Context 提供。
    pub fn temp_expand_info(&self, current_tick: u64) -> Option<(&str, u64)> {
        self.temp_expand.as_ref().and_then(|(id, expires_at)| {
            let remaining = expires_at.saturating_sub(current_tick);
            if remaining > 0 {
                Some((id.as_str(), remaining))
            } else {
                None
            }
        })
    }

    /// 计算组件的有效渲染级别（叠加 is_static 和 temp_expand 保底）。
    /// `current_tick` 由 Context 提供。
    pub(super) fn effective_level(
        &self,
        node: &ComponentNode,
        assigned: RenderLevel,
        current_tick: u64,
    ) -> RenderLevel {
        let mut l = assigned;
        if node.is_static() && l < RenderLevel::Summary {
            l = RenderLevel::Summary;
        }
        if let Some((ref te_id, expires_at)) = self.temp_expand
            && node.id() == te_id
            && current_tick < expires_at
            && l < RenderLevel::Summary
        {
            l = RenderLevel::Summary;
        }
        l
    }

    /// 综合可见性、低预算模式、容量分配和 effective_level 计算出最终渲染级别。
    fn compute_root_level(
        &self,
        visible: bool,
        low_budget: bool,
        root: &ComponentNode,
        assigned: Option<RenderLevel>,
        current_tick: u64,
    ) -> RenderLevel {
        if !visible {
            return RenderLevel::Hidden;
        }
        if low_budget {
            let base = if root.priority() == PriorityLevel::Critical {
                RenderLevel::Title
            } else {
                RenderLevel::Hidden
            };
            self.effective_level(root, base, current_tick)
        } else {
            self.effective_level(
                root,
                assigned.unwrap_or(RenderLevel::Standard),
                current_tick,
            )
        }
    }

    pub fn write(&mut self, id: &str, mode: DataMode, data: &str) -> bool {
        if let Some(node) = self.find_mut(id) {
            node.write(mode, data);
            true
        } else {
            false
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &ComponentNode> {
        self.roots.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ComponentNode> {
        self.roots.iter_mut()
    }

    /// 深度优先遍历所有节点（根 + Composite 子树）。
    pub fn iter_all(&self) -> AllNodes<'_> {
        AllNodes {
            stack: self.roots.iter().rev().collect(),
        }
    }

    /// 深度优先遍历所有节点（可变引用）。
    pub fn iter_all_mut(&mut self) -> AllNodesMut<'_> {
        AllNodesMut {
            stack: self
                .roots
                .iter_mut()
                .rev()
                .map(|r| r as *mut ComponentNode)
                .collect(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Phase 1: 准备渲染计划。
    ///
    /// 评估可见性条件、执行容量规划（会修改节点级别）、
    /// 消费 recent actions。所有副作用集中在此阶段。
    /// `current_tick` 由 Context 提供，用于 temp_expand 过期判断。
    /// `current_tick` 由 Context 提供，用于 temp_expand 过期判断。
    pub fn prepare(&mut self, budget: usize, current_tick: u64) -> RenderPlan {
        // 预计算可见性
        let condition_checks: Vec<bool> = self
            .roots
            .iter()
            .map(|n| {
                let cond = n.visibility_condition();
                cond.evaluate(&|e| self.triggered.contains(e), &self.active_conditions)
            })
            .collect();

        // recent block — clone to preserve across ticks (cleared every 3)
        let recent_actions = self.recent.clone();
        let recent_rendered = render_recent_actions(&recent_actions);

        let has_meta = self.has_meta_components();
        let overhead = if has_meta {
            0 // meta components are in roots, budget handled by capacity planning
        } else {
            recent_rendered
                .as_ref()
                .map_or(0, |s| crate::tokenizer::estimate(s) + 1)
        };
        let effective = budget.saturating_sub(overhead);

        let mut assignments = Vec::new();
        let mut capacity_total_estimated = 0;

        if effective >= MIN_RENDER_BUDGET {
            // 优先级分层：每个组件按其优先级获得保底级别，不再双重计费预留
            let minimums: Vec<RenderLevel> = self
                .roots
                .iter()
                .map(|r| {
                    if r.is_pinned() {
                        crate::runtime::capacity::PINNED_MINIMUM
                    } else {
                        crate::runtime::capacity::tier_minimum(r.priority())
                    }
                })
                .collect();

            let heatmap: Vec<u8> = self.roots.iter().map(|n| n.heat()).collect();
            let refs: Vec<&dyn CuiComponent> =
                self.roots.iter().map(|n| n.component_ref()).collect();

            let capacity_plan =
                crate::runtime::capacity::plan_tree(&refs, effective, &heatmap, &minimums);
            let total_estimated = capacity_plan.total_estimated;

            // Composite 子节点容量规划（剩余预算按比例分配）
            let mut composites: Vec<(usize, Option<f32>)> = Vec::new();
            for (i, root) in self.roots.iter().enumerate() {
                let assigned = capacity_plan
                    .assignments
                    .get(i)
                    .map(|a| a.level)
                    .unwrap_or(RenderLevel::Standard);
                if self.effective_level(root, assigned, current_tick) == RenderLevel::Hidden {
                    continue;
                }
                if let ComponentNode::Composite { budget_ratio, .. } = root {
                    composites.push((i, *budget_ratio));
                }
            }
            if !composites.is_empty() {
                let remaining = effective.saturating_sub(total_estimated);
                let (with_ratio, without_ratio): (Vec<_>, Vec<_>) =
                    composites.into_iter().partition(|(_, r)| r.is_some());
                let total_ratio: f32 = with_ratio.iter().filter_map(|(_, r)| *r).sum::<f32>().max(0.001);
                let weighted = without_ratio.len();
                let total_weight = weighted as f32 + total_ratio;
                for (i, ratio) in &with_ratio {
                    let share = ((remaining as f32 * ratio.unwrap_or(0.0) / total_weight) as usize).max(1);
                    if let Some(root_mut) = self.roots.get_mut(*i) {
                        plan_composite_children(root_mut, share);
                    }
                }
                for (i, _) in &without_ratio {
                    let share = ((remaining as f32 * 1.0 / total_weight) as usize).max(1);
                    if let Some(root_mut) = self.roots.get_mut(*i) {
                        plan_composite_children(root_mut, share);
                    }
                }
            }

            assignments = capacity_plan.assignments;
            capacity_total_estimated = total_estimated;

            // 冷态可折叠组件钳制：plan_tree 可能因预算宽松将其升级到 Standard，
            // 但折叠态（Summary）是有意"休息"状态，不自动升级。
            // AI 显式展开后 fire() 变热，后续 plan_tree 自然保持展开。
            for (i, root) in self.roots.iter_mut().enumerate() {
                if !root.is_collapsible() || !root.is_collapsed() || root.heat() > 0 {
                    continue;
                }
                let plan_level = assignments.iter().find(|a| a.index == i).map(|a| a.level);
                let clamped_level = plan_level.unwrap_or(root.level());
                if clamped_level > RenderLevel::Summary {
                    root.set_level(RenderLevel::Summary);
                    if let Some(assignment) = assignments.iter_mut().find(|a| a.index == i) {
                        assignment.level = RenderLevel::Summary;
                    }
                }
            }
        }

        // temp_expand 提升（递归到所有节点，确保非根组件也可受益）
        if let Some((ref te_id, _)) = self.temp_expand {
            for root in self.roots.iter_mut() {
                apply_temp_expand_to_tree(root, te_id);
            }
        }

        // 对整棵树递归应用可见性条件（VisibilityCondition 优先级高于容量规划和 temp_expand）
        for root in self.roots.iter_mut() {
            apply_visibility_to_tree(root, &self.triggered, &self.active_conditions);
        }

        // 收集隐藏节点信息
        let low_budget = effective < MIN_RENDER_BUDGET;
        let mut hidden_info: Vec<(String, bool, bool, bool)> = Vec::new();
        for (i, root) in self.roots.iter().enumerate() {
            let visible = *condition_checks.get(i).unwrap_or(&true);
            let assigned = assignments.get(i).map(|a| a.level);
            let level = self.compute_root_level(visible, low_budget, root, assigned, current_tick);

            if level == RenderLevel::Hidden {
                let is_composite = matches!(root, ComponentNode::Composite { .. });
                hidden_info.push((
                    root.id().to_string(),
                    root.is_inert(),
                    root.is_dirty(),
                    is_composite,
                ));
            }
        }

        let expanded = self.overview_expanded;

        // 同步元数据组件数据（在 reorder 模式开启时）
        if has_meta {
            self.sync_meta_components(recent_rendered.as_deref(), &hidden_info);
        }

        RenderPlan {
            condition_checks,
            assignments,
            recent_rendered,
            hidden_info,
            overview_expanded: expanded,
            effective_budget: effective,
            total_estimated: capacity_total_estimated,
        }
    }

    /// Phase 2: 纯渲染 —— 从 RenderPlan 生成输出字符串。
    ///
    /// 不修改任何内部状态，可安全地多次调用。
    /// 输出顺序：recent → 可见内容 → 隐藏组件概述。
    /// `ordering_override` 可覆盖树上的默认排序策略，实现单次渲染的动态重排。
    /// `current_tick` 由 Context 提供，用于概述区的 temp_expand 剩余 tick 显示。
    pub fn render_plan(
        &mut self,
        plan: &RenderPlan,
        ordering_override: Option<OrderingStrategy>,
        current_tick: u64,
    ) -> String {
        let mut output = String::new();

        let has_meta = self.has_meta_components();

        if !has_meta {
            if let Some(ref block) = plan.recent_rendered {
                output.push_str(block);
                output.push('\n');
            }
        }

        // 低预算模式：Critical 至少 Title，其余进入 overview
        let low_budget = plan.effective_budget < MIN_RENDER_BUDGET;

        // 按排序策略计算渲染顺序（支持单次覆盖）
        // reorder 模式下直接按物理顺序，不再虚拟排序
        let strategy = ordering_override.unwrap_or(self.ordering);
        let roots_ref: Vec<&ComponentNode> = self.roots.iter().collect();
        let sorted_indices = if has_meta {
            // 物理已排序，直接按 root 顺序渲染
            (0..roots_ref.len()).collect()
        } else {
            ordering::sort_indices(&roots_ref, strategy)
        };

        for &orig_idx in &sorted_indices {
            let root = &self.roots[orig_idx];
            let assigned = plan.assignments.get(orig_idx).map(|a| a.level);
            let visible = *plan.condition_checks.get(orig_idx).unwrap_or(&true);
            let level = self.compute_root_level(visible, low_budget, root, assigned, current_tick);

            if level != RenderLevel::Hidden {
                output.push_str(&root.render_recursive(level, self.delta_mode));
            }
        }

        // 概述区（可见内容之后）
        if !has_meta && !plan.hidden_info.is_empty() {
            output.push_str("## [_overview]\n  ");
            let ids: Vec<String> = plan
                .hidden_info
                .iter()
                .map(|(id, _, dirty, _)| {
                    if *dirty {
                        format!("`{id}`●")
                    } else {
                        format!("`{id}`")
                    }
                })
                .collect();
            output.push_str(&ids.join(" "));
            output.push_str(" `[expand_hidden]`\n");
        }

        output
    }

    /// 提交渲染副作用 —— 清理 dirty 标记、触发事件和临时状态。
    ///
    /// 在 `render_plan()` 之后调用。tick 推进和 temp_expand 清理由 Context 管理。
    pub fn commit(&mut self) {
        for root in self.roots.iter_mut() {
            cool_node_signal(root);
        }
        self.overview_expanded = false;
        self.triggered.clear();

        self.recent_ticks += 1;
        if self.recent_ticks >= 3 {
            self.recent.clear();
            self.recent_ticks = 0;
        }
    }

    /// 添加一条操作记录到 recent（在 [_recent] 中展示约 3 个 tick）。
    pub fn add_recent(&mut self, title: &str, action: &str, success: bool) {
        self.recent.push(ActionRecord {
            component_title: title.to_string(),
            action: action.to_string(),
            target_level: None,
            success,
        });
    }

    /// 便捷渲染（三步合一）：prepare → render_plan → commit。
    ///
    /// 不含渲染状态机保护 —— 状态机由 Context 管理。
    /// `current_tick` 由调用者提供，tick 推进由 Context 负责。
    pub fn render(
        &mut self,
        budget: usize,
        ordering_override: Option<OrderingStrategy>,
        current_tick: u64,
    ) -> String {
        let (output, _) = self.render_with_stats(budget, ordering_override, current_tick);
        output
    }

    /// 渲染并返回统计信息。
    /// `current_tick` 由调用者提供。
    pub fn render_with_stats(
        &mut self,
        budget: usize,
        ordering_override: Option<OrderingStrategy>,
        current_tick: u64,
    ) -> (String, schedule::RenderStats) {
        let plan = self.prepare(budget, current_tick);
        let stats = plan.stats();
        let output = self.render_plan(&plan, ordering_override, current_tick);
        self.commit();
        (output, stats)
    }

    /// 生成组件树快照 —— 用于调试和测试的序列化格式。
    pub fn snapshot(&self) -> TreeSnapshot {
        fn node_snapshot(node: &ComponentNode) -> NodeSnapshot {
            let (kind, children) = match node {
                ComponentNode::Leaf(_) => ("leaf".to_string(), Vec::new()),
                ComponentNode::Composite { children, .. } => (
                    "composite".to_string(),
                    children.iter().map(node_snapshot).collect(),
                ),
            };
            NodeSnapshot {
                id: node.id().to_string(),
                title: node.title().to_string(),
                level: node.level().as_str().to_string(),
                kind,
                priority: node.priority().as_str().to_string(),
                is_static: node.is_static(),
                is_inert: node.is_inert(),
                is_dirty: node.is_dirty(),
                children,
            }
        }

        let roots: Vec<NodeSnapshot> = self.roots.iter().map(node_snapshot).collect();

        let mut total_nodes = 0;
        let mut leaf_nodes = 0;
        let mut composite_nodes = 0;
        let mut hidden_nodes = 0;
        let mut dirty_nodes = 0;
        for n in self.iter_all() {
            total_nodes += 1;
            match n {
                ComponentNode::Leaf(_) => leaf_nodes += 1,
                ComponentNode::Composite { .. } => composite_nodes += 1,
            }
            if n.level() == RenderLevel::Hidden {
                hidden_nodes += 1;
            }
            if n.is_dirty() {
                dirty_nodes += 1;
            }
        }

        TreeSnapshot {
            roots,
            stats: TreeStats {
                total_nodes,
                leaf_nodes,
                composite_nodes,
                hidden_nodes,
                dirty_nodes,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::builtin::TextBlock;
    use crate::PriorityLevel;

    #[test]
    fn reorder_roots_creates_meta_components() {
        let mut tree = ComponentTree::new();
        assert!(!tree.has_meta_components());

        tree.reorder_roots();

        assert!(tree.has_meta_components());
        assert!(tree.roots.iter().any(|n| n.id() == "_cui_recent"));
        assert!(tree.roots.iter().any(|n| n.id() == "_cui_overview"));
    }

    #[test]
    fn reorder_roots_idempotent() {
        let mut tree = ComponentTree::new();
        tree.reorder_roots();
        let count = tree.len();
        tree.reorder_roots();
        assert_eq!(tree.len(), count);
    }

    #[test]
    fn reorder_roots_sorts_stable_before_volatile() {
        let mut tree = ComponentTree::new();

        let mut volatile = TextBlock::new("volatile", "V", "vol")
            .priority(PriorityLevel::Normal)
            .build();
        volatile.set_volatility(200);

        let stable = TextBlock::new("stable", "S", "sta")
            .priority(PriorityLevel::Normal)
            .build();
        // volatility = 0 by default

        tree.push(stable);
        tree.push(volatile);

        tree.reorder_roots();

        // After reorder: meta + stable first (low vol), volatile last (high vol)
        let ids: Vec<String> = tree.roots.iter().map(|n| n.id().to_string()).collect();
        let stable_pos = ids.iter().position(|id| id == "stable").unwrap();
        let volatile_pos = ids.iter().position(|id| id == "volatile").unwrap();
        assert!(stable_pos < volatile_pos,
            "expected stable before volatile, got {ids:?}");
    }

    #[test]
    fn reorder_roots_puts_volatile_last() {
        let mut tree = ComponentTree::new();
        let s = TextBlock::new("s", "S", "").build();
        tree.push(s);

        let mut volatile = TextBlock::new("v", "V", "").build();
        volatile.set_volatility(200);
        tree.push(volatile);

        tree.reorder_roots();

        // volatile (vol=200) should be last; meta components (vol=0, inert) before it
        let ids: Vec<String> = tree.roots.iter().map(|n| n.id().to_string()).collect();
        let v_pos = ids.iter().position(|id| id == "v").unwrap();
        assert_eq!(v_pos, ids.len() - 1,
            "volatile should be last, got order: {ids:?}");
    }

    #[test]
    fn render_without_reorder_preserves_hardcoded_blocks() {
        let mut tree = ComponentTree::new();
        let a = TextBlock::new("a", "A", "body a").build();
        tree.push(a);
        tree.add_recent("A", "expand", true);

        let output = tree.render(4096, None, 0);
        assert!(output.contains("## [_recent]"), "expected hardcoded recent block: {output}");
    }

    #[test]
    fn render_after_reorder_uses_meta_components() {
        let mut tree = ComponentTree::new();
        let a = TextBlock::new("a", "A", "body a").build();
        tree.push(a);
        tree.add_recent("A", "expand", true);

        tree.reorder_roots();
        let output = tree.render(4096, None, 0);

        // _cui_recent is Inline, its body is the recent content
        assert!(output.contains("## [_recent]"), "expected recent from meta component: {output}");
        // not double: only one occurrence
        let count = output.matches("## [_recent]").count();
        assert_eq!(count, 1, "expected single _recent, got {count}: {output}");
    }

    #[test]
    fn render_no_duplicate_recent_after_reorder() {
        let mut tree = ComponentTree::new();
        let a = TextBlock::new("a", "A", "body a").build();
        tree.push(a);
        tree.add_recent("A", "refresh", true);
        tree.add_recent("B", "close", false);

        tree.reorder_roots();
        let output = tree.render(4096, None, 0);

        let count = output.matches("## [_recent]").count();
        assert_eq!(count, 1, "recent block should appear exactly once: {output}");
    }

    #[test]
    fn reorder_then_render_keeps_body_content() {
        let mut tree = ComponentTree::new();
        let a = TextBlock::new("hello", "Hello", "world content").build();
        tree.push(a);
        tree.reorder_roots();

        let output = tree.render(4096, None, 0);
        assert!(output.contains("## [Hello]"), "expected header: {output}");
        assert!(output.contains("world content"), "expected body: {output}");
    }
}

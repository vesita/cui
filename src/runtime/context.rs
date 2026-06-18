//! 运行时上下文管理器 —— CUI 框架的统一入口。
//!
//! 基于 [`ComponentTree`] 提供组件注册、条件管理、生命周期事件、
//! 容量规划渲染、对话操作和模板解析。所有数据和状态由此单一管理器持有。

use std::sync::{Arc, Mutex};

use crate::RenderLevel;
use crate::action::{ActionRequest, ActionResult, DialogueOps};
use crate::compile::template::{ReadMode, TemplateResolver};
use crate::component::{ComponentNode, ComponentTree};

use crate::data::DataMode;
use crate::manage::ManageEvent;
use crate::runtime::cycle::RenderCycle;
use crate::runtime::schedule::RenderStats;
use crate::runtime::dialogue::DialogueManager;
use crate::runtime::event::{ComponentEvent, EventBus, SimpleEventBus};
use crate::runtime::handler::{ActionContext, ActionHandler, ActionHandlerRef, HandlerRegistry};
use crate::runtime::ordering::OrderingStrategy;

// ── ComponentStore ──────────────────────────────────────────

/// 组件存储 trait —— 管理组件生命周期和读写。
///
/// 后端代码应依赖此 trait 而非 `&mut Context`，以便 mock 测试。
/// `Context` 实现了此 trait。
pub trait ComponentStore {
    /// 注册一个组件节点。
    fn register(&mut self, node: ComponentNode);
    /// 按 ID 移除组件，返回被移除的节点。
    fn remove_node(&mut self, id: &str) -> Option<ComponentNode>;
    /// 向组件写入数据。
    fn write_data(&mut self, id: &str, mode: DataMode, data: &str) -> bool;
    /// 读取组件正文（Detailed 级别），不存在返回空。
    fn read_data(&self, id: &str) -> String;
    /// 查找组件节点只读引用。
    fn find_node(&self, id: &str) -> Option<&ComponentNode>;
    /// 获取内部 ComponentTree 只读引用。
    fn tree_ref(&self) -> &ComponentTree;
    /// 获取内部 ComponentTree 可变引用。
    fn tree_mut_ref(&mut self) -> &mut ComponentTree;
}

// ── Renderer ─────────────────────────────────────────────────

/// 渲染 trait —— 将渲染职责与 Context 的其他功能分离。
///
/// 后端代码可依赖此 trait 而非 `&mut Context`，以便 mock 测试。
/// `Context` 实现了此 trait。
pub trait Renderer {
    /// 渲染所有组件（默认 token budget）。
    fn render(&mut self) -> String;
    /// 获取最近一次渲染的统计信息。
    fn last_render_stats(&self) -> Option<&RenderStats>;
}

// ── ActionDispatcher ─────────────────────────────────────────

/// 动作分发 trait —— 将 CUI 动作处理与 Context 的其他功能分离。
///
/// 后端代码可依赖此 trait 而非 `&mut Context`，以便 mock 测试。
/// `Context` 实现了此 trait。
pub trait ActionDispatcher {
    /// 派发 CUI 动作到指定组件。
    fn component_action(&mut self, request: &ActionRequest) -> ActionResult;
}

/// 默认渲染预算（token 数）。
pub const DEFAULT_RENDER_BUDGET: usize = 262144;

/// 上下文管理器 —— CUI 框架的统一入口。
///
/// 持有 [`ComponentTree`] 作为组件数据模型，
/// 同时管理对话消息缓冲、渲染状态机和全局状态。
pub struct Context {
    tree: ComponentTree,
    dialogue: DialogueManager,
    event_bus: SimpleEventBus,
    /// 命名处理器注册表，用于解析 `ActionHandlerRef::Named("tool.bash")`。
    handler_registry: HandlerRegistry,

    /// 后端注入的扩展资源（类型擦除），handler 通过 `resource::<T>()` 下行获取。
    extension: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// 最近一次渲染的统计信息（容量规划反馈）。
    last_render_stats: Option<RenderStats>,
    /// 渲染 tick 计数器 —— 每次实际渲染推进，虚拟渲染不推进。
    tick: u64,
    /// 渲染周期状态机 —— 确保 prepare/render_plan/commit 的正确顺序。
    cycle: RenderCycle,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        let tree = ComponentTree::new();
        Self {
            tree,
            dialogue: DialogueManager::new(),
            event_bus: SimpleEventBus::new(),
            handler_registry: HandlerRegistry::new(),
            extension: None,
            last_render_stats: None,
            tick: 0,
            cycle: RenderCycle::Idle(crate::runtime::cycle::Idle {}),
        }
    }

    // ── 组件注册 ───────────────────────────────────────

    /// 注册一个组件节点（自动发射 `{id}.registered` 事件）。
    pub fn register(&mut self, node: ComponentNode) {
        let id = node.id().to_string();
        self.tree.push(node);
        self.emit(&id, "registered", &format!(r#"{{"id":"{id}"}}"#));
    }

    /// 注册多个组件节点。
    pub fn register_all(&mut self, nodes: impl IntoIterator<Item = ComponentNode>) {
        for node in nodes {
            self.register(node);
        }
    }

    /// 按 ID 移除组件（自动发射 `{id}.removed` 事件）。
    pub fn remove(&mut self, id: &str) -> Option<ComponentNode> {
        let removed = self.tree.remove(id);
        if removed.is_some() {
            self.emit(id, "removed", &format!(r#"{{"id":"{id}"}}"#));
        }
        removed
    }

    /// 注册对话 `ComponentNode`，同时提供 `DialogueOps` 访问。
    /// 自动发射 `{id}.registered` 事件。
    pub fn register_dialogue_node(
        &mut self,
        node: ComponentNode,
        ops: Arc<Mutex<Box<dyn DialogueOps + Send>>>,
    ) {
        let id = node.id().to_string();
        self.dialogue.set_shared(ops);
        self.tree.push(node);
        self.emit(&id, "registered", &format!(r#"{{"id":"{id}"}}"#));
    }

    /// 清空所有组件和对话消息。
    pub fn clear(&mut self) {
        self.tree.clear();
        self.dialogue.clear();
    }

    // ── 渲染 ───────────────────────────────────────────

    /// 以默认预算渲染所有组件。
    pub fn render(&mut self) -> String {
        self.render_impl(DEFAULT_RENDER_BUDGET)
    }

    /// 重排根组件顺序以优化 LLM prompt 缓存命中率。
    ///
    /// 渲染后调用：按 volatility、dirty_count 等信号物理排序 `roots`，
    /// 稳定组件前移形成缓存前缀。同时注册 `_cui_recent` / `_cui_overview`
    /// 元数据组件，后续渲染由组件树自动管理。
    ///
    /// 返回 `&mut Self` 支持链式调用：
    /// ```ignore
    /// ctx.render();
    /// ctx.reorder().render();
    /// ```
    pub fn reorder(&mut self) -> &mut Self {
        self.tree.reorder_roots();
        self
    }

    /// 虚拟渲染（不推进 tick，不清理状态），默认预算。
    pub fn render_volatile(&mut self) -> String {
        self.render_volatile_impl(DEFAULT_RENDER_BUDGET)
    }

    /// 指定 token 预算。
    ///
    /// ```ignore
    /// ctx.with_budget(50000).render();
    /// ctx.with_budget(50000).render_volatile();
    /// ```
    pub fn with_budget(&mut self, budget: usize) -> BudgetRender<'_> {
        BudgetRender {
            ctx: self,
            budget: budget.max(1),
        }
    }

    /// 在指定条件下渲染。渲染完成后条件自动清除，不持久化。
    pub fn in_condition(&mut self, condition: &str) -> ConditionRender<'_> {
        ConditionRender {
            ctx: self,
            conditions: vec![condition.to_string()],
            budget: DEFAULT_RENDER_BUDGET,
        }
    }

    /// 内部实现：按指定预算执行完整渲染管线。
    ///
    /// prepare → render_plan → commit，推进 tick，清理过期 temp_expand。
    pub(crate) fn render_impl(&mut self, budget: usize) -> String {
        use crate::runtime::cycle::RenderCycleMessages;
        debug_assert!(
            self.cycle
                .can_handle(&RenderCycleMessages::Prepare(crate::runtime::cycle::Prepare)),
            "render() 只能在 Idle 状态调用"
        );
        self.cycle.on_prepare(crate::runtime::cycle::Prepare);

        let plan = self.tree.prepare(budget, self.tick);

        debug_assert!(
            self.cycle.can_handle(&RenderCycleMessages::DoRenderPlan(
                crate::runtime::cycle::DoRenderPlan
            )),
            "render_plan() 只能在 Preparing 状态调用"
        );
        self.cycle
            .on_do_render_plan(crate::runtime::cycle::DoRenderPlan);

        let output = self.tree.render_plan(&plan, None, self.tick);
        let stats = plan.stats();
        self.last_render_stats = Some(stats);

        debug_assert!(
            self.cycle.can_handle(&RenderCycleMessages::CommitMsg(
                crate::runtime::cycle::CommitMsg
            )),
            "commit() 只能在 Rendering 状态调用"
        );
        self.cycle.on_commit_msg(crate::runtime::cycle::CommitMsg);

        self.tree.commit();
        self.tick += 1;

        // 清理已过期的 temp_expand
        if let Some((_, expires_at)) = self.tree.temp_expand_raw()
            && self.tick >= expires_at
        {
            self.tree.clear_temp_expand();
        }

        output
    }

    /// 内部实现：虚拟渲染（不推进 tick，不清理信号标记和触发事件）。
    ///
    /// **会消费** recent actions，同一周期内重复调用会得到不同结果。
    pub(crate) fn render_volatile_impl(&mut self, budget: usize) -> String {
        use crate::runtime::cycle::RenderCycleMessages;
        // 保存状态
        let saved_triggered = self.tree.triggered_snapshot();
        let saved_conditions = self.tree.conditions_snapshot();
        let saved_overview = self.tree.overview_expanded();
        let saved_levels: Vec<(String, RenderLevel)> = self
            .tree
            .iter_all()
            .map(|n| (n.id().to_string(), n.level()))
            .collect();

        // 保存 meta 组件的 body 和信号，prepare() 中的 sync_meta_components()
        // 会修改这些数据，虚拟渲染后需恢复，避免污染下次真实渲染。
        let saved_meta: Vec<(String, String, bool)> = {
            use crate::component::builtin::CuiFileLeaf;
            ["_cui_recent", "_cui_overview"]
                .iter()
                .filter_map(|mid| {
                    let node = self.tree.find_mut(mid)?;
                    let body = node
                        .component_mut()
                        .as_any_mut()
                        .and_then(|a| a.downcast_mut::<CuiFileLeaf>())
                        .map(|leaf| leaf.body.clone())?;
                    let was_dirty = node.info().signal.interactive || node.info().signal.data_freshness > 0;
                    Some((mid.to_string(), body, was_dirty))
                })
                .collect()
        };

        debug_assert!(
            self.cycle
                .can_handle(&RenderCycleMessages::Prepare(crate::runtime::cycle::Prepare)),
            "render_volatile() 只能在 Idle 状态调用"
        );
        self.cycle.on_prepare(crate::runtime::cycle::Prepare);

        let plan = self.tree.prepare(budget, self.tick);
        let output = self.tree.render_plan(&plan, None, self.tick);

        // 中止周期，不推进 tick
        if self
            .cycle
            .can_handle(&RenderCycleMessages::Abort(crate::runtime::cycle::Abort))
        {
            self.cycle.on_abort(crate::runtime::cycle::Abort);
        }

        // 恢复状态
        self.tree.restore_triggered(saved_triggered);
        self.tree.restore_conditions(saved_conditions);
        self.tree.set_overview_expanded(saved_overview);
        for (id, level) in saved_levels {
            if let Some(node) = self.tree.find_mut(&id) {
                node.set_level(level);
            }
        }

        // 恢复 meta 组件的 body 和信号
        for (mid, saved_body, was_dirty) in &saved_meta {
            if let Some(node) = self.tree.find_mut(mid) {
                use crate::component::builtin::CuiFileLeaf;
                if let Some(leaf) = node
                    .component_mut()
                    .as_any_mut()
                    .and_then(|a| a.downcast_mut::<CuiFileLeaf>())
                {
                    leaf.body = saved_body.clone();
                }
                if *was_dirty {
                    node.info_mut().signal.interactive = true;
                    node.info_mut().signal.data_freshness = 255;
                } else {
                    node.info_mut().signal.interactive = false;
                    node.info_mut().signal.data_freshness = 0;
                }
            }
        }

        output
    }

    /// 设置组件树的排序策略（动态重排）。
    ///
    /// 通过此方法可在不同渲染阶段使用不同策略，例如在 LLM 提示生成前
    /// 设置为 `CacheOptimized` 以提升缓存命中率。
    ///
    /// 默认为 `ByPriority`（按注册顺序）。
    pub fn set_ordering(&mut self, strategy: OrderingStrategy) {
        self.tree.set_ordering(strategy);
    }

    // ── 生命周期 ───────────────────────────────────────

    /// 派发管理事件到所有根组件。
    pub fn on_event(&mut self, event: ManageEvent) {
        for node in self.tree.iter_mut() {
            node.on_event(event);
        }
    }

    /// 触发外部事件。
    pub fn trigger(&mut self, event: &'static str) {
        self.tree.trigger(event);
        self.on_event(ManageEvent::External(event));
    }

    /// 通知所有组件新 Cycle 开始。
    pub fn start_new_cycle(&mut self, cycle_id: u32) {
        for node in self.tree.iter_mut() {
            node.start_new_cycle(cycle_id);
        }
    }

    /// 对所有组件执行压缩操作，并清理超过 300 秒未访问的组件状态。
    pub fn compress(&mut self) -> bool {
        let mut any = false;
        for node in self.tree.iter_mut() {
            if node.compress() {
                any = true;
            }
        }
        self.tree
            .cleanup_stale_component_state(std::time::Duration::from_secs(300));
        any
    }

    /// 收集所有可持久化组件的渲染内容。

    // ── 数据写入 ───────────────────────────────────────

    /// 向指定组件写入数据（自动发射 `{id}.data_changed` 事件）。
    pub fn write(&mut self, id: &str, mode: DataMode, data: &str) -> bool {
        let ok = self.tree.write(id, mode, data);
        if ok {
            self.emit(id, "data_changed", data);
        }
        ok
    }

    // ── 数据读取 ───────────────────────────────────────

    /// 按组件 ID 读取正文（Detailed 级别）。
    pub fn read(&self, id: &str) -> String {
        self.tree
            .find(id)
            .map(|node| node.render_node(RenderLevel::Detailed))
            .unwrap_or_default()
    }

    /// 按 ID 前缀批量读取组件正文（递归搜索所有节点）。
    pub fn read_by_label_prefix(&self, prefix: &str) -> String {
        let mut out = String::new();
        for node in self.tree.iter() {
            Self::collect_by_prefix(node, prefix, &mut out);
        }
        out
    }

    fn collect_by_prefix(node: &ComponentNode, prefix: &str, out: &mut String) {
        if node.id().starts_with(prefix) {
            let body = node.render_body_only(RenderLevel::Detailed);
            if !body.trim().is_empty() {
                out.push_str(&format!("## {}\n{}\n", node.title(), body));
            }
        }
        if let ComponentNode::Composite { children, .. } = node {
            for child in children {
                Self::collect_by_prefix(child, prefix, out);
            }
        }
    }

    /// 获取对话操作句柄。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// ctx.dialogue_mut().push(r#"{"role":"user","content":"hi"}"#);
    /// let msgs: Vec<String> = ctx.dialogue_mut().read();
    /// ctx.dialogue_mut().scroll(5);
    /// ctx.dialogue_mut().expand_cold(0, 3);
    /// ```
    pub fn dialogue_mut(&mut self) -> DialogueHandle<'_> {
        DialogueHandle { ctx: self }
    }

    pub fn dialogue(&self) -> DialogueRef<'_> {
        DialogueRef { ctx: self }
    }

    pub fn persistence(&self) -> PersistHandle<'_> {
        PersistHandle { ctx: self }
    }

    // ── 动作 ───────────────────────────────────────────

    /// 派发 CUI 动作到指定组件。
    ///
    /// 执行流程：
    /// 1. 匹配 ActionVariant/ActionDef
    /// 2. 如果 variant 绑定了 handler：解析 handler ref，调用 handler.execute()
    /// 3. 如果 handler 返回 ActionOutput，处理 events 和 new_level
    /// 4. 如果无 handler：回退到展示动作（改变渲染级别）
    /// 5. 成功后发射 `{id}.action_executed` 事件
    pub fn component_action(&mut self, request: &ActionRequest) -> ActionResult {
        let id = &request.component_id;
        let action = &request.action;

        // 特殊处理：概述区动作
        if id == "_overview" {
            return self.handle_overview_action(action);
        }

        // 合并预设参数（def.params）和请求参数（request.params）
        let mut params = String::new();
        let mut def_found = false;
        if let Some(node) = self.tree.find(id) {
            let level = node.level();
            let actions = node.actions(level);
            if let Some(def) = actions.iter().find(|a| a.id() == *action) {
                def_found = true;
                params = merge_action_params(def.params(), request.params.as_deref());
                if let Some(handler_ref) = def.handler() {
                    if let Some(handler) = self.resolve_handler_ref(handler_ref) {
                        let merged_params = params.clone();
                        let merged_request = ActionRequest {
                            params: Some(merged_params),
                            ..request.clone()
                        };
                        match handler.execute_request(&merged_request, self as &mut dyn ActionContext) {
                            Ok(output) => {
                                // 处理 ActionOutput.events
                                for (event_name, event_data) in &output.events {
                                    if let Some((source, kind)) = event_name.split_once('.') {
                                        self.emit(source, kind, event_data);
                                    } else {
                                        self.emit("*", event_name, event_data);
                                    }
                                }
                                // 处理 new_level + 数据写入 + 渲染快照，单次 find_mut 合并操作
                                let applied_level = output.new_level.or(def.target_level());
                                let (snapshot, title) = {
                                    if let Some(node_mut) = self.tree.find_mut(id) {
                                        if let Some(new_level) = applied_level {
                                            node_mut.set_level(new_level);
                                        }
                                        if let Some(data) = &output.data {
                                            node_mut.write(
                                                crate::data::DataMode::Overwrite,
                                                data,
                                            );
                                        }
                                        node_mut.mark_dirty();
                                        let snap = node_mut.render_node(node_mut.level());
                                        let t = node_mut.title().to_string();
                                        (snap, t)
                                    } else {
                                        return ActionResult::error(
                                            id,
                                            action,
                                            "组件已消失",
                                        );
                                    }
                                };
                                self.emit(
                                    id,
                                    "action_executed",
                                    &format!(
                                        r#"{{"action":"{action}","success":{}}}"#,
                                        output.success
                                    ),
                                );
                                self.tree.add_recent(&title, action, true);
                                let mut ar =
                                    ActionResult::new(id.clone(), action.to_string())
                                        .with_snapshot(snapshot);
                                if let Some(lvl) = applied_level {
                                    ar = ar.with_new_level(lvl);
                                }
                                return ar;
                            }
                            Err(e) => {
                                self.emit(
                                    id,
                                    "action_executed",
                                    &format!(r#"{{"action":"{action}","success":false}}"#),
                                );
                                return ActionResult::error(id, action, e.to_string());
                            }
                        }
                    } else {
                        return ActionResult::error(
                            id,
                            action,
                            format!("处理器未注册: {:?}", handler_ref),
                        );
                    }
                }
            }
        }

        // 回退：展示动作（改变渲染级别）
        let fallback_params = if def_found {
            params.as_str()
        } else {
            request.params.as_deref().unwrap_or("")
        };
        let (result, title) = self
            .tree
            .find_mut(id)
            .map(|node| {
                let r = node.handle_action(action, fallback_params);
                if let Some(new_level) = r.new_level() {
                    node.set_level(new_level);
                }
                let t = node.title().to_string();
                (r, t)
            })
            .unwrap_or_else(|| {
                (
                    ActionResult::error(id, action, format!("组件 '{id}' 未找到")),
                    id.to_string(),
                )
            });

        // "show" 动作自动触发 temp_expand，实现 Toast 式自动消失
        if result.is_success() && action == "show" {
            self.tree.set_temp_expand(id, 3, self.tick);
        }

        // 记录操作到 recent，帮助 AI 记住历史操作
        if result.is_success() {
            self.tree.add_recent(&title, action, true);
        }

        // 发射生命周期事件
        self.emit(
            id,
            "action_executed",
            &format!(
                r#"{{"action":"{action}","success":{}}}"#,
                result.is_success()
            ),
        );

        result
    }

    /// 发布事件到事件总线。
    pub fn emit(&mut self, source: &str, kind: &str, data: &str) {
        self.event_bus.emit(ComponentEvent::new(source, kind, data));
    }

    /// 订阅事件。
    ///
    /// `pattern` 支持通配符：
    /// - `"dialogue.data_changed"` — 精确匹配
    /// - `"dialogue.*"` — 匹配该组件所有事件
    /// - `"*.data_changed"` — 匹配所有组件的 data_changed
    /// - `"*"` — 匹配所有事件
    pub fn on(&mut self, pattern: &str, handler: Box<dyn Fn(&ComponentEvent) + Send>) {
        self.event_bus.on(pattern, handler);
    }

    /// 处理概述区动作。
    fn handle_overview_action(&mut self, action: &str) -> ActionResult {
        if action == "expand_hidden" {
            self.tree.set_overview_expanded(true);
            return ActionResult::new("_overview", action.to_string())
                .with_message("已展开所有隐藏组件（单次有效，下轮自动折叠）");
        }
        if let Some(target_id) = action.strip_prefix("temp_expand:") {
            if target_id.is_empty() {
                return ActionResult::error("_overview", action, "temp_expand 缺少目标组件 ID");
            }
            self.tree.set_temp_expand(target_id, 3, self.tick);
            let inner = ActionRequest {
                component_id: target_id.to_string(),
                action: "expand".to_string(),
                params: None,
            };
            return self.component_action(&inner);
        }
        if let Some(target_id) = action.strip_prefix("expand_group:") {
            if target_id.is_empty() {
                return ActionResult::error("_overview", action, "expand_group 缺少目标组件 ID");
            }
            let inner = ActionRequest {
                component_id: target_id.to_string(),
                action: "expand_group".to_string(),
                params: None,
            };
            return self.component_action(&inner);
        }
        if let Some(target_id) = action.strip_prefix("expand:") {
            if target_id.is_empty() {
                return ActionResult::error("_overview", action, "expand 缺少目标组件 ID");
            }
            let inner = ActionRequest {
                component_id: target_id.to_string(),
                action: "expand".to_string(),
                params: None,
            };
            return self.component_action(&inner);
        }
        ActionResult::error("_overview", action, format!("未知概述区动作: {action}"))
    }

    // ── Handler 注册表 ──────────────────────────────────

    /// 注册命名处理器，用于解析 `.cui` 文件中的 `handler: tool.bash`。
    pub fn register_handler(&mut self, name: impl Into<String>, handler: Arc<dyn ActionHandler>) {
        self.handler_registry.register(name, handler);
    }

    /// 批量注册处理器。
    pub fn register_handlers(&mut self, registry: &HandlerRegistry) {
        for (name, handler) in registry.iter() {
            self.handler_registry.register(name, handler.clone());
        }
    }

    /// 获取 HandlerRegistry 的可变引用，用于批量操作。
    pub fn handler_registry_mut(&mut self) -> &mut HandlerRegistry {
        &mut self.handler_registry
    }

    /// 获取 HandlerRegistry 的只读引用。
    pub fn handler_registry(&self) -> &HandlerRegistry {
        &self.handler_registry
    }

    /// 按名称查找已注册的处理器。
    pub fn resolve_handler(&self, name: &str) -> Option<Arc<dyn ActionHandler>> {
        self.handler_registry.resolve(name)
    }

    /// 解析 `ActionHandlerRef`：`Inline` 直接返回，`Named` 查注册表。
    fn resolve_handler_ref(&self, r: &ActionHandlerRef) -> Option<Arc<dyn ActionHandler>> {
        self.handler_registry.resolve_ref(r)
    }

    // ── 扩展资源注入 ──────────────────────────────────

    /// 注入扩展资源（handler 通过 `ActionContext::resource::<T>()` 获取）。
    pub fn set_extension<T: 'static + Send + Sync>(&mut self, ext: T) {
        self.extension = Some(Arc::new(ext));
    }

    /// 清除已注入的扩展资源。
    pub fn clear_extension(&mut self) {
        self.extension = None;
    }

    // ── 查询 ───────────────────────────────────────────

    /// 获取最近一次渲染的统计信息（预算使用反馈）。
    ///
    /// 返回 `None` 如果尚未渲染过。每次 `render()` 后更新。
    pub fn last_render_stats(&self) -> Option<&RenderStats> {
        self.last_render_stats.as_ref()
    }

    /// 获取当前渲染 tick。
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// 获取内部 ComponentTree 的只读引用。
    pub fn tree(&self) -> &ComponentTree {
        &self.tree
    }

    /// 获取内部 ComponentTree 的可变引用。
    ///
    /// **注意**：直接操作 `ComponentTree` 会绕过 Context 层面的安全防护（渲染状态机、
    /// 生命周期钩子、重复 ID 断言、recent action 追踪）。在绝大多数场景下应优先使用
    /// `register`、`remove`、`write` 等高层 API。此方法仅作为底层转义口，用于测试或
    /// 非常规操作。
    pub fn tree_mut(&mut self) -> &mut ComponentTree {
        &mut self.tree
    }

    /// 显示 Toast 通知，在指定组件上写入消息并临时展开 3 个 tick。
    ///
    /// 需先注册一个 Toast 组件（`ctx.register(toast("my_toast"))`）。
    /// ```ignore
    /// ctx.toast("my_toast", "文件已保存");
    /// ```
    pub fn toast(&mut self, id: &str, message: &str) {
        self.write(id, DataMode::Overwrite, message);
        self.tree.set_temp_expand(id, 3, self.tick);
    }
}

// ── EventBus ─────────────────────────────────────────────

impl EventBus for Context {
    fn emit(&mut self, event: ComponentEvent) {
        self.event_bus.emit(event);
    }

    fn on(&mut self, pattern: &str, handler: Box<dyn Fn(&ComponentEvent) + Send>) {
        self.event_bus.on(pattern, handler);
    }
}

// ── ActionContext ─────────────────────────────────────────

impl ActionContext for Context {
    fn write(&mut self, component_id: &str, mode: DataMode, data: &str) {
        self.tree.write(component_id, mode, data);
    }

    fn read(&self, component_id: &str) -> Option<String> {
        self.tree
            .find(component_id)
            .map(|n| n.render_body_only(RenderLevel::Detailed))
    }

    fn emit(&mut self, source: &str, kind: &str, data: &str) {
        self.event_bus.emit(ComponentEvent::new(source, kind, data));
    }

    fn on(&mut self, pattern: &str, handler: Box<dyn Fn(&ComponentEvent) + Send>) {
        self.event_bus.on(pattern, handler);
    }

    fn state(&self, key: &str) -> Option<String> {
        self.tree.get_global_state(key).map(|s| s.to_string())
    }

    fn set_state(&mut self, key: &str, value: &str) {
        self.tree.set_global_state(key, value);
    }

    fn resource(&self) -> Option<&dyn std::any::Any> {
        self.extension
            .as_ref()
            .map(|a| a.as_ref() as &dyn std::any::Any)
    }

    fn component_exists(&self, id: &str) -> bool {
        self.tree.find(id).is_some()
    }

    fn component_level(&self, id: &str) -> Option<RenderLevel> {
        self.tree.find(id).map(|n| n.level())
    }

    fn list_components(&self) -> Vec<(String, RenderLevel)> {
        let mut result = Vec::new();
        for node in self.tree.iter() {
            Self::collect_component_info(node, &mut result);
        }
        result
    }

    fn register_handler(&mut self, name: &str, handler: Arc<dyn ActionHandler>) {
        self.handler_registry.register(name, handler);
    }
}

impl Context {
    /// 递归收集组件 ID 和级别信息。
    fn collect_component_info(node: &ComponentNode, result: &mut Vec<(String, RenderLevel)>) {
        result.push((node.id().to_string(), node.level()));
        if let ComponentNode::Composite { children, .. } = node {
            for child in children {
                Self::collect_component_info(child, result);
            }
        }
    }
}

// ── TemplateResolver ──────────────────────────────────────

impl TemplateResolver for Context {
    fn read_component(&self, id: &str, mode: ReadMode) -> String {
        match self.tree.find(id) {
            Some(node) => {
                let level = match mode {
                    ReadMode::Full => RenderLevel::Detailed,
                    ReadMode::Truncated(_) => RenderLevel::Standard,
                    ReadMode::Trimmed(_) => RenderLevel::Detailed,
                };
                let mut body = node.render_body_only(level);
                match mode {
                    ReadMode::Truncated(max) => {
                        if body.chars().count() > max {
                            body = body.chars().take(max).collect();
                        }
                    }
                    ReadMode::Trimmed(max) => {
                        if body.chars().count() > max {
                            let half = max / 2;
                            let prefix: String = body.chars().take(half).collect();
                            let suffix: String = body
                                .chars()
                                .skip(body.chars().count().saturating_sub(half))
                                .collect();
                            body = format!("{}...{}", prefix, suffix);
                        }
                    }
                    ReadMode::Full => {}
                }
                body
            }
            None => String::new(),
        }
    }
}

// ── ConditionRender ──────────────────────────────────────────

/// 条件渲染构建器，由 [`Context::in_condition`] 返回。
///
/// 在渲染时将指定条件应用于组件树的 `VisibilityCondition::When` 评估。
/// 渲染完成后条件自动清除，不会持久化到 `ComponentTree`。
pub struct ConditionRender<'a> {
    ctx: &'a mut Context,
    conditions: Vec<String>,
    budget: usize,
}

impl ConditionRender<'_> {
    /// 添加额外条件（OR 逻辑）。
    pub fn and(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
        self
    }

    /// 指定 token 预算。
    ///
    /// ```ignore
    /// ctx.in_condition("plan").with_budget(50000).render();
    /// ```
    pub fn with_budget(mut self, budget: usize) -> Self {
        self.budget = budget;
        self
    }

    /// 渲染。
    pub fn render(self) -> String {
        let old = self.ctx.tree().conditions_snapshot();
        self.ctx.tree_mut().clear_conditions();
        for c in &self.conditions {
            self.ctx.tree_mut().add_condition(c);
        }
        let output = self.ctx.render_impl(self.budget);
        self.ctx.tree_mut().restore_conditions(old);
        output
    }

    /// 虚拟渲染（不推进 tick，不消费 recent actions）。
    pub fn render_volatile(self) -> String {
        let old = self.ctx.tree().conditions_snapshot();
        self.ctx.tree_mut().clear_conditions();
        for c in &self.conditions {
            self.ctx.tree_mut().add_condition(c);
        }
        let output = self.ctx.render_volatile_impl(self.budget);
        self.ctx.tree_mut().restore_conditions(old);
        output
    }
}

/// 预算渲染构建器，由 [`Context::with_budget`] 返回。
pub struct BudgetRender<'a> {
    ctx: &'a mut Context,
    budget: usize,
}

impl BudgetRender<'_> {
    /// 渲染。
    pub fn render(self) -> String {
        self.ctx.render_impl(self.budget)
    }

    /// 虚拟渲染（不推进 tick）。
    pub fn render_volatile(self) -> String {
        self.ctx.render_volatile_impl(self.budget)
    }
}

/// 合并动作预设参数和请求参数。
///
/// `preset` 来自 `ActionDef.params`（.cui 文件中声明），
/// `request` 来自 AI 工具调用中的参数。
/// 预设作为默认值，请求中的同名键覆盖预设。
fn merge_action_params(
    preset: Option<&std::collections::HashMap<String, String>>,
    request: Option<&str>,
) -> String {
    let mut merged: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();

    // 先插入预设参数
    if let Some(m) = preset {
        for (k, v) in m {
            merged.insert(k.clone(), v.clone());
        }
    }

    // 请求参数覆盖同名预设
    if let Some(req_str) = request
        && !req_str.trim().is_empty()
    {
        if let Ok(req_map) = serde_json::from_str::<serde_json::Value>(req_str) {
            if let Some(obj) = req_map.as_object() {
                for (k, v) in obj {
                    merged.insert(k.clone(), json_value_to_string(v));
                }
            }
        } else {
            tracing::warn!("merge_action_params: 请求参数非 JSON, 已忽略: {req_str}");
        }
    }

    if merged.is_empty() {
        String::new()
    } else {
        serde_json::to_string(&merged).unwrap_or_default()
    }
}

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ── 对话操作句柄 ──────────────────────────────────

// ── 对话读句柄 ────────────────────────────────────

pub struct DialogueRef<'a> {
    ctx: &'a Context,
}

impl<'a> DialogueRef<'a> {
    pub fn read(&self) -> Vec<String> {
        self.ctx.dialogue.read_hot_messages()
    }
    pub fn read_all(&self) -> &[String] {
        self.ctx.dialogue.read_all_messages()
    }
}

// ── 对话读写句柄 ──────────────────────────────────

/// 对话操作句柄，将分散的 `push_message` / `read_messages` / `scroll_dialogue` 等
/// 收敛到 `ctx.dialogue_mut()` 下。
pub struct DialogueHandle<'a> {
    ctx: &'a mut Context,
}

impl<'a> DialogueHandle<'a> {
    /// 推送 JSON 序列化消息。
    pub fn push(&mut self, json: &str) {
        self.ctx.dialogue.push_message(json, &mut self.ctx.tree);
    }

    /// 读取热窗消息。
    pub fn read(&self) -> Vec<String> {
        self.ctx.dialogue.read_hot_messages()
    }

    /// 读取全量消息。
    pub fn read_all(&self) -> &[String] {
        self.ctx.dialogue.read_all_messages()
    }

    /// 滚动到指定位置。
    pub fn scroll(&mut self, position: i32) -> String {
        self.ctx.dialogue
            .with_dialogue(|ops| match ops.scroll_to(position) {
                Some(msg) => format!(r#"{{"success":true,"message":"{}"}}"#, msg),
                None => r#"{"error":"滚动失败"}"#.to_string(),
            })
            .unwrap_or_else(|| r#"{"error":"对话组件未注册"}"#.to_string())
    }

    /// 按轮次滚动。
    pub fn scroll_cycles(&mut self, step: i32) -> String {
        self.ctx.dialogue
            .with_dialogue(|ops| match ops.scroll_cycles(step) {
                Some(msg) => format!(r#"{{"success":true,"message":"{}"}}"#, msg),
                None => r#"{"error":"滚动失败"}"#.to_string(),
            })
            .unwrap_or_else(|| r#"{"error":"对话组件未注册"}"#.to_string())
    }

    /// 对齐到轮次边界。
    pub fn align_turns(&mut self) -> bool {
        self.ctx.dialogue
            .with_dialogue(|ops| ops.align_turns())
            .unwrap_or(false)
    }

    /// 展开冷区域。
    pub fn expand_cold(&mut self, start: i32, end: i32) -> String {
        self.ctx.dialogue
            .with_dialogue(|ops| match ops.expand_cold(start, end) {
                Some(msg) => format!(r#"{{"success":true,"message":"{}"}}"#, msg),
                None => r#"{"success":false,"message":"展开失败"}"#.to_string(),
            })
            .unwrap_or_else(|| r#"{"error":"对话组件未注册"}"#.to_string())
    }

    /// 关闭冷区域。
    pub fn close_cold(&mut self) -> bool {
        self.ctx.dialogue
            .with_dialogue(|ops| ops.close_cold())
            .unwrap_or(false)
    }

    /// 请求冷区域。
    pub fn request_cold(&mut self) -> bool {
        self.ctx.dialogue
            .with_dialogue(|ops| ops.request_cold())
            .unwrap_or(false)
    }

    /// 冷区域倒计时推进。
    pub fn tick_cold(&mut self) -> bool {
        self.ctx.dialogue
            .with_dialogue(|ops| ops.tick_cold())
            .unwrap_or(false)
    }
}

// ── 持久化句柄 ────────────────────────────────────

pub struct PersistHandle<'a> {
    ctx: &'a Context,
}

impl<'a> PersistHandle<'a> {
    pub fn collect(&self) -> Vec<(String, Vec<String>)> {
        let mut map: Vec<(String, Vec<String>)> = Vec::new();
        for node in self.ctx.tree.iter() {
            if let Some(key) = node.persist_key() {
                let rendered = node.render_node(RenderLevel::Detailed);
                if let Some(pos) = map.iter().position(|(k, _)| k == key) {
                    map[pos].1.push(rendered);
                } else {
                    map.push((key.to_owned(), vec![rendered]));
                }
            }
        }
        map
    }
}

#[cfg(test)]
mod tests;

//! 组件节点 —— ComponentSignal、NodeSchema、NodeInfo、ComponentNode。

use std::cell::{Cell, RefCell};

use crate::action::{ActionDef, ActionResult, VisibilityRule};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::keyword::ComponentKind;
use crate::level::RenderLevel;
use crate::manage::ManageEvent;

use super::base::{BaseComponent, ComponentLifecycle, Persistable};

/// FNV-1a 哈希，用于内容变化检测。
fn hash_str(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// RenderLevel 到缓存数组索引的映射。
fn level_cache_index(level: RenderLevel) -> usize {
    match level {
        RenderLevel::Hidden => 0,
        RenderLevel::Title => 1,
        RenderLevel::Summary => 2,
        RenderLevel::Standard => 3,
        RenderLevel::Detailed => 4,
    }
}

/// 组件活跃度信号 —— 用于容量规划的热力图输入。
///
/// 替代二值 `dirty` 标记，提供渐变式新鲜度信息：
/// - `interactive`：AI 是否在最近一个 Cycle 内与此组件交互
/// - `data_freshness`：数据新鲜度 (0-255)，每次交互重置为 255，commit 后衰减
/// - `volatility`：内容波动率 (0-255)，越高表示内容越频繁变化，用于排序优化
#[derive(Clone, Debug, Default)]
pub struct ComponentSignal {
    pub(crate) interactive: bool,
    pub(crate) data_freshness: u8,
    dirty_count: u32,
    pub(crate) last_content_hash: u64,
    pub(crate) volatility: u8,
}

impl ComponentSignal {
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }
    pub fn data_freshness(&self) -> u8 {
        self.data_freshness
    }
    pub fn dirty_count(&self) -> u32 {
        self.dirty_count
    }
    pub fn volatility(&self) -> u8 {
        self.volatility
    }
}

impl ComponentSignal {
    /// 标记为活跃（AI 刚与此组件交互）。
    pub(super) fn fire(&mut self) {
        self.interactive = true;
        self.data_freshness = 255;
        self.dirty_count = self.dirty_count.saturating_add(1);
    }

    /// 衰减信号（commit 时调用）。
    pub(crate) fn cool(&mut self) {
        self.interactive = false;
        self.data_freshness = ((self.data_freshness as u16 * 3) / 4) as u8;
    }

    /// 计算热力值 (0-4)，供容量规划使用。
    pub(super) fn heat(&self) -> u8 {
        if self.interactive {
            4
        } else {
            (self.data_freshness / 64).min(3)
        } // 0..=3
    }
}

/// 节点类型信息缓存 —— 构造时从 `BaseComponent` 复制，避免重复调用 trait 方法。
#[derive(Debug, Clone)]
pub struct NodeSchema {
    pub(crate) kind: ComponentKind,
    pub(crate) inputs: Vec<crate::keyword::IoDef>,
    pub(crate) outputs: Vec<crate::keyword::IoDef>,
}

impl NodeSchema {
    pub fn kind(&self) -> ComponentKind {
        self.kind
    }
    pub fn inputs(&self) -> &[crate::keyword::IoDef] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[crate::keyword::IoDef] {
        &self.outputs
    }
}

/// 组件节点的共享数据 —— 消除 Leaf/Composite 字段重复。
pub struct NodeInfo {
    pub(crate) component: Box<dyn BaseComponent>,
    pub(crate) level: RenderLevel,
    pub(crate) signal: ComponentSignal,
    pub(crate) dynamic_actions: Vec<ActionDef>,
    pub(crate) schema: NodeSchema,
    pub(crate) render_cache: RefCell<[Option<String>; RenderLevel::VARIANT_COUNT]>,
    pub(crate) content_hash: Cell<u64>,
    pub(crate) content_gen: Cell<u64>,
    pub(crate) render_gen: Cell<u64>,
    pub(crate) lifecycle: Option<Box<dyn ComponentLifecycle>>,
    pub(crate) persist: Option<Box<dyn Persistable>>,
    actions_cache: RefCell<Option<(RenderLevel, u64, Vec<ActionDef>)>>,
    dyn_actions_gen: Cell<u64>,
    pub(crate) collapsible: bool,
    pub(crate) collapsed: bool,
    pub(crate) pinned: bool,
}

/// 组件节点 —— 树的统一表示。
pub enum ComponentNode {
    Leaf(NodeInfo),
    Composite {
        info: NodeInfo,
        children: Vec<ComponentNode>,
        budget_ratio: Option<f32>,
    },
}

macro_rules! find_impl {
    ($slf:expr, $id:expr, $iter:ident, $recur:ident) => {{
        if $slf.id() == $id {
            return Some($slf);
        }
        if let ComponentNode::Composite { children, .. } = $slf {
            for child in children.$iter() {
                if let Some(found) = child.$recur($id) {
                    return Some(found);
                }
            }
        }
        None
    }};
}

impl ComponentNode {
    fn make_info(
        component: Box<dyn BaseComponent>,
        lifecycle: Option<Box<dyn ComponentLifecycle>>,
        persist: Option<Box<dyn Persistable>>,
    ) -> NodeInfo {
        let schema = NodeSchema {
            kind: component.kind(),
            inputs: component.input_schema().to_vec(),
            outputs: component.output_schema().to_vec(),
        };
        NodeInfo {
            component,
            level: RenderLevel::Standard,
            signal: ComponentSignal::default(),
            dynamic_actions: Vec::new(),
            schema,
            render_cache: RefCell::new(Default::default()),
            content_hash: Cell::new(0),
            content_gen: Cell::new(1),
            render_gen: Cell::new(0),
            lifecycle,
            persist,
            actions_cache: RefCell::new(None),
            dyn_actions_gen: Cell::new(0),
            collapsible: false,
            collapsed: true,
            pinned: false,
        }
    }

    pub fn leaf(component: impl BaseComponent + 'static) -> Self {
        let info = Self::make_info(Box::new(component), None, None);
        Self::Leaf(info)
    }

    pub fn leaf_with_lifecycle(
        component: impl BaseComponent + 'static,
        lifecycle: impl ComponentLifecycle + 'static,
        persist: impl Persistable + 'static,
    ) -> Self {
        Self::Leaf(Self::make_info(
            Box::new(component),
            Some(Box::new(lifecycle)),
            Some(Box::new(persist)),
        ))
    }

    pub fn composite(
        component: impl BaseComponent + 'static,
        children: Vec<ComponentNode>,
    ) -> Self {
        Self::Composite {
            info: Self::make_info(Box::new(component), None, None),
            children,
            budget_ratio: None,
        }
    }

    // ── 私有访问器（通过 NodeInfo 消除重复） ──

    pub(crate) fn info(&self) -> &NodeInfo {
        match self {
            Self::Leaf(info) => info,
            Self::Composite { info, .. } => info,
        }
    }

    pub(crate) fn info_mut(&mut self) -> &mut NodeInfo {
        match self {
            Self::Leaf(info) => info,
            Self::Composite { info, .. } => info,
        }
    }

    pub(crate) fn component_ref(&self) -> &dyn BaseComponent {
        self.info().component.as_ref()
    }

    pub(crate) fn component_mut(&mut self) -> &mut Box<dyn BaseComponent> {
        &mut self.info_mut().component
    }

    pub(super) fn level_val(&self) -> RenderLevel {
        self.info().level
    }

    pub(super) fn level_mut(&mut self) -> &mut RenderLevel {
        &mut self.info_mut().level
    }

    pub(super) fn signal_ref(&self) -> &ComponentSignal {
        &self.info().signal
    }

    pub(super) fn signal_mut(&mut self) -> &mut ComponentSignal {
        &mut self.info_mut().signal
    }

    pub(super) fn dyn_actions(&self) -> &[ActionDef] {
        &self.info().dynamic_actions
    }

    pub(super) fn dyn_actions_mut(&mut self) -> &mut Vec<ActionDef> {
        &mut self.info_mut().dynamic_actions
    }

    /// 节点 schema（缓存自 BaseComponent 的 kind/input_schema/output_schema）。
    pub fn schema(&self) -> &NodeSchema {
        &self.info().schema
    }

    // ── 公开方法 ──

    /// 替换 node 的 `dynamic_actions`（来自 .cui 前端 actions）。
    pub fn set_actions(&mut self, actions: Vec<ActionDef>) {
        *self.dyn_actions_mut() = actions;
        let info = self.info_mut();
        info.dyn_actions_gen
            .set(info.dyn_actions_gen.get().wrapping_add(1));
    }

    pub fn id(&self) -> &str {
        self.component_ref().id()
    }
    pub fn title(&self) -> &str {
        self.component_ref().title()
    }
    pub fn priority(&self) -> crate::keyword::PriorityLevel {
        self.component_ref().priority()
    }
    pub fn is_static(&self) -> bool {
        self.component_ref().is_static()
    }
    pub fn is_inert(&self) -> bool {
        self.component_ref().is_inert()
    }
    pub fn is_collapsible(&self) -> bool {
        self.info().collapsible
    }
    pub fn set_collapsible(&mut self, v: bool) {
        self.info_mut().collapsible = v;
    }
    pub fn is_collapsed(&self) -> bool {
        self.info().collapsed
    }
    pub fn set_collapsed(&mut self, v: bool) {
        self.info_mut().collapsed = v;
    }
    pub fn is_pinned(&self) -> bool {
        self.info().pinned
    }
    pub(crate) fn set_pinned(&mut self, v: bool) {
        self.info_mut().pinned = v;
    }
    pub fn visibility_condition(&self) -> VisibilityCondition {
        self.component_ref().visibility_condition()
    }
    pub fn level(&self) -> RenderLevel {
        self.level_val()
    }
    pub fn dirty_count(&self) -> u32 {
        self.signal_ref().dirty_count
    }

    pub fn set_level(&mut self, level: RenderLevel) {
        let old = *self.level_mut();
        *self.level_mut() = level;
        if old != level
            && let Some(lc) = &mut self.info_mut().lifecycle
        {
            lc.on_level_change(old, level);
        }
    }

    pub fn mark_dirty(&mut self) {
        self.signal_mut().fire();
        // 清空渲染缓存，下次 render 重新生成
        self.info_mut().render_cache.borrow_mut().fill(None);
    }

    /// 检查节点或其子树是否有活跃信号。
    pub fn is_dirty(&self) -> bool {
        if self.signal_ref().interactive || self.signal_ref().data_freshness > 0 {
            return true;
        }
        if let Self::Composite { children, .. } = self {
            children.iter().any(|c| c.is_dirty())
        } else {
            false
        }
    }

    /// 获取节点的容量规划热力值。
    pub fn heat(&self) -> u8 {
        self.signal_ref().heat()
    }

    /// 获取内容波动率 (0-255)：越低越稳定，适合做缓存前缀。
    pub fn volatility(&self) -> u8 {
        self.signal_ref().volatility
    }

    #[cfg(test)]
    pub fn set_volatility(&mut self, v: u8) {
        self.info_mut().signal.volatility = v;
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Self> {
        find_impl!(self, id, iter_mut, find_mut)
    }

    pub fn find(&self, id: &str) -> Option<&Self> {
        find_impl!(self, id, iter, find)
    }

    pub fn has_child(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    /// 从 Composite 子节点中递归移除指定 ID 的子节点。
    ///
    /// 仅在 Composite 的 children 中搜索，不匹配 self。
    /// 移除前调用 `on_unmount()`。
    pub fn remove_child(&mut self, id: &str) -> Option<ComponentNode> {
        if let Self::Composite { children, .. } = self {
            if let Some(idx) = children.iter().position(|n| n.id() == id) {
                if let Some(lc) = &mut children[idx].info_mut().lifecycle {
                    lc.on_unmount();
                }
                return Some(children.remove(idx));
            }
            for child in children.iter_mut() {
                if let Some(found) = child.remove_child(id) {
                    return Some(found);
                }
            }
        }
        None
    }

    pub fn actions(&self, current_level: RenderLevel) -> Vec<ActionDef> {
        let info = self.info();
        let gen_id = info.dyn_actions_gen.get();
        {
            let cache = info.actions_cache.borrow();
            if let Some((cached_level, cached_gen, cached)) = &*cache
                && *cached_level == current_level
                && *cached_gen == gen_id
            {
                return cached.clone();
            }
        }

        let filter_level = |v: &VisibilityRule| match v {
            VisibilityRule::LevelLessThan(max) => current_level < *max,
            VisibilityRule::LevelGreaterThan(min) => current_level > *min,
        };

        let mut merged: Vec<ActionDef> = self
            .component_ref()
            .action_variants()
            .iter()
            .filter(|v| v.show_when.as_ref().is_none_or(filter_level))
            .map(ActionDef::from)
            .collect();

        for da in self.dyn_actions() {
            let passes = da.show_when().is_none_or(filter_level);
            if !passes {
                continue;
            }
            if let Some(existing) = merged.iter_mut().find(|a| a.id() == da.id()) {
                *existing = da.clone();
            } else {
                merged.push(da.clone());
            }
        }

        if self.is_collapsible() {
            let expand = ActionDef::new("expand", "展开")
                .with_target_level(RenderLevel::Standard)
                .with_show_when(VisibilityRule::LevelLessThan(RenderLevel::Standard));
            if expand.show_when().is_none_or(&filter_level) {
                merged.push(expand);
            }
            let collapse = ActionDef::new("collapse", "折叠")
                .with_target_level(RenderLevel::Summary)
                .with_show_when(VisibilityRule::LevelGreaterThan(RenderLevel::Summary));
            if collapse.show_when().is_none_or(&filter_level) {
                merged.push(collapse);
            }
        }

        *info.actions_cache.borrow_mut() = Some((current_level, gen_id, merged.clone()));
        merged
    }

    pub fn handle_action(&mut self, action: &str, params: &str) -> ActionResult {
        let is_collapsible = self.is_collapsible();
        let info = self.info_mut();
        let component_id = info.component.id().to_string();

        // 通用展开/折叠：所有可折叠组件自动获得，无需各组件重复实现
        if is_collapsible {
            match action {
                "expand" if info.level < RenderLevel::Standard => {
                    info.level = RenderLevel::Standard;
                    info.signal.fire();
                    info.render_cache.borrow_mut().fill(None);
                    let snapshot = info.component.render(info.level);
                    return ActionResult::new(component_id, action.to_string())
                        .with_message("已展开")
                        .with_new_level(RenderLevel::Standard)
                        .with_snapshot(snapshot);
                }
                "collapse" if info.level > RenderLevel::Summary => {
                    info.level = RenderLevel::Summary;
                    info.signal.fire();
                    info.render_cache.borrow_mut().fill(None);
                    let snapshot = info.component.render(info.level);
                    return ActionResult::new(component_id, action.to_string())
                        .with_message("已折叠")
                        .with_new_level(RenderLevel::Summary)
                        .with_snapshot(snapshot);
                }
                _ => {}
            }
        }

        if let Some(result) = Self::try_self_or_dynamic(
            &mut info.component,
            &mut info.level,
            &mut info.signal,
            &info.dynamic_actions,
            action,
            params,
        ) {
            return result;
        }
        // 对于 Composite：递归子节点
        if let Self::Composite { children, .. } = self {
            for child in children.iter_mut() {
                let result = child.handle_action(action, params);
                if result.is_success() {
                    return result;
                }
            }
        }
        ActionResult::error(&component_id, action, format!("未知动作: {action}"))
    }

    /// 组件自身 + 动态 action 回退：None = 两者都不认得。
    fn try_self_or_dynamic(
        component: &mut Box<dyn BaseComponent>,
        level: &mut RenderLevel,
        signal: &mut ComponentSignal,
        dynamic_actions: &[ActionDef],
        action: &str,
        params: &str,
    ) -> Option<ActionResult> {
        let mut result = component.handle_action(action, params);
        if result.is_success() {
            if let Some(new_level) = result.new_level() {
                *level = new_level;
            }
            result.set_snapshot(component.render(*level));
            signal.fire();
            result.set_component_id(component.id().to_string());
            return Some(result);
        }
        // 回退到动态 actions（来自 .cui 声明）
        if let Some(def) = dynamic_actions.iter().find(|a| a.id() == action) {
            if let Some(target) = def.target_level() {
                *level = target;
            }
            let snapshot = component.render(*level);
            signal.fire();
            let mut r = ActionResult::new(component.id().to_string(), action.to_string())
                .with_message(format!("{} 已完成", def.label()))
                .with_snapshot(snapshot);
            if let Some(lvl) = def.target_level() {
                r = r.with_new_level(lvl);
            }
            return Some(r);
        }
        None
    }

    pub fn write(&mut self, mode: DataMode, data: &str) {
        self.component_mut().write(mode, data);
        if let Some(lc) = &mut self.info_mut().lifecycle {
            lc.on_update(mode, data);
        }
        let cg = self.info().content_gen.get() + 1;
        self.info_mut().content_gen.set(cg);
        self.mark_dirty();
    }

    pub fn on_event(&mut self, event: ManageEvent) {
        if let Some(lc) = &mut self.info_mut().lifecycle {
            lc.on_event(event);
        }
        if let Self::Composite { children, .. } = self {
            for child in children.iter_mut() {
                child.on_event(event);
            }
        }
    }

    pub fn start_new_cycle(&mut self, cycle_id: u32) {
        if let Some(lc) = &mut self.info_mut().lifecycle {
            lc.start_new_cycle(cycle_id);
        }
        if let Self::Composite { children, .. } = self {
            for child in children.iter_mut() {
                child.start_new_cycle(cycle_id);
            }
        }
    }

    pub fn compress(&mut self) -> bool {
        let any = self
            .info_mut()
            .lifecycle
            .as_mut()
            .is_some_and(|lc| lc.compress());
        if let Self::Composite { children, .. } = self {
            children
                .iter_mut()
                .fold(any, |acc, child| child.compress() || acc)
        } else {
            any
        }
    }

    pub fn persist_key(&self) -> Option<&str> {
        self.info().persist.as_ref().and_then(|p| p.persist_key())
    }

    /// 渲染当前节点（不递归子节点）。
    ///
    /// Action/Inline 组件仅返回 body，无标题栏包装；
    /// 其余 kind 使用完整 `## [id] title` 块格式。
    pub fn render_node(&self, level: RenderLevel) -> String {
        let component = self.component_ref();
        let body = self.render_body_only(level);
        let output = match component.kind() {
            ComponentKind::Action | ComponentKind::Inline => body,
            _ => {
                let actions = self.actions(level);
                crate::runtime::output::render_component(
                    component.id(),
                    component.title(),
                    level,
                    &body,
                    &actions,
                    self.is_dirty(),
                    component.priority(),
                )
            }
        };
        self.info().render_gen.set(self.info().content_gen.get());
        output
    }

    /// 仅渲染正文（不含 `render_node` 的标题前缀和动作按钮）。
    ///
    /// 内部使用渲染缓存：同一级别下内容不变时返回缓存结果。
    /// 缓存通过 `mark_dirty()` 和 `write()` 自动失效。
    pub fn render_body_only(&self, level: RenderLevel) -> String {
        if !self.component_ref().should_render(level) {
            return String::new();
        }
        let idx = level_cache_index(level);
        let cache = self.info().render_cache.borrow();
        if let Some(ref cached) = cache[idx] {
            let rendered = cached.clone();
            drop(cache);
            self.info().content_hash.set(hash_str(&rendered));
            return rendered;
        }
        drop(cache);
        let rendered = self.component_ref().render(level);
        self.info().render_cache.borrow_mut()[idx] = Some(rendered.clone());
        self.info().content_hash.set(hash_str(&rendered));
        rendered
    }

    /// 递归渲染当前节点及所有子节点。
    ///
    /// 对于 Leaf：等价于 `render_node`。
    /// 对于 Composite：先渲染自身 header，再递归渲染所有非 Hidden 子节点。
    /// `delta` 为 true 时使用差量渲染，未变化节点被收集为批量引用输出。
    pub fn render_recursive(&self, level: RenderLevel, delta: bool) -> String {
        let mut out = if delta {
            if self.info().content_gen.get() == self.info().render_gen.get() {
                match self.component_ref().kind() {
                    ComponentKind::Action | ComponentKind::Inline => String::new(),
                    _ => crate::runtime::output::render_delta_marker(self.component_ref().id()),
                }
            } else {
                self.render_node(level)
            }
        } else {
            self.render_node(level)
        };
        if let Self::Composite { children, .. } = self {
            // 复合节点自身未变化时（delta 标记），子节点可能个体变化；
            // 复合节点自身变化时也一样，都需要遍历子节点。
            // 对复合节点不在 delta 模式下跳过子节点，因为子节点的变化独立于父节点。
            let mut batched: Vec<String> = Vec::new();
            for child in children.iter() {
                let child_level = child.level();
                if child_level == RenderLevel::Hidden {
                    continue;
                }
                let child_id = child.id();
                if delta {
                    let cg = child.info().content_gen.get();
                    let rg = child.info().render_gen.get();
                    if cg == rg {
                        let kind = child.component_ref().kind();
                        if !matches!(kind, ComponentKind::Action | ComponentKind::Inline) {
                            batched.push(child_id.to_string());
                            continue;
                        }
                    }
                }
                if !batched.is_empty() {
                    out.push_str(&format!("Unchanged: {}\n", batched.join(", ")));
                    batched.clear();
                }
                out.push_str(&child.render_recursive(child_level, delta));
            }
            if !batched.is_empty() {
                out.push_str(&format!("Unchanged: {}\n", batched.join(", ")));
            }
        }
        out
    }
}

impl std::fmt::Debug for ComponentNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leaf(info) => f
                .debug_struct("Leaf")
                .field("id", &info.component.id())
                .field("level", &info.level)
                .field("interactive", &info.signal.interactive)
                .finish(),
            Self::Composite { info, children, .. } => f
                .debug_struct("Composite")
                .field("id", &info.component.id())
                .field("level", &info.level)
                .field("children", &children.len())
                .finish(),
        }
    }
}

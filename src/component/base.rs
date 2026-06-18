//! 组件接口 —— CuiComponent、ComponentLifecycle、Persistable trait。

use crate::action::{ActionResult, ActionVariant};
use crate::condition::VisibilityCondition;
use crate::data::DataMode;
use crate::keyword::{ComponentKind, IoDef};
use crate::level::RenderLevel;
use crate::manage::ManageEvent;

/// 组件生命周期 —— 可选的行为扩展。
///
/// 实现此 trait 的组件可以通过 `ComponentNode::leaf_with_lifecycle` 注入。
/// 默认空实现（no-op）。
pub trait ComponentLifecycle: Send {
    /// 组件被挂载到树中时调用。
    fn on_mount(&mut self) {}
    /// 组件从树中移除时调用。
    fn on_unmount(&mut self) {}
    /// 组件渲染级别变更时调用。
    fn on_level_change(&mut self, _old: RenderLevel, _new: RenderLevel) {}
    /// 组件收到外部事件时调用。
    fn on_event(&mut self, _event: ManageEvent) {}
    /// 组件收到新数据时调用（通过 write）。
    fn on_update(&mut self, _mode: DataMode, _data: &str) {}
    /// 压缩旧数据，返回 true 表示有数据被清理。
    fn compress(&mut self) -> bool {
        false
    }
    /// 新 cycle 开始。
    fn start_new_cycle(&mut self, _cycle_id: u32) {}
}

/// 持久化 —— 可选的行为扩展。
///
/// 实现此 trait 的组件可以通过 `ComponentNode::leaf_with_lifecycle` 注入。
pub trait Persistable: Send {
    fn persist_key(&self) -> Option<&str> {
        None
    }
}

/// 基础组件接口 —— 所有组件必须实现的最小接口。
///
/// 包含渲染和交互方法。生命周期和持久化方法已移至 [`ComponentLifecycle`] 和 [`Persistable`]。
pub trait CuiComponent: Send {
    // ── 必需方法 ──

    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn priority(&self) -> crate::keyword::PriorityLevel;

    /// 按指定级别渲染 Markdown 正文。
    fn render(&self, level: RenderLevel) -> String;

    /// 处理 AI 动作。
    fn handle_action(&mut self, action: &str, params: &str) -> ActionResult;

    // ── 可选渲染/元数据方法 ──

    /// 预估指定级别的 token 消耗。
    fn estimated_tokens(&self, level: RenderLevel) -> usize {
        let body = self.render(level);
        crate::tokenizer::estimate(&body)
    }

    fn action_variants(&self) -> &'static [ActionVariant] {
        &[]
    }
    fn is_static(&self) -> bool {
        false
    }
    fn is_inert(&self) -> bool {
        false
    }
    fn visibility_condition(&self) -> VisibilityCondition {
        VisibilityCondition::Always
    }

    /// 组件类型分类。
    fn kind(&self) -> ComponentKind {
        ComponentKind::Block
    }
    /// 层级类型的子类标签（如 `tool.bash` → `"bash"`）。
    fn subtype(&self) -> Option<&str> {
        None
    }
    /// 输入参数 schema。
    fn input_schema(&self) -> &[IoDef] {
        &[]
    }
    /// 输出参数 schema。
    fn output_schema(&self) -> &[IoDef] {
        &[]
    }

    /// 接收外部数据写入（默认 no-op）。
    fn write(&mut self, _mode: DataMode, _data: &str) {}

    /// 是否应在指定级别渲染。返回 false 则跳过渲染（仍保留缓存）。
    fn should_render(&self, _level: RenderLevel) -> bool {
        true
    }

    /// 用户是否固定此组件（跳过预算降级，优先升级）。
    /// 仅通过用户覆盖路径设置，开发者文件不可用。
    fn is_pinned(&self) -> bool {
        false
    }

    /// 用于内部 downcasting（仅框架内部使用）。
    #[doc(hidden)]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

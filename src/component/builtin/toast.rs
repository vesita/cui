use crate::action::{ActionResult, ActionVariant};
use crate::component::base::CuiComponent;
use crate::component::node::ComponentNode;
use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;

/// Toast 临时通知组件。
///
/// 类似 UI 框架中的 toast 通知：显示一条消息，若干周期后自动消失。
/// 配合 `ComponentTree::set_temp_expand` 使用，或通过 `Context::toast()` 便捷方法。
///
/// ```ignore
/// ctx.register(toast("my_toast"));
/// ctx.toast("my_toast", "文件已保存");  // 3 周期后自动消失
/// ```
pub struct Toast {
    id: String,
    message: String,
}

impl Toast {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            message: String::new(),
        }
    }
}

impl CuiComponent for Toast {
    fn id(&self) -> &str {
        &self.id
    }

    fn title(&self) -> &str {
        "通知"
    }

    fn priority(&self) -> PriorityLevel {
        PriorityLevel::Critical
    }

    fn render(&self, level: RenderLevel) -> String {
        if level < RenderLevel::Summary || self.message.is_empty() {
            return String::new();
        }
        let body = format!("[通知] {}\n", self.message);
        if level >= RenderLevel::Standard {
            crate::format_cui_block(&[("type", "toast"), ("id", &self.id)], &body)
        } else {
            body
        }
    }

    fn handle_action(&mut self, action: &str, params: &str) -> ActionResult {
        match action {
            "show" => {
                self.message = params.to_string();
                ActionResult::new(&self.id, action.to_string())
                    .with_message("通知已显示")
                    .with_new_level(RenderLevel::Summary)
            }
            "dismiss" => {
                self.message.clear();
                ActionResult::new(&self.id, action.to_string()).with_message("通知已关闭")
            }
            _ => ActionResult::error(&self.id, action, "未知 toast 动作"),
        }
    }

    fn action_variants(&self) -> &'static [ActionVariant] {
        static ACTIONS: &[ActionVariant] = &[
            ActionVariant::new("show", "显示通知"),
            ActionVariant::new("dismiss", "关闭通知"),
        ];
        ACTIONS
    }

    fn is_static(&self) -> bool {
        !self.message.is_empty()
    }

    fn write(&mut self, mode: DataMode, data: &str) {
        match mode {
            DataMode::Overwrite => self.message = data.to_string(),
            DataMode::Append => {
                self.message.push_str(data);
            }
            DataMode::Clear => self.message.clear(),
        }
    }
}

/// 创建 Toast 组件节点。
pub fn toast(id: impl Into<String>) -> ComponentNode {
    ComponentNode::leaf(Toast::new(id))
}

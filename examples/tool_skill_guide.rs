//! # 工具与技能注册指南
//!
//! 演示如何用链式 API 声明工具和技能。
//!
//! ```bash
//! cargo run --example tool_skill_guide
//! ```

use std::sync::Arc;

use cui::runtime::handler::{ActionContext, ActionHandler, ActionOutput, HandlerRegistry};
use cui::runtime::registry::{ComponentTypeDef, TypeRegistry};
use cui::{Cui, PriorityLevel};

fn main() {
    let mut type_registry = TypeRegistry::new();
    type_registry.register(ComponentTypeDef {
        name: "tool.code_review".into(),
        default_kind: cui::keyword::ComponentKind::Block,
        default_actions: vec![
            cui::action::ActionDef::new("expand", "展开详情")
                .with_target_level(cui::RenderLevel::Detailed),
            cui::action::ActionDef::new("execute", "执行审查")
                .with_handler(cui::runtime::handler::ActionHandlerRef::Unresolved("handler".into())),
            cui::action::ActionDef::new("collapse", "折叠")
                .with_target_level(cui::RenderLevel::Summary)
                .with_show_when(cui::action::VisibilityRule::LevelGreaterThan(
                    cui::RenderLevel::Summary,
                )),
        ],
        body_template: None,
        inputs: vec![],
        default_priority: Some(PriorityLevel::High),
        default_inert: false,
        default_static: false,
        description: "代码审查工具".into(),
    });

    let mut handler_registry = HandlerRegistry::new();
    handler_registry.register("tool.read_file", Arc::new(ReadFileHandler));
    handler_registry.register("tool.run_test", Arc::new(RunTestHandler));
    handler_registry.register("tool.code_review", Arc::new(CodeReviewHandler));

    let mut ctx = Cui::init()
        .without_introduction()
        .type_registry(type_registry)
        .tools("tools", "可用工具", PriorityLevel::High, (
            "examples/cui/tools/read_file.cui",
            "examples/cui/tools/run_test.cui",
            "examples/cui/tools/code_review.cui",
        ))
        .skills("skill_ref", "技能参考", (
            ("Rust 代码审查", "检查 unsafe 块、错误处理、性能瓶颈"),
            ("安全审计", "扫描 SQL 注入、XSS、CSRF 漏洞"),
        ))
        .handlers(&handler_registry)
        .build();

    println!("══════════════════════════════════════════");
    println!("完整渲染");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.render());

    println!("\n══════════════════════════════════════════");
    println!("act 条件（工具可见）");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("act").render());
}

struct ReadFileHandler;
impl ActionHandler for ReadFileHandler {
    fn id(&self) -> &str { "tool.read_file" }
    fn execute(&self, _params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
        Ok(ActionOutput::success("读取 src/main.rs: 42 行，无问题"))
    }
}

struct RunTestHandler;
impl ActionHandler for RunTestHandler {
    fn id(&self) -> &str { "tool.run_test" }
    fn execute(&self, _params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
        Ok(ActionOutput::success("测试通过: 18/18"))
    }
}

struct CodeReviewHandler;
impl ActionHandler for CodeReviewHandler {
    fn id(&self) -> &str { "tool.code_review" }
    fn execute(&self, _params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
        Ok(ActionOutput::success("审查完成: 发现 1 个高严重度问题"))
    }
}

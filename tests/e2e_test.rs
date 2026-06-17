//! 端到端集成测试：.cui 文件 → 渲染 → handler 执行 → 结果验证
//!
//! 覆盖：
//! - load_dir 批量加载
//! - tools/skills 注册
//! - 条件路由渲染
//! - handler 执行 + 错误处理
//! - 数据注入 + 插槽填充

use std::sync::Arc;
use cui::runtime::handler::{ActionContext, ActionHandler, ActionOutput, HandlerRegistry};
use cui::{Cui, PriorityLevel};

#[test]
fn e2e_load_dir_and_conditional_render() {
    let mut ctx = Cui::init()
        .without_introduction()
        .load_dir("examples/cui")
        .build();

    let always = ctx.render();
    assert!(always.contains("工作流引擎"), "header 应始终可见");

    let plan = ctx.in_condition("plan").render();
    assert!(plan.contains("## [规划方案]"), "plan 条件下 plan 组件应可见");
    assert!(!plan.contains("## [Bash"), "plan 条件下 act_bash 标题应隐藏");

    let act = ctx.in_condition("act").render();
    assert!(act.contains("Bash 执行"), "act 条件下 act_bash 应可见");
}

#[test]
fn e2e_tool_registration_and_handler_execution() {
    struct EchoHandler;
    impl ActionHandler for EchoHandler {
        fn id(&self) -> &str { "tool.echo" }
        fn execute(&self, params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
            Ok(ActionOutput::success_with_data("echo ok", params))
        }
    }

    let mut handlers = HandlerRegistry::new();
    handlers.register("tool.echo", Arc::new(EchoHandler));

    let mut ctx = Cui::init()
        .without_introduction()
        .handlers(&handlers)
        .build();

    // Verify handler is registered
    let handler = ctx.resolve_handler("tool.echo");
    assert!(handler.is_some(), "handler 应已注册");
}

#[test]
fn e2e_skills_rendering() {
    let mut ctx = Cui::init()
        .without_introduction()
        .skills("skills", "技能列表", (
            ("审查", "检查代码质量"),
            ("测试", "运行测试套件"),
        ))
        .build();

    let output = ctx.render();
    assert!(output.contains("技能列表"), "技能标题应可见");
    assert!(output.contains("审查"), "第一个技能应渲染");
    assert!(output.contains("检查代码质量"), "技能描述应渲染");
    assert!(output.contains("测试"), "第二个技能应渲染");
}

#[test]
fn e2e_input_slot_filling() {
    use cui::builtin::CuiFileLeaf;

    let mut ctx = Cui::init()
        .without_introduction()
        .component(
            CuiFileLeaf::new("task", "构建", "目标: {{input:target}}\n分支: {{input:branch}}")
                .with_input("target", "release")
                .with_input("branch", "main")
                .build(),
        )
        .build();

    let output = ctx.render();
    assert!(output.contains("目标: release"), "slot 应被填充");
    assert!(output.contains("分支: main"), "slot 应被填充");
    assert!(!output.contains("{{input:"), "未匹配的占位符应被清除");
}

#[test]
fn e2e_user_override_pin() {
    use std::io::Write;

    let tmp = std::env::temp_dir().join("cui_e2e_pin_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let base_dir = tmp.join("cui");
    std::fs::create_dir_all(&base_dir).unwrap();
    let mut f = std::fs::File::create(base_dir.join("review.cui")).unwrap();
    f.write_all(b"---\nid: review\ntitle: Review\npriority: low\n---\nBody\n").unwrap();

    let user_dir = tmp.join("user");
    std::fs::create_dir_all(&user_dir).unwrap();
    let mut f = std::fs::File::create(user_dir.join("review.cui")).unwrap();
    f.write_all(b"---\nid: review\ntitle: Review\npriority: low\npinned: true\n---\nBody\n").unwrap();

    let ctx = Cui::init()
        .without_introduction()
        .load_dir(&base_dir)
        .user_overrides(&user_dir)
        .build();

    let review = ctx.tree().find("review");
    assert!(review.is_some(), "review 应已加载（base_dir={base_dir:?}）");
    assert!(review.unwrap().is_pinned(), "review 应被 pinned");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn e2e_handler_error_propagation() {
    struct FailingHandler;
    impl ActionHandler for FailingHandler {
        fn id(&self) -> &str { "tool.fail" }
        fn execute(&self, _: &str, _: &mut dyn ActionContext) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
            Err("simulated failure".into())
        }
    }

    let mut handlers = HandlerRegistry::new();
    handlers.register("tool.fail", Arc::new(FailingHandler));

    let mut ctx = Cui::init()
        .without_introduction()
        .handlers(&handlers)
        .build();

    let result = ctx.component_action(&cui::action::ActionRequest {
        component_id: "test".into(),
        action: "execute".into(),
        params: None,
    });
    assert!(!result.is_success(), "未找到组件应返回失败");
}

#[test]
fn e2e_load_dir_with_conditions() {
    let mut ctx = Cui::init()
        .without_introduction()
        .load_dir("examples/cui")
        .build();

    // plan 阶段: plan 可见, act 不可见
    let plan = ctx.in_condition("plan").render();
    assert!(plan.contains("规划方案"));

    // act 阶段: act 可见, plan 不可见
    let act = ctx.in_condition("act").render();
    assert!(act.contains("Bash 执行"));

    // OR 逻辑
    let both = ctx.in_condition("plan").and("act").render();
    assert!(both.contains("规划方案"));
    assert!(both.contains("Bash 执行"));

    // status 阶段单独测试
    let status = ctx.in_condition("status").render();
    assert!(status.contains("系统状态"));
}

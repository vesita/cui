//! # CUI Conditional Rendering Guide
//!
//! 展示 CUI 框架的条件渲染机制 —— 组件在构建期声明条件，渲染期通过
//! `in_condition()` 过滤可见性；
//! `with_budget()` 控制 token 预算；
//! `render_volatile()` 做无副作用预览。
//!
//! ```bash
//! cargo run --example conditional_rendering
//! ```

use cui::condition::VisibilityCondition;
use cui::builtin::{data_slot, group, TextBlock};
use cui::Cui;

fn main() {
    let mut ctx = init_scene();

    println!("══════════════════════════════════════════");
    println!("1. render() — 无条件，仅 Always 可见");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.render());

    println!("\n══════════════════════════════════════════");
    println!("2. in_condition(\"plan\").render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").render());

    println!("\n══════════════════════════════════════════");
    println!("3. in_condition(\"plan\").and(\"debug\").render() — OR");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").and("debug").render());

    println!("\n══════════════════════════════════════════");
    println!("4. in_condition(\"act\").render() — 切换阶段");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("act").render());

    println!("\n══════════════════════════════════════════");
    println!("5. with_budget(500).render() — 低预算容量规划");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.with_budget(500).render());

    println!("\n══════════════════════════════════════════");
    println!("6. in_condition(\"plan\").with_budget(800).render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").with_budget(800).render());

    println!("\n══════════════════════════════════════════");
    println!("7. with_budget(50000).render_volatile() — 无副作用预览");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.with_budget(50000).render_volatile());

    println!("\n══════════════════════════════════════════");
    println!("8. render() — 条件不残留，trigger 后可见");
    println!("══════════════════════════════════════════");
    ctx.trigger("config_changed");
    println!("{}", ctx.render());

    println!("\n══════════════════════════════════════════");
    println!("9. in_condition(\"plan\").and(\"act\").and(\"review\").render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").and("act").and("review").render());
}

fn init_scene() -> cui::Context {
    Cui::init()
        .component(TextBlock::new("header", "工作流引擎", "功能: 阶段切换 + 预算控制 + 无副作用预览").build())
        .component(data_slot("status_slot", "运行时状态"))
        .component(
            group("plan_group", "规划阶段")
                .with_condition(VisibilityCondition::when("plan"))
                .push(TextBlock::new("plan_overview", "规划概览", "分析用户意图").with_condition(VisibilityCondition::when("plan")).build())
                .push(TextBlock::new("plan_subtasks", "子任务", "- 分析代码\n- 生成方案").with_condition(VisibilityCondition::when("plan")).build())
                .build(),
        )
        .component(TextBlock::new("plan_steps", "执行步骤", "1.读取 2.变更 3.diff 4.apply").with_condition(VisibilityCondition::when("plan")).build())
        .component(TextBlock::new("tool_bash", "bash", "```\necho executing\n```").with_condition(VisibilityCondition::when("act")).build())
        .component(TextBlock::new("tool_debug", "诊断面板", "内存: OK\n连接: 3\n缓存: 87%").with_condition(VisibilityCondition::when("debug")).build())
        .component(TextBlock::new("review_block", "审查结果", "src/main.rs: +12 -3\n所有测试通过").with_condition(VisibilityCondition::when("review")).build())
        .component(TextBlock::new("config_notify", "配置已变更", "timeout: 30s → 60s").with_condition(VisibilityCondition::OnTrigger("config_changed".into())).build())
        .build()
}

//! # CUI Conditional Rendering Guide
//!
//! 本示例展示 CUI 框架的三种条件渲染机制，可作为其他项目的参考指南。
//!
//! ## 三种可见性条件
//!
//! | 条件              | 行为                            | 适用场景                          |
//! |-------------------|---------------------------------|-----------------------------------|
//! | `Always`          | 始终可见（默认）                | 固定内容、核心规则                |
//! | `When("phase")`   | 当 `"phase"` 在活跃条件集中可见 | 阶段切换（plan/act/review）       |
//! | `OnTrigger("evt")`| 调用 `ctx.trigger("evt")` 后可见 | MCP 工具变更、异步外部事件         |
//!
//! ## 运行方式
//!
//! ```bash
//! cargo run --example conditional_rendering
//! ```
//!
//! ## 核心 API
//!
//! - `ctx.set_condition("plan")`        添加条件到活跃集
//! - `ctx.remove_condition("plan")`     移除一条条件
//! - `ctx.clear_conditions()`           清空所有条件
//! - `ctx.render_with_condition("act")` 添加条件并渲染（便捷方法）
//! - `ctx.trigger("config_changed")`    触发外部事件
//! - `TextBlock::new(...).with_condition(c).build()` 声明条件

use cui::condition::VisibilityCondition;
use cui::builtin::{data_slot, group, TextBlock};
use cui::Cui;

fn main() {
    let mut ctx = init_scene();

    println!("══════════════════════════════════════════");
    println!("1. 初始状态 ── 无任何条件");
    println!("══════════════════════════════════════════");
    print_section(&mut ctx, "initial");

    println!("\n══════════════════════════════════════════");
    println!("2. 进入 planning 阶段 ── set_condition(\"plan\")");
    println!("══════════════════════════════════════════");
    ctx.set_condition("plan");
    print_section(&mut ctx, "plan");

    println!("\n══════════════════════════════════════════");
    println!("3. 多个条件共存 ── plan + debug");
    println!("══════════════════════════════════════════");
    ctx.set_condition("debug");
    print_section(&mut ctx, "plan+debug");

    println!("\n══════════════════════════════════════════");
    println!("4. 切换到 acting 阶段 ── remove plan, add act");
    println!("══════════════════════════════════════════");
    ctx.remove_condition("plan");
    ctx.set_condition("act");
    print_section(&mut ctx, "act+debug");

    println!("\n══════════════════════════════════════════");
    println!("5. 进入 review 阶段 ── render_with_condition(\"review\")");
    println!("══════════════════════════════════════════");
    let output = ctx.render_with_condition("review");
    println!("── [review] ──────────────────────────────────");
    println!("{output}");

    println!("\n══════════════════════════════════════════");
    println!("6. 外部事件触发 ── ctx.trigger(\"config_changed\")");
    println!("══════════════════════════════════════════");
    ctx.trigger("config_changed");
    print_section(&mut ctx, "config_triggered");

    println!("\n══════════════════════════════════════════");
    println!("7. 全部条件 ── plan + act + review + debug");
    println!("══════════════════════════════════════════");
    ctx.clear_conditions();
    ctx.set_condition("plan");
    ctx.set_condition("act");
    ctx.set_condition("review");
    print_section(&mut ctx, "all_conditions");
}

/// 构建一个多阶段工作流场景。
///
/// 组件树：
/// - header          — Always: 所有阶段都显示的通用头部
/// - plan_group      — When("plan"): 规划阶段的分组（含子组件）
/// - plan_steps      — When("plan"): 规划步骤列表
/// - tool_bash       — When("act"):  执行阶段的 Bash 工具
/// - tool_files      — When("act"):  执行阶段的文件工具
/// - tool_debug      — When("debug"): 调试模式下的诊断面板
/// - review_block    — When("review"): 审查阶段的结果展示
/// - config_notify   — OnTrigger("config_changed"): 配置变更通知
/// - status_slot     — Always: 运行时状态槽位
fn init_scene() -> cui::Context {
    Cui::init()
        .without_introduction()
        .component(
            TextBlock::new("header", "工作流引擎", "当前正在执行 Agent 工作流...\n- Phase: 运行时切换\n- Budget: 256K tokens")
                .build(),
        )
        .component(data_slot("status_slot", "运行时状态"))
        .component(
            group("plan_group", "规划阶段")
                .with_condition(VisibilityCondition::when("plan"))
                .push(
                    TextBlock::new("plan_overview", "规划概览", "分析用户意图并生成执行计划")
                        .with_condition(VisibilityCondition::when("plan"))
                        .build(),
                )
                .push(
                    TextBlock::new("plan_subtasks", "子任务", "- [ ] 分析代码结构\n- [ ] 定位相关文件\n- [ ] 生成修改方案")
                        .with_condition(VisibilityCondition::when("plan"))
                        .build(),
                )
                .build(),
        )
        .component(
            TextBlock::new("plan_steps", "执行步骤", "1. 读取目标文件\n2. 识别需要变更的位置\n3. 生成 diff\n4. 应用修改")
                .with_condition(VisibilityCondition::when("plan"))
                .build(),
        )
        .component(
            TextBlock::new("tool_bash", "bash", "```bash\necho \"executing...\"\n```")
                .with_condition(VisibilityCondition::when("act"))
                .build(),
        )
        .component(
            TextBlock::new("tool_files", "files", "```\nread: src/main.rs\nwrite: src/lib.rs\n```")
                .with_condition(VisibilityCondition::when("act"))
                .build(),
        )
        .component(
            TextBlock::new(
                "tool_debug",
                "诊断面板",
                "系统状态:\n- 内存: OK\n- 连接数: 3\n- 缓存命中率: 87%",
            )
            .with_condition(VisibilityCondition::when("debug"))
            .build(),
        )
        .component(
            TextBlock::new(
                "review_block",
                "审查结果",
                "变更摘要:\n- src/main.rs: +12 -3\n- src/lib.rs: +5 -2\n\n所有测试通过",
            )
            .with_condition(VisibilityCondition::when("review"))
            .build(),
        )
        .component(
            TextBlock::new(
                "config_notify",
                "配置已变更",
                "检测到工作流配置更新:\n- max_concurrency: 4 → 8\n- timeout: 30s → 60s",
            )
            .with_condition(VisibilityCondition::OnTrigger("config_changed".into()))
            .build(),
        )
        .build()
}

/// 渲染并打印，标注哪些组件可见/隐藏。
fn print_section(ctx: &mut cui::Context, label: &str) {
    let output = ctx.render();
    println!("── [{label}] ──────────────────────────────────");
    println!("{output}");

    print!("\n可见性: ");
    for node in ctx.tree().iter() {
        let mark = if node.level() < cui::RenderLevel::Title {
            "#"
        } else {
            "V"
        };
        print!("[{mark} {}] ", node.id());
    }
    println!("\n");
}

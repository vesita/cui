//! # CUI 数据输入示例
//!
//! 演示统一的数据输入模式：
//!
//! | 层面 | 声明 | 占位 | Rust API |
//! |------|------|------|----------|
//! | `.cui` 文件 | `inputs:` | `{{input:name}}` | — |
//! | Rust 代码 | — | — | `with_input(name, value)` |
//! | 运行时流式 | — | — | `DataSlot` + `write()` |
//!
//! ```bash
//! cargo run --example data_input
//! ```

use cui::condition::VisibilityCondition;
use cui::builtin::{CuiFileLeaf, data_slot};
use cui::data::DataMode;
use cui::Cui;

fn main() {
    let mut ctx = init_scene();

    println!("══════════════════════════════════════════");
    println!("1. plan 阶段 — 传入输入数据");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").render());

    println!("\n══════════════════════════════════════════");
    println!("2. act 阶段 — 传入文件列表与规则");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("act").render());

    println!("\n══════════════════════════════════════════");
    println!("3. review 阶段 + DataSlot 运行时追加");
    println!("══════════════════════════════════════════");
    ctx.write("runtime_log", DataMode::Append, "⏱ 耗时: 3.2s\n📊 覆盖: 85% 变更行");
    println!("{}", ctx.in_condition("review").render());

    println!("\n══════════════════════════════════════════");
    println!("4. 全阶段同时可见 (plan + act + review)");
    println!("══════════════════════════════════════════");
    println!(
        "{}",
        ctx.in_condition("plan")
            .and("act")
            .and("review")
            .render()
    );
}

fn init_scene() -> cui::Context {
    Cui::init()
        // ── plan 阶段：用 with_input_values() 批量注入 ──
        .component(
            CuiFileLeaf::new(
                "plan",
                "代码审查 - 计划",
                concat!(
                    "分析仓库变更范围，制定审查策略。\n",
                    "\n",
                    "**仓库**: {{input:repo}}\n",
                    "**基准分支**: {{input:base_branch}}\n",
                    "**目标分支**: {{input:target_branch}}\n",
                ),
            )
            .with_condition(VisibilityCondition::when("plan"))
            .with_input_values(&[
                ("repo", "/home/user/project"),
                ("base_branch", "main"),
                ("target_branch", "feat/login"),
            ])
            .build(),
        )
        // ── act 阶段：用 with_input() 逐个注入 ──
        .component(
            CuiFileLeaf::new(
                "act",
                "代码审查 - 执行",
                concat!(
                    "逐文件执行代码审查规则。\n",
                    "\n",
                    "**审查文件**:\n",
                    "{{input:files}}\n",
                    "\n",
                    "**审查规则**: {{input:rules}}\n",
                ),
            )
            .with_condition(VisibilityCondition::when("act"))
            .with_input("files", "- src/auth.rs  (+45 / -12)\n- src/db.rs   (+28 / -5)\n- tests/auth.rs (+62 新增)")
            .with_input("rules", "逻辑正确性, 错误处理, 安全审计")
            .build(),
        )
        // ── review 阶段：结果汇总 ──
        .component(
            CuiFileLeaf::new(
                "review",
                "代码审查 - 汇总",
                concat!(
                    "审查结果汇总。\n",
                    "\n",
                    "**高严重度**: {{input:high_issues}}\n",
                    "**中严重度**: {{input:medium_issues}}\n",
                    "**测试**: {{input:test_status}}\n",
                ),
            )
            .with_condition(VisibilityCondition::when("review"))
            .with_input("high_issues", "auth.rs:42 — token 未校验过期时间")
            .with_input("medium_issues", "db.rs:118 — 缺少连接超时配置")
            .with_input("test_status", "18/18 通过")
            .build(),
        )
        // ── DataSlot：运行时动态追加的数据通道 ──
        .component(data_slot("runtime_log", "运行时日志"))
        .build()
}

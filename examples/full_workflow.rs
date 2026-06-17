//! # CUI 完整流程示例
//!
//! 综合演示：目录加载 → 条件路由 → 工具注册 → 技能参考 → 数据注入 → 用户覆盖。
//!
//! ```bash
//! cargo run --example full_workflow
//! ```

use std::sync::Arc;
use cui::runtime::handler::{ActionContext, ActionHandler, ActionOutput, HandlerRegistry};
use cui::{Cui, PriorityLevel};

fn main() {
    // ═══════════════════════════════════════════════════════════
    // 1. 处理器
    // ═══════════════════════════════════════════════════════════
    let mut handlers = HandlerRegistry::new();
    handlers.register("tool.read_file", Arc::new(ReadFileHandler));
    handlers.register("tool.run_test", Arc::new(RunTestHandler));

    // ═══════════════════════════════════════════════════════════
    // 2. 构建：目录加载 + 工具 + 技能 + 处理器
    // ═══════════════════════════════════════════════════════════
    let mut ctx = Cui::init()
        .without_introduction()
        .load_dir("examples/cui")
        .tools("tools", "可用工具", PriorityLevel::High, (
            "examples/cui/tools/read_file.cui",
            "examples/cui/tools/run_test.cui",
        ))
        .skills("skills", "技能参考", (
            ("Rust 审查", "检查 unsafe 块、错误处理、性能瓶颈"),
            ("安全审计", "扫描 SQL 注入、XSS、CSRF 漏洞"),
        ))
        .handlers(&handlers)
        .build();

    // ═══════════════════════════════════════════════════════════
    // 3. 运行时数据注入
    // ═══════════════════════════════════════════════════════════
    ctx.write("act_bash", cui::DataMode::Append, "> 上次: `cargo build` — 0 errors");

    // ═══════════════════════════════════════════════════════════
    // 4. 阶段渲染
    // ═══════════════════════════════════════════════════════════
    println!("══════════════ plan 阶段 ══════════════");
    println!("{}", ctx.in_condition("plan").render());

    println!("\n══════════════ act 阶段 ══════════════");
    println!("{}", ctx.in_condition("act").render());

    println!("\n══════════════ review 阶段 ══════════════");
    println!("{}", ctx.in_condition("review").render());

    println!("\n══════════════ 全阶段 (plan+act+review) ══════════════");
    println!("{}", ctx.in_condition("plan").and("act").and("review").render());

    println!("\n══════════════ 低预算（技能优先降级，工具保底） ══════════════");
    println!("{}", ctx.with_budget(300).render_volatile());
}

struct ReadFileHandler;
impl ActionHandler for ReadFileHandler {
    fn id(&self) -> &str { "tool.read_file" }
    fn execute(&self, _params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
        Ok(ActionOutput::success("src/main.rs: 42 行"))
    }
}

struct RunTestHandler;
impl ActionHandler for RunTestHandler {
    fn id(&self) -> &str { "tool.run_test" }
    fn execute(&self, _params: &str, _ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
        Ok(ActionOutput::success("18/18 通过"))
    }
}

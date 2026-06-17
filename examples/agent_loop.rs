//! # Agent 主循环 —— 模拟 LLM Agent 与 CUI 交互的完整流程
//!
//! 演示运行时核心模式：
//! 1. 定义 .cui 模板（plan / act / review 阶段）
//! 2. 注册工具 handler（读写组件数据、发出事件）
//! 3. 推送对话消息（模拟 LLM 对话轮次）
//! 4. 模拟 component_action 调用（LLM 操作组件）
//! 5. 观察 token 预算压力下的降级行为
//! 6. CacheOptimized 排序（Claude 前缀缓存友好）
//! 7. 持久化快照
//!
//! ```bash
//! cargo run --example agent_loop
//! ```

use std::sync::Arc;
use cui::action::ActionRequest;
use cui::{ActionContext, ActionHandler, ActionOutput, HandlerRegistry};
use cui::{Cui, DataMode, OrderingStrategy, PriorityLevel};

fn main() {
    let mut handlers = HandlerRegistry::new();
    handlers.register("tool.read_file", Arc::new(ReadFileHandler));
    handlers.register("tool.run_test", Arc::new(RunTestHandler));

    let mut ctx = Cui::init()
        .load_dir("examples/cui")
        .tools("tools", "可用工具", PriorityLevel::High, (
            "examples/cui/tools/read_file.cui",
            "examples/cui/tools/run_test.cui",
        ))
        .handlers(&handlers)
        .build();

    // ── 1. Plan 阶段 ──────────────────────────────

    println!("══════ Plan 阶段 ══════");
    ctx.dialogue_mut().push(r#"{"role":"user","content":"帮我审查这个 Rust 项目"}"#);
    let plan = ctx.in_condition("plan").render();
    println!("{}", &plan[..plan.len().min(800)]);
    println!("  ... (共 {} 字符)\n", plan.len());

    // ── 2. Act 阶段：写入工具数据 ─────────────────

    println!("\n══════ Act 阶段：写入工具数据 ══════");
    ctx.dialogue_mut().push(r#"{"role":"assistant","content":"正在读取代码...","tool_calls":[{"id":"1","function":{"name":"component_action","arguments":"{\"id\":\"tool.read_file\",\"action\":\"expand\"}"}}]}"#);

    // handler 通过 ActionContext::write 注入数据
    ctx.write("tool.read_file", DataMode::Append, "读取: src/main.rs (42 行)\n  0 错误, 3 警告\n");
    ctx.write("tool.run_test", DataMode::Append, "运行: cargo test\n  结果: 18/18 通过\n");

    // 渲染 act 阶段（act 条件组件可见）
    println!("{}", ctx.in_condition("act").render());

    // ── 3. LLM 操作组件：expand 展开详情 ──────────

    println!("\n══════ LLM 操作：展开 tool.read_file ══════");
    let req = ActionRequest::new("tool.read_file", "expand");
    let result = ctx.component_action(&req);
    println!("  结果: {}", result.message().unwrap_or("ok"));

    // ── 4. Review 阶段 ────────────────────────────

    println!("\n══════ Review 阶段 ══════");
    ctx.dialogue_mut().push(r#"{"role":"tool","tool_call_id":"1","content":"代码读取完成"}"#);
    ctx.dialogue_mut().push(r#"{"role":"assistant","content":"审查完成：代码质量良好，测试全部通过"}"#);
    println!("{}", ctx.in_condition("review").render());
    println!("  对话消息数: {}", ctx.dialogue_mut().read().len());

    // ── 5. 预算压力 ──────────────────────────────

    println!("\n══════ 预算压力: 150 tokens ══════");
    let rendered = ctx.with_budget(150).render_volatile();
    println!("{}", rendered);

    if let Some(stats) = ctx.last_render_stats() {
        println!("  预算使用率: {:.0}% ({}/{} tokens), {} 组件 {} 隐藏",
            stats.usage_pct * 100.0, stats.total_estimated,
            stats.budget, stats.component_count, stats.hidden_count);
    }

    // ── 6. CacheOptimized 排序 ────────────────────

    println!("\n══════ CacheOptimized 排序 ══════");
    ctx.set_ordering(OrderingStrategy::CacheOptimized);
    let out = ctx.in_condition("plan").and("review").render();
    println!("{}", &out[..out.len().min(200)]);
    println!("  ... (共 {} 字符)", out.len());

    // ── 7. 持久化 ─────────────────────────────────

    println!("\n══════ 持久化快照 ══════");
    for (cid, data) in ctx.persistence().collect() {
        println!("  {cid}: {} 条", data.len());
    }
    println!("  （可序列化保存，进程重启时恢复）");
}

// ── Handler 实现 ──────────────────────────────────

struct ReadFileHandler;
impl ActionHandler for ReadFileHandler {
    fn id(&self) -> &str { "tool.read_file" }
    fn execute(&self, _params: &str, actx: &mut dyn ActionContext) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
        actx.write("tool.read_file", DataMode::Append, "读取代码完成: 42 行, 0 错误\n");
        Ok(ActionOutput::success("已读取代码").with_event("data_changed", "read_file"))
    }
}

struct RunTestHandler;
impl ActionHandler for RunTestHandler {
    fn id(&self) -> &str { "tool.run_test" }
    fn execute(&self, _params: &str, actx: &mut dyn ActionContext) -> Result<ActionOutput, Box<dyn std::error::Error + Send + Sync>> {
        actx.write("tool.run_test", DataMode::Append, "cargo test: 18/18 通过\n");
        Ok(ActionOutput::success("测试通过").with_event("data_changed", "run_test"))
    }
}

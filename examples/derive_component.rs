//! # CuiComponent derive 宏使用指南
//!
//! 演示 `#[derive(CuiComponent)]` 的全部 7 个属性，覆盖 6 种常见场景。
//!
//! ```bash
//! cargo run --example derive_component
//! ```

use cui::component::ComponentNode;
use cui::condition::VisibilityCondition;
use cui::data::DataMode;
use cui::Cui;
use cui::{CuiComponent, RenderLevel};

// ── 场景 1: 最简模式 — 常量 id / title / priority ──────────

#[derive(CuiComponent)]
#[cui(id = "status", title = "状态", priority = "high")]
struct StatusComponent;

// ── 场景 2: 字段代理 — id_field / title_field ──────────────

#[derive(CuiComponent)]
#[cui(id_field = "name", title_field = "label", priority = "high")]
struct TaskComponent {
    name: String,
    label: String,
}

// ── 场景 3: write + render_from — 驱动型数据组件 ────────────

#[derive(CuiComponent)]
#[cui(
    id_field = "key",
    title_field = "caption",
    priority = "normal",
    write,
    write_field = "body",
    render_from = "body"
)]
struct LogComponent {
    key: String,
    caption: String,
    body: String,
}

// ── 场景 4: kind + inert + is_static + render_from — 静态参考 ──

#[derive(CuiComponent)]
#[cui(
    id = "rules",
    title = "审查规则",
    kind = "block",
    inert,
    is_static,
    render_from = "body"
)]
struct RulesComponent {
    body: String,
}

// ── 场景 5: visibility_field — 条件可见 ─────────────────────

#[derive(CuiComponent)]
#[cui(
    id = "env_info",
    title = "环境信息",
    priority = "low",
    write,
    render_from = "content",
    visibility_field = "condition"
)]
struct EnvSection {
    content: String,
    condition: VisibilityCondition,
}

// ── 场景 6: 全属性组合 — 真实业务组件 ────────────────────────

#[derive(CuiComponent)]
#[cui(
    id_field = "id",
    title_field = "title",
    priority = "critical",
    kind = "block",
    write,
    write_field = "data",
    render_from = "data",
    is_static,
    visibility_field = "cond"
)]
struct ResultSection {
    id: String,
    title: String,
    data: String,
    cond: VisibilityCondition,
}

fn main() {
    println!("══════════════════════════════════════════");
    println!("1. 最简模式 — #[cui(id, title, priority)]");
    println!("══════════════════════════════════════════");
    let s = StatusComponent;
    println!("  id={}  title={}  priority={:?}\n", s.id(), s.title(), s.priority());

    println!("══════════════════════════════════════════");
    println!("2. 字段代理 — #[cui(id_field, title_field)]");
    println!("══════════════════════════════════════════");
    let t = TaskComponent { name: "build".into(), label: "构建任务".into() };
    println!("  id={}  title={}\n", t.id(), t.title());

    println!("══════════════════════════════════════════");
    println!("3. write + render_from — 数据注入与渲染");
    println!("══════════════════════════════════════════");
    let mut log = LogComponent {
        key: "build_log".into(),
        caption: "构建日志".into(),
        body: String::new(),
    };
    log.write(DataMode::Append, "[INFO] 编译开始\n");
    log.write(DataMode::Append, "[INFO] 检查依赖\n");
    log.write(DataMode::Append, "[OK]   构建成功 (2.3s)");
    println!("  Summary:\n  {}", log.render(RenderLevel::Summary));
    println!("  Standard:\n{}", indent(&log.render(RenderLevel::Standard)));

    println!("══════════════════════════════════════════");
    println!("4. kind + inert + is_static + render_from");
    println!("══════════════════════════════════════════");
    let rules = RulesComponent {
        body: "1. 检查 unsafe 块\n2. 验证错误处理\n3. 审计外部输入".into(),
    };
    println!("  kind={:?}  inert={}  is_static={}",
        rules.kind(), rules.is_inert(), rules.is_static());
    println!("  render:\n{}", indent(&rules.render(RenderLevel::Standard)));

    println!("══════════════════════════════════════════");
    println!("5. visibility_field — 条件可见");
    println!("══════════════════════════════════════════");
    let env = EnvSection {
        content: "OS: Linux  Shell: bash  Git: 2.45.0".into(),
        condition: VisibilityCondition::when("act"),
    };
    let vis = env.visibility_condition();
    println!("  visible: {}", if vis == VisibilityCondition::Always { "always" } else { "when(\"act\")" });
    println!("  render:\n{}", indent(&env.render(RenderLevel::Standard)));

    println!("══════════════════════════════════════════");
    println!("6. 全属性组合 — Context 中实际渲染");
    println!("══════════════════════════════════════════");

    let mut ctx = build_context();

    println!("--- plan 阶段 (仅 plan_log 可见) ---");
    println!("{}", ctx.in_condition("plan").render());

    println!("\n--- act 阶段 (env_info 变为可见) ---");
    println!("{}", ctx.in_condition("act").render());
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n")
}

fn build_context() -> cui::Context {
    let mut result = ResultSection {
        id: "results".into(),
        title: "执行结果".into(),
        data: String::new(),
        cond: VisibilityCondition::Always,
    };
    result.write(DataMode::Append, "[read]  src/main.rs    — 42 行\n");
    result.write(DataMode::Append, "[bash]  cargo build   — 0 errors\n");
    result.write(DataMode::Append, "[test]  18/18 通过");

    let mut plan_log = LogComponent {
        key: "plan_log".into(),
        caption: "计划日志".into(),
        body: String::new(),
    };
    plan_log.write(DataMode::Append, "步骤1: 分析变更范围\n");
    plan_log.write(DataMode::Append, "步骤2: 制定审查策略");

    let act_env = EnvSection {
        content: "OS: Linux  |  Shell: bash  |  Git: 2.45.0".into(),
        condition: VisibilityCondition::when("act"),
    };

    Cui::init()
        .component(ComponentNode::leaf(plan_log))
        .component(ComponentNode::leaf(act_env))
        .component(ComponentNode::leaf(result))
        .build()
}

//! # 用户覆盖与固定示例
//!
//! 演示用户如何通过 `~/.cui/user/` 目录自定义组件：
//! - 覆盖 `inputs` 默认值
//! - 用 `pinned: true` 固定组件，使其不被预算降级
//!
//! ```bash
//! cargo run --example user_override
//! ```

use cui::Cui;
use std::fs;
use std::io::Write;

fn main() {
    let tmp = std::env::temp_dir().join("cui_example_user_override");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // ── 开发者提供的基础组件 ──
    let base_dir = tmp.join("cui");
    fs::create_dir_all(&base_dir).unwrap();
    write_file(&base_dir.join("header.cui"), concat!(
        "---\n",
        "id: header\n",
        "title: 工作台\n",
        "priority: critical\n",
        "inert: true\n",
        "---\n",
        "当前会话状态面板。\n",
    ));
    write_file(&base_dir.join("plan.cui"), concat!(
        "---\n",
        "id: plan\n",
        "title: 规划\n",
        "when: plan\n",
        "inputs:\n",
        "  - {name: scope, default_value: \"全项目\"}\n",
        "---\n",
        "分析 {{input:scope}} 范围内的变更。\n",
    ));
    write_file(&base_dir.join("review.cui"), concat!(
        "---\n",
        "id: review\n",
        "title: 审查\n",
        "when: review\n",
        "priority: low\n",
        "---\n",
        "代码审查结果汇总。\n",
    ));

    // ── 用户覆盖目录 ──
    let user_dir = tmp.join("user");
    fs::create_dir_all(&user_dir).unwrap();
    write_file(&user_dir.join("review.cui"), concat!(
        "---\n",
        "id: review\n",
        "title: 审查\n",
        "priority: low\n",
        "pinned: true\n",
        "---\n",
        "代码审查结果汇总。\n",
    ));

    // ── 基础渲染 ──
    let mut ctx = Cui::init()
        .without_introduction()
        .load_dir(&base_dir)
        .build();

    println!("══════════════════════════════════════════");
    println!("基础渲染（无用户覆盖，审查组件正常参与预算）");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.render());

    println!("\n══════════════════════════════════════════");
    println!("低预算渲染（审查组件优先被降级）");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.with_budget(100).render_volatile());

    // ── 带用户覆盖 ──
    let mut ctx2 = Cui::init()
        .without_introduction()
        .load_dir(&base_dir)
        .with_user_overrides_from(&user_dir)
        .build();

    println!("\n══════════════════════════════════════════");
    println!("用户覆盖 — 正常预算（plan + review 可见）");
    println!("══════════════════════════════════════════");
    println!("{}", ctx2.render());

    println!("\n══════════════════════════════════════════");
    println!("低预算（无 pin 保护时 plan 被降级）");
    println!("══════════════════════════════════════════");
    let out = Cui::init()
        .without_introduction()
        .load_dir(&base_dir)
        .build()
        .with_budget(50)
        .render_volatile();
    println!("{}", out);

    println!("\n══════════════════════════════════════════");
    println!("低预算（review 有 pin 保护 = 存活，plan 被降级）");
    println!("══════════════════════════════════════════");
    println!("{}", ctx2.with_budget(50).render_volatile());

    let _ = fs::remove_dir_all(&tmp);
}

fn write_file(path: &std::path::Path, content: &str) {
    let mut f = fs::File::create(path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

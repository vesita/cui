//! # .cui 文件条件渲染
//!
//! 用 `load_dir()` 一次性加载整个目录的 `.cui` 文件。
//! 配合 `in_condition()` + `with_budget()` 实现声明式阶段渲染。
//!
//! ```bash
//! cargo run --example cui_file_conditional
//! ```

use cui::Cui;

fn main() {
    let mut ctx = Cui::init()
        .without_introduction()
        .load_dir("examples/cui")
        .build();

    println!("══════════════════════════════════════════");
    println!("1. render() — 无任何条件，仅 Always 可见");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.render());

    println!("\n══════════════════════════════════════════");
    println!("2. in_condition(\"plan\").render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").render());

    println!("\n══════════════════════════════════════════");
    println!("3. in_condition(\"act\").render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("act").render());

    println!("\n══════════════════════════════════════════");
    println!("4. in_condition(\"plan\").and(\"act\").render() — OR");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("plan").and("act").render());

    println!("\n══════════════════════════════════════════");
    println!("5. in_condition(\"act\").with_budget(800).render()");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.in_condition("act").with_budget(800).render());

    println!("\n══════════════════════════════════════════");
    println!("6. with_budget(50000).render_volatile() — 无副作用预览");
    println!("══════════════════════════════════════════");
    println!("{}", ctx.with_budget(50000).render_volatile());

    println!("\n══════════════════════════════════════════");
    println!("7. 运行时写入 + 条件渲染");
    println!("══════════════════════════════════════════");
    ctx.write("act_bash", cui::DataMode::Append, "\n\n**上次执行**: `cargo test` → 通过");
    println!("{}", ctx.in_condition("act").render());
}

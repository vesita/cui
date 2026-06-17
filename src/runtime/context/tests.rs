use super::*;
use crate::component::builtin::{data_slot, text_block};
use crate::manage::ManageEvent;

#[test]
fn new_context_empty() {
    let ctx = Context::new();
    assert!(ctx.dialogue().read().is_empty());
}

#[test]
fn register_and_render() {
    let mut ctx = Context::new();
    ctx.register(text_block("hi", "问候", "你好世界"));
    let output = ctx.render();
    assert!(output.contains("[问候]"));
    assert!(output.contains("你好世界"));
}

#[test]
fn register_all() {
    let mut ctx = Context::new();
    ctx.register_all(vec![text_block("a", "A", "aaa"), text_block("b", "B", "bbb")]);
    assert_eq!(ctx.tree().len(), 2);
}

#[test]
fn remove_component() {
    let mut ctx = Context::new();
    ctx.register(text_block("a", "A", "aaa"));
    ctx.register(text_block("b", "B", "bbb"));
    assert!(ctx.remove("a").is_some());
    assert_eq!(ctx.tree().len(), 1);
    assert!(ctx.remove("nonexistent").is_none());
}

#[test]
fn clear_all() {
    let mut ctx = Context::new();
    ctx.register(text_block("a", "A", "aaa"));
    ctx.clear();
    assert!(ctx.tree().is_empty());
}

#[test]
fn render_with_condition() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "content"));
    let output = ctx.in_condition("act").render();
    assert!(output.contains("[C]"));
}

#[test]
fn render_works() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "long content here that takes up space"));
    let output = ctx.render();
    assert!(!output.is_empty());
}

#[test]
fn write_and_read() {
    let mut ctx = Context::new();
    ctx.register(data_slot("state", "状态"));
    assert!(ctx.write("state", DataMode::Overwrite, "running"));
    let output = ctx.render();
    assert!(output.contains("running"));
    let read_back = ctx.read("state");
    assert!(!read_back.is_empty());
}

#[test]
fn write_nonexistent() {
    let mut ctx = Context::new();
    assert!(!ctx.write("nonexistent", DataMode::Overwrite, "data"));
}

#[test]
fn read_nonexistent() {
    let ctx = Context::new();
    assert_eq!(ctx.read("nonexistent"), "");
}

#[test]
fn read_by_label_prefix() {
    let mut ctx = Context::new();
    ctx.register_all(vec![
        data_slot("tool:git", "Git"),
        data_slot("tool:node", "Node"),
        text_block("other", "其他", "xxx"),
    ]);
    ctx.write("tool:git", DataMode::Overwrite, "git status");
    ctx.write("tool:node", DataMode::Overwrite, "node version");
    let result = ctx.read_by_label_prefix("tool:");
    assert!(result.contains("Git"));
    assert!(result.contains("Node"));
    assert!(!result.contains("其他"));
}

#[test]
fn trigger_and_on_event() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "content"));
    ctx.trigger("event1");
    ctx.on_event(ManageEvent::StepStart);
}

#[test]
fn start_new_cycle() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "content"));
    ctx.start_new_cycle(1);
}

#[test]
fn compress() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "content"));
    assert!(!ctx.compress());
}

#[test]
fn push_and_read_messages() {
    let mut ctx = Context::new();
    ctx.dialogue_mut().push(r#"{"role":"user"}"#);
    ctx.dialogue_mut().push(r#"{"role":"assistant"}"#);
    assert_eq!(ctx.dialogue_mut().read().len(), 2);
    assert!(ctx.dialogue_mut().read()[0].contains("user"));
}

#[test]
fn component_action_unknown() {
    let mut ctx = Context::new();
    let req =
        ActionRequest { component_id: "unknown".into(), action: "expand".into(), params: None };
    let result = ctx.component_action(&req);
    assert!(!result.is_success());
}

#[test]
fn component_action_on_component() {
    let mut ctx = Context::new();
    ctx.register(text_block("t", "T", "content"));
    let req = ActionRequest { component_id: "t".into(), action: "expand".into(), params: None };
    let result = ctx.component_action(&req);
    assert!(!result.is_success());
}

#[test]
fn overview_expand_action() {
    let mut ctx = Context::new();
    ctx.register(text_block("target", "目标", "content"));
    let req = ActionRequest {
        component_id: "_overview".into(),
        action: "expand:target".into(),
        params: None,
    };
    let result = ctx.component_action(&req);
    assert!(!result.is_success());
}

#[test]
fn overview_temp_expand_action() {
    let mut ctx = Context::new();
    ctx.register(text_block("target", "目标", "content"));
    let req = ActionRequest {
        component_id: "_overview".into(),
        action: "temp_expand:target".into(),
        params: None,
    };
    let result = ctx.component_action(&req);
    assert!(!result.is_success());
    assert!(ctx.tree().temp_expand_info(ctx.tick()).is_some());
    assert_eq!(ctx.tree().temp_expand_info(ctx.tick()).unwrap().1, 3);
}

#[test]
fn overview_expand_on_group() {
    use crate::component::builtin::group;
    let mut ctx = Context::new();
    ctx.register(group("g", "分组").build());
    let req = ActionRequest {
        component_id: "_overview".into(),
        action: "expand_group:g".into(),
        params: None,
    };
    let result = ctx.component_action(&req);
    assert!(result.is_success());
}

#[test]
fn overview_expand_hidden_action() {
    let mut ctx = Context::new();
    let req = ActionRequest {
        component_id: "_overview".into(),
        action: "expand_hidden".into(),
        params: None,
    };
    let result = ctx.component_action(&req);
    assert!(result.is_success());
}

#[test]
fn overview_unknown_action() {
    let mut ctx = Context::new();
    let req = ActionRequest {
        component_id: "_overview".into(),
        action: "nonexistent".into(),
        params: None,
    };
    let result = ctx.component_action(&req);
    assert!(!result.is_success());
}

#[test]
fn tree_access() {
    let mut ctx = Context::new();
    ctx.register(text_block("a", "A", "aaa"));
    ctx.register(text_block("b", "B", "bbb"));
    assert_eq!(ctx.tree().len(), 2);
    ctx.tree_mut().remove("a");
    assert_eq!(ctx.tree().len(), 1);
}

#[test]
fn merge_params_empty() {
    let result = merge_action_params(None, None);
    assert_eq!(result, "");
}

#[test]
fn merge_params_preset_only() {
    let mut preset = std::collections::HashMap::new();
    preset.insert("key1".into(), "val1".into());
    let result = merge_action_params(Some(&preset), None);
    assert!(result.contains("key1"));
    assert!(result.contains("val1"));
}

#[test]
fn merge_params_request_overrides() {
    let mut preset = std::collections::HashMap::new();
    preset.insert("command".into(), "echo default".into());
    let result = merge_action_params(Some(&preset), Some(r#"{"command":"echo override"}"#));
    assert!(result.contains("echo override"));
    assert!(!result.contains("echo default"));
}

#[test]
fn merge_params_non_json_request() {
    let mut preset = std::collections::HashMap::new();
    preset.insert("a".into(), "1".into());
    let result = merge_action_params(Some(&preset), Some("plain text"));
    // invalid JSON is dropped; preset params still apply
    assert!(result.contains("a"));
    assert!(result.contains("1"));
}

#[test]
fn collect_persistable_empty() {
    let ctx = Context::new();
    assert!(ctx.persistence().collect().is_empty());
}

#[test]
fn toast_shows_and_auto_dismisses() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    ctx.toast("t", "文件已保存");
    let output = ctx.render();
    assert!(output.contains("文件已保存"));
    assert!(ctx.tree().temp_expand_info(ctx.tick()).is_some());
}

#[test]
fn toast_action_show() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    let req = ActionRequest {
        component_id: "t".into(),
        action: "show".into(),
        params: Some("操作完成".into()),
    };
    let result = ctx.component_action(&req);
    assert!(result.is_success());
    assert!(ctx.tree().temp_expand_info(ctx.tick()).is_some());
}

#[test]
fn toast_action_dismiss() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    let _ = ctx.component_action(&ActionRequest {
        component_id: "t".into(),
        action: "show".into(),
        params: Some("待关闭".into()),
    });
    let req = ActionRequest { component_id: "t".into(), action: "dismiss".into(), params: None };
    let result = ctx.component_action(&req);
    assert!(result.is_success());
}

#[test]
fn toast_temp_expand_decrements_on_render() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    ctx.toast("t", "hello");
    let (_, rem) = ctx.tree().temp_expand_info(ctx.tick()).unwrap();
    assert_eq!(rem, 3);
    ctx.render();
    let info = ctx.tree().temp_expand_info(ctx.tick());
    assert!(info.is_some());
    assert_eq!(info.unwrap().1, 2);
}

#[test]
fn in_condition_single_shows_matching() {
    use crate::condition::VisibilityCondition;
    use crate::component::builtin::TextBlock;
    let mut ctx = Context::new();
    ctx.register(
        TextBlock::new("header", "头部", "always visible").build(),
    );
    ctx.register(
        TextBlock::new("plan", "计划", "plan content")
            .with_condition(VisibilityCondition::when("plan"))
            .build(),
    );
    ctx.register(
        TextBlock::new("act", "执行", "act content")
            .with_condition(VisibilityCondition::when("act"))
            .build(),
    );

    let output = ctx.in_condition("plan").render();
    assert!(output.contains("[头部]"), "Always should render");
    assert!(output.contains("[计划]"), "plan should render with plan condition");
    assert!(!output.contains("[执行]"), "act should NOT render with plan condition");
}

#[test]
fn in_condition_and_shows_or_logic() {
    use crate::condition::VisibilityCondition;
    use crate::component::builtin::TextBlock;
    let mut ctx = Context::new();
    ctx.register(
        TextBlock::new("header", "头部", "always visible").build(),
    );
    ctx.register(
        TextBlock::new("plan", "计划", "plan")
            .with_condition(VisibilityCondition::when("plan"))
            .build(),
    );
    ctx.register(
        TextBlock::new("act", "执行", "act")
            .with_condition(VisibilityCondition::when("act"))
            .build(),
    );

    let output = ctx.in_condition("plan").and("act").render();
    assert!(output.contains("[头部]"));
    assert!(output.contains("[计划]"));
    assert!(output.contains("[执行]"));
}

#[test]
fn in_condition_clears_after_render() {
    use crate::condition::VisibilityCondition;
    use crate::component::builtin::TextBlock;
    let mut ctx = Context::new();
    ctx.register(
        TextBlock::new("plan", "计划", "plan")
            .with_condition(VisibilityCondition::when("plan"))
            .build(),
    );

    // 通过 tree 直接设置一个持久条件，模拟外部已有条件
    ctx.tree_mut().add_condition("persistent");
    let output = ctx.in_condition("plan").render();
    assert!(output.contains("[计划]"));
    assert!(ctx.tree().has_condition("persistent"), "persistent condition should survive");
    assert!(!ctx.tree().has_condition("plan"), "in_condition should be cleared after render");
}

#[test]
fn render_without_condition_hides_when_components() {
    use crate::condition::VisibilityCondition;
    use crate::component::builtin::TextBlock;
    let mut ctx = Context::new();
    ctx.register(
        TextBlock::new("header", "头部", "always visible").build(),
    );
    ctx.register(
        TextBlock::new("plan", "计划", "plan")
            .with_condition(VisibilityCondition::when("plan"))
            .build(),
    );

    let output = ctx.render();
    assert!(output.contains("[头部]"), "Always should render");
    assert!(!output.contains("plan content"), "when:plan should be hidden without condition");
}

#[test]
fn tick_does_not_advance_on_volatile_render() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    ctx.toast("t", "hello");
    let tick_before = ctx.tick();
    assert_eq!(tick_before, 0);
    ctx.with_budget(99999).render_volatile();
    assert_eq!(ctx.tick(), 0);
    let (_, rem) = ctx.tree().temp_expand_info(ctx.tick()).unwrap();
    assert_eq!(rem, 3);
    ctx.render();
    assert_eq!(ctx.tick(), 1);
}

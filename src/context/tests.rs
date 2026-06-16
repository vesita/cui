use super::*;
use crate::component::builtin::{data_slot, text_block};
use crate::manage::ManageEvent;

#[test]
fn new_context_empty() {
    let ctx = Context::new();
    assert!(ctx.read_messages().is_empty());
}

#[test]
fn register_and_render() {
    let mut ctx = Context::new();
    ctx.register(text_block("hi", "问候", "你好世界"));
    let output = ctx.render();
    assert!(output.contains("[hi]"));
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
    let output = ctx.render_with_condition("act");
    assert!(output.contains("[c]"));
}

#[test]
fn render_with_budget() {
    let mut ctx = Context::new();
    ctx.register(text_block("c", "C", "long content here that takes up space"));
    let output = ctx.render_with_budget(200);
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
    ctx.push_message(r#"{"role":"user"}"#);
    ctx.push_message(r#"{"role":"assistant"}"#);
    assert_eq!(ctx.read_messages().len(), 2);
    assert!(ctx.read_messages()[0].contains("user"));
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
    assert!(ctx.collect_persistable().is_empty());
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
fn tick_does_not_advance_on_volatile_render() {
    use crate::component::builtin::toast;
    let mut ctx = Context::new();
    ctx.register(toast("t"));
    ctx.toast("t", "hello");
    let tick_before = ctx.tick();
    assert_eq!(tick_before, 0);
    ctx.render_volatile_with_budget(99999);
    assert_eq!(ctx.tick(), 0);
    let (_, rem) = ctx.tree().temp_expand_info(ctx.tick()).unwrap();
    assert_eq!(rem, 3);
    ctx.render();
    assert_eq!(ctx.tick(), 1);
}

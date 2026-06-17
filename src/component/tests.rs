use super::*;
use crate::action::ActionResult;
use crate::data::DataMode;
use crate::keyword::PriorityLevel;
use crate::level::RenderLevel;
use crate::runtime::schedule;
use crate::runtime::test_utils::MockComponent;

// ── ComponentNode 测试 ────────────────────────────────

#[test]
fn leaf_creation() {
    let node = ComponentNode::leaf(MockComponent::new("test", "测试"));
    assert_eq!(node.id(), "test");
    assert_eq!(node.title(), "测试");
    assert_eq!(node.priority(), PriorityLevel::Normal);
    assert_eq!(node.level(), RenderLevel::Standard);
}

#[test]
fn composite_creation() {
    let child = ComponentNode::leaf(MockComponent::new("child", "子组件"));
    let node = ComponentNode::composite(MockComponent::new("parent", "父组件"), vec![child]);
    assert_eq!(node.id(), "parent");
    assert_eq!(node.title(), "父组件");
    assert_eq!(node.level(), RenderLevel::Standard);
}

#[test]
fn composite_level_derived_from_children() {
    let mut node = ComponentNode::composite(MockComponent::new("p", "p"), vec![]);
    assert_eq!(node.level(), RenderLevel::Standard);
    node.set_level(RenderLevel::Hidden);
    assert_eq!(node.level(), RenderLevel::Hidden);

    let node2 = ComponentNode::composite(
        MockComponent::new("p2", "p2"),
        vec![ComponentNode::leaf(MockComponent::new("c", "c"))],
    );
    assert_eq!(node2.level(), RenderLevel::Standard);
}

#[test]
fn set_level_leaf_only() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    assert_eq!(node.level(), RenderLevel::Standard);
    node.set_level(RenderLevel::Summary);
    assert_eq!(node.level(), RenderLevel::Summary);
    node.set_level(RenderLevel::Hidden);
    assert_eq!(node.level(), RenderLevel::Hidden);
}

#[test]
fn mark_dirty_leaf() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    match &node {
        ComponentNode::Leaf(info) => assert!(!info.signal.interactive),
        _ => panic!("not leaf"),
    }
    node.mark_dirty();
    assert!(node.is_dirty());
}

#[test]
fn find_in_leaf() {
    let node = ComponentNode::leaf(MockComponent::new("target", "目标"));
    assert!(node.find("target").is_some());
    assert!(node.find("nonexistent").is_none());
}

#[test]
fn find_in_composite() {
    let child = ComponentNode::leaf(MockComponent::new("child", "子"));
    let parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    assert!(parent.find("parent").is_some());
    assert!(parent.find("child").is_some());
    assert!(parent.find("nonexistent").is_none());
}

#[test]
fn find_nested_composite() {
    let grandchild = ComponentNode::leaf(MockComponent::new("gc", "孙"));
    let child = ComponentNode::composite(MockComponent::new("c", "子"), vec![grandchild]);
    let parent = ComponentNode::composite(MockComponent::new("p", "父"), vec![child]);
    assert!(parent.find("gc").is_some());
    assert!(parent.find("c").is_some());
    assert!(parent.find("p").is_some());
}

#[test]
fn find_mut_leaf() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    assert!(node.find_mut("t").is_some());
    assert!(node.find_mut("x").is_none());
}

#[test]
fn find_mut_modify_level() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    node.find_mut("t").unwrap().set_level(RenderLevel::Summary);
    assert_eq!(node.level(), RenderLevel::Summary);
}

#[test]
fn has_child() {
    let child = ComponentNode::leaf(MockComponent::new("c", "C"));
    let parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    assert!(parent.has_child("c"));
    assert!(!parent.has_child("x"));
    assert!(parent.has_child("p"));
}

#[test]
fn actions_empty_when_no_variants() {
    let node = ComponentNode::leaf(MockComponent::new("t", "T").with_content(""));
    let actions = node.actions(RenderLevel::Standard);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id(), "expand");
}

#[test]
fn actions_with_level_filter() {
    let node = ComponentNode::leaf(MockComponent::new("t", "T"));
    let actions = node.actions(RenderLevel::Title);
    assert_eq!(actions.len(), 2);
}

#[test]
fn leaf_handle_action() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    let result = node.handle_action("expand", "");
    assert!(result.is_success());
    assert_eq!(node.level(), RenderLevel::Detailed);
}

#[test]
fn leaf_handle_action_renders_snapshot() {
    let mut node =
        ComponentNode::leaf(MockComponent::new("t", "T").with_content("snapshot content"));
    let result = node.handle_action("expand", "");
    assert!(result.snapshot().is_some());
    let snap = result.snapshot().unwrap();
    assert!(snap.contains("snapshot content"));
}

#[test]
fn composite_handle_action_delegates_to_child() {
    let child = ComponentNode::leaf(
        MockComponent::new("child", "子").with_action_result(ActionResult::new("child", "child")),
    );
    let mut parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    let result = parent.handle_action("child", "");
    assert!(result.is_success());
    assert_eq!(result.component_id(), "child");
}

#[test]
fn composite_handle_action_delegates_to_grandchild() {
    let gc = ComponentNode::leaf(
        MockComponent::new("gc", "孙").with_action_result(ActionResult::new("gc", "gc")),
    );
    let child = ComponentNode::composite(MockComponent::new("c", "子"), vec![gc]);
    let mut parent = ComponentNode::composite(MockComponent::new("p", "父"), vec![child]);
    let result = parent.handle_action("gc", "");
    assert!(result.is_success());
    assert_eq!(result.component_id(), "gc");
}

#[test]
fn handle_action_unknown_error() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    let result = node.handle_action("nonexistent", "");
    assert!(!result.is_success());
}

#[test]
fn write_to_leaf_calls_component_and_hooks() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    node.write(DataMode::Overwrite, "new data");
    match &node {
        ComponentNode::Leaf(_) => {}
        _ => panic!("not leaf"),
    }
}

#[test]
fn write_marked_dirty() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    node.write(DataMode::Overwrite, "data");
    match &node {
        ComponentNode::Leaf(info) => assert!(info.signal.interactive),
        _ => panic!("not leaf"),
    }
}

#[test]
fn on_event_propagates() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    node.on_event(crate::manage::ManageEvent::StepEnd);
}

#[test]
fn start_new_cycle_propagates() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    node.start_new_cycle(42);
}

#[test]
fn compress_delegates_to_component() {
    let mut node = ComponentNode::leaf_with_lifecycle(
        MockComponent::new("t", "T"),
        MockComponent::new("t", "T"),
        MockComponent::new("t", "T"),
    );
    assert!(node.compress());
}

#[test]
fn compress_propagates_through_composite() {
    let child = ComponentNode::leaf_with_lifecycle(
        MockComponent::new("c", "C"),
        MockComponent::new("c", "C"),
        MockComponent::new("c", "C"),
    );
    let mut parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    assert!(parent.compress());
}

#[test]
fn persist_key_delegates_to_component() {
    let node = ComponentNode::leaf(MockComponent::new("t", "T"));
    assert!(node.persist_key().is_none());
}

#[test]
fn persist_key_returns_value() {
    let node = ComponentNode::leaf_with_lifecycle(
        MockComponent::new("t", "T"),
        MockComponent::new("t", "T"),
        MockComponent::new("t", "T").with_persist("my_data"),
    );
    assert_eq!(node.persist_key(), Some("my_data"));
}

#[test]
fn render_node_output_format() {
    let node =
        ComponentNode::leaf(MockComponent::new("test_id", "测试组件").with_content("body text"));
    let output = node.render_node(RenderLevel::Standard);
    assert!(output.contains("[测试组件]"));
    assert!(output.contains("body text"));
}

#[test]
fn render_node_with_actions() {
    let node = ComponentNode::leaf(MockComponent::new("t", "T"));
    let output = node.render_node(RenderLevel::Title);
    assert!(output.contains("展开"));
}

#[test]
fn render_recursive_composite_with_children() {
    let node = ComponentNode::composite(
        MockComponent::new("parent", "父"),
        vec![
            ComponentNode::leaf(MockComponent::new("child1", "子1").with_content("child1 body")),
            ComponentNode::leaf(MockComponent::new("child2", "子2").with_content("child2 body")),
        ],
    );
    let output = node.render_recursive(RenderLevel::Standard, false);
    assert!(output.contains("[父]"));
    assert!(output.contains("[子1]"));
    assert!(output.contains("[子2]"));
    assert!(output.contains("child1 body"));
    assert!(output.contains("child2 body"));
}

#[test]
fn render_recursive_hides_children_at_hidden_level() {
    let mut node = ComponentNode::composite(
        MockComponent::new("p", "P"),
        vec![ComponentNode::leaf(
            MockComponent::new("c", "C").with_content("hidden body"),
        )],
    );
    let ComponentNode::Composite {
        ref mut children, ..
    } = node
    else {
        panic!("not composite")
    };
    children[0].set_level(RenderLevel::Hidden);

    let output = node.render_recursive(RenderLevel::Standard, false);
    assert!(output.contains("[P]"));
    assert!(!output.contains("hidden body"));
}

#[test]
fn is_dirty_leaf() {
    let mut node = ComponentNode::leaf(MockComponent::new("t", "T"));
    assert!(!node.is_dirty());
    node.mark_dirty();
    assert!(node.is_dirty());
    match &mut node {
        ComponentNode::Leaf(info) => {
            info.signal.data_freshness = 0;
            info.signal.interactive = false;
        }
        _ => panic!("not leaf"),
    }
    assert!(!node.is_dirty());
    node.mark_dirty();
    assert!(node.is_dirty());
}

#[test]
fn is_dirty_composite_propagates() {
    let mut child = ComponentNode::leaf(MockComponent::new("c", "C"));
    child.mark_dirty();
    assert!(child.is_dirty());
    let parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    assert!(parent.is_dirty());

    let mut child2 = ComponentNode::leaf(MockComponent::new("c2", "C2"));
    child2.mark_dirty();
    let mut parent2 = ComponentNode::composite(MockComponent::new("p2", "P2"), vec![child2]);
    let ComponentNode::Composite {
        ref mut children, ..
    } = parent2
    else {
        panic!()
    };
    let ComponentNode::Leaf(info) = &mut children[0] else {
        panic!()
    };
    info.signal.data_freshness = 0;
    info.signal.interactive = false;
    assert!(!parent2.is_dirty());
}

// ── ComponentTree 测试 ─────────────────────────────────

#[test]
fn tree_new_is_empty() {
    let tree = ComponentTree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
}

#[test]
fn tree_push_and_len() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    assert_eq!(tree.len(), 2);
    assert!(!tree.is_empty());
}

#[test]
fn tree_remove() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    let removed = tree.remove("a");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id(), "a");
    assert_eq!(tree.len(), 1);
}

#[test]
fn tree_remove_nonexistent() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    assert!(tree.remove("nonexistent").is_none());
}

#[test]
fn tree_remove_nested_composite() {
    let mut tree = ComponentTree::new();
    let child = ComponentNode::leaf(MockComponent::new("child", "子"));
    let parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    tree.push(parent);
    let removed = tree.remove("child");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id(), "child");
    assert!(tree.find("parent").is_some());
    assert!(tree.find("child").is_none());
}

#[test]
fn tree_remove_nested_grandchild() {
    let mut tree = ComponentTree::new();
    let gc = ComponentNode::leaf(MockComponent::new("gc", "孙"));
    let child = ComponentNode::composite(MockComponent::new("c", "子"), vec![gc]);
    let parent = ComponentNode::composite(MockComponent::new("p", "父"), vec![child]);
    tree.push(parent);
    let removed = tree.remove("gc");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id(), "gc");
    assert!(tree.find("gc").is_none());
    assert!(tree.find("c").is_some());
    assert!(tree.find("p").is_some());
}

#[test]
fn tree_remove_nested_clears_temp_expand() {
    let mut tree = ComponentTree::new();
    let child = ComponentNode::leaf(MockComponent::new("child", "子"));
    let parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    tree.push(parent);
    tree.set_temp_expand("child", 3, 0);
    tree.remove("child");
    assert!(tree.temp_expand_info(0).is_none());
}

#[test]
fn tree_clear() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    tree.clear();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
}

#[test]
fn tree_find() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    assert!(tree.find("a").is_some());
    assert!(tree.find("b").is_some());
    assert!(tree.find("c").is_none());
}

#[test]
fn tree_find_mut_and_modify() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("t", "T")));
    tree.find_mut("t").unwrap().set_level(RenderLevel::Summary);
    assert_eq!(tree.find("t").unwrap().level(), RenderLevel::Summary);
}

#[test]
fn tree_state_management() {
    let mut tree = ComponentTree::new();
    tree.add_condition("review");
    assert!(tree.has_condition("review"));
    tree.remove_condition("review");
    assert!(!tree.has_condition("review"));
    tree.set_state("key1", "val1");
}

#[test]
fn tree_trigger() {
    let mut tree = ComponentTree::new();
    tree.trigger("some_event");
}

#[test]
fn tree_mark_dirty() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("t", "T")));
    tree.mark_dirty("t");
    tree.mark_dirty("nonexistent");
}

#[test]
fn tree_write() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("t", "T")));
    assert!(tree.write("t", DataMode::Overwrite, "data"));
    assert!(!tree.write("nonexistent", DataMode::Overwrite, "data"));
}

#[test]
fn tree_iter() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    let ids: Vec<&str> = tree.iter().map(|n| n.id()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn tree_iter_mut() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    for node in tree.iter_mut() {
        node.set_level(RenderLevel::Summary);
    }
    assert_eq!(tree.find("a").unwrap().level(), RenderLevel::Summary);
}

#[test]
fn tree_render_basic() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(
        MockComponent::new("comp1", "组件 1").with_content("hello"),
    ));
    let output = tree.render(200, None, 0);
    assert!(!output.is_empty());
    assert!(output.contains("[组件 1]"));
}

#[test]
fn tree_render_with_multiple_roots() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(
        MockComponent::new("a", "A").with_content("aaa"),
    ));
    tree.push(ComponentNode::leaf(
        MockComponent::new("b", "B").with_content("bbb"),
    ));
    let output = tree.render(500, None, 0);
    assert!(output.contains("[A]"));
    assert!(output.contains("[B]"));
}

#[test]
fn tree_render_extreme_budget_all_hidden() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(
        MockComponent::new("a", "A").with_content("some content here that is longer"),
    ));
    let output = tree.render(1, None, 0);
    assert!(!output.is_empty());
}

#[test]
fn tree_render_tracks_recent_actions() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.add_recent("A", "expand", true);
    let output = tree.render(500, None, 0);
    assert!(output.contains("_recent"));
    assert!(output.contains("expand"));
}

#[test]
fn tree_render_no_recent_after_first_render() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    let _first = tree.render(500, None, 0);
    let _second = tree.render(500, None, 0);
}

#[test]
fn tree_temp_expand_toast_flow() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("comp", "组件")));
    tree.set_temp_expand("comp", 3, 0);
    assert!(tree.temp_expand_info(0).is_some());
    assert_eq!(tree.temp_expand_info(0).unwrap().1, 3);
    assert_eq!(tree.temp_expand_info(1).unwrap().1, 2);
    assert_eq!(tree.temp_expand_info(2).unwrap().1, 1);
    assert!(tree.temp_expand_info(3).is_none());
}

#[test]
fn tree_temp_expand_survives_render() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("comp", "组件")));
    tree.set_temp_expand("comp", 3, 0);
    let _first = tree.render(500, None, 0);
    assert!(tree.temp_expand_info(0).is_some());
    assert_eq!(tree.temp_expand_info(0).unwrap().1, 3);
    let _second = tree.render(500, None, 0);
    assert_eq!(tree.temp_expand_info(0).unwrap().1, 3);
}

#[test]
fn tree_render_visibility_by_condition() {
    let mut tree = ComponentTree::new();
    tree.add_condition("review");
    let _output = tree.render(500, None, 0);
}

#[test]
fn tree_render_with_composite_root() {
    let mut tree = ComponentTree::new();
    let child = ComponentNode::leaf(MockComponent::new("child", "子"));
    let parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    tree.push(parent);
    let output = tree.render(500, None, 0);
    assert!(output.contains("[父]"));
}

#[test]
fn tree_remove_clears_temp_expand() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    tree.set_temp_expand("a", 3, 0);
    tree.remove("a");
    assert!(tree.temp_expand_info(0).is_none());
    assert!(tree.find("b").is_some());
    assert!(tree.find("a").is_none());
}

#[test]
fn debug_format_leaf() {
    let node = ComponentNode::leaf(MockComponent::new("test", "测试"));
    let debug = format!("{:?}", node);
    assert!(debug.contains("Leaf"));
    assert!(debug.contains("test"));
}

#[test]
fn debug_format_composite() {
    let child = ComponentNode::leaf(MockComponent::new("c", "C"));
    let parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    let debug = format!("{:?}", parent);
    assert!(debug.contains("Composite"));
}

#[test]
fn render_node_output_includes_level() {
    let node = ComponentNode::leaf(MockComponent::new("t", "T").with_content("data"));
    let output = node.render_node(RenderLevel::Summary);
    assert!(output.contains("data"));
}

#[test]
fn delta_render_skips_unchanged_component() {
    let node =
        ComponentNode::leaf(MockComponent::new("stable", "固定内容").with_content("不变的数据"));
    // 第一次渲染：全量
    let first = node.render_recursive(RenderLevel::Standard, true);
    assert!(first.contains("固定内容"));
    assert!(first.contains("不变的数据"));
    // 第二次渲染（未 write）：差量标记
    let second = node.render_recursive(RenderLevel::Standard, true);
    assert!(second.contains("[unmodified]"));
    assert!(!second.contains("不变的数据"), "差量渲染不应输出 body");
}

#[test]
fn delta_render_full_after_write() {
    let mut node = ComponentNode::leaf(MockComponent::new("dyn", "动态").with_content("旧内容"));
    // 第一次渲染
    let _first = node.render_recursive(RenderLevel::Standard, true);
    // write 触发 content_gen 增长（MockComponent::write 不更新 render() 输出，
    // 但 ComponentNode::write 会递增 content_gen）
    node.write(crate::data::DataMode::Overwrite, "新内容");
    // 第二次渲染：content_gen != render_gen → 全量渲染
    let second = node.render_recursive(RenderLevel::Standard, true);
    assert!(!second.contains("[unmodified]"), "write 后不应差量跳过");
    // render() 仍返回旧内容（MockComponent 限制），但无 [unmodified] 即证明差量未触发
    assert!(second.contains("旧内容"), "全量渲染应输出 render() 内容");
}

#[test]
fn leaf_handle_action_failure() {
    let mut node = ComponentNode::leaf(
        MockComponent::new("t", "T").with_action_result(ActionResult::error("t", "fail", "failed")),
    );
    let result = node.handle_action("fail", "");
    assert!(!result.is_success());
}

#[test]
fn composite_handle_action_self_first() {
    let child = ComponentNode::leaf(
        MockComponent::new("c", "C").with_action_result(ActionResult::new("c", "c")),
    );
    let mut parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    let result = parent.handle_action("c", "");
    assert!(result.is_success());
    assert_eq!(result.component_id(), "c");
}

#[test]
fn composite_handle_action_unknown() {
    let child = ComponentNode::leaf(MockComponent::new("c", "C"));
    let mut parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    let result = parent.handle_action("nonexistent", "");
    assert!(!result.is_success());
}

#[test]
fn tree_render_with_state_visibility_condition() {
    let mut tree = ComponentTree::new();
    tree.set_state("key", "value");
    tree.push(ComponentNode::leaf(MockComponent::new("comp", "组件")));
    let output = tree.render(500, None, 0);
    assert!(output.contains("[组件]"));
}

#[test]
fn tree_push_creates_recent_record() {
    let mut tree = ComponentTree::new();
    tree.add_recent("组件", "expand", true);
    let output = tree.render(500, None, 0);
    assert!(output.contains("_recent"));
    assert!(output.contains("expand"));
}

#[test]
fn tree_write_to_nonexistent_returns_false() {
    let mut tree = ComponentTree::new();
    assert!(!tree.write("nonexistent", DataMode::Overwrite, "data"));
}

// ── iter_all / iter_all_mut ────────────────────────────

#[test]
fn iter_all_flat_tree() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));
    let ids: Vec<&str> = tree.iter_all().map(|n| n.id()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn iter_all_nested_composite() {
    let mut tree = ComponentTree::new();
    let gc = ComponentNode::leaf(MockComponent::new("gc", "孙"));
    let child = ComponentNode::composite(MockComponent::new("c", "子"), vec![gc]);
    let parent = ComponentNode::composite(MockComponent::new("p", "父"), vec![child]);
    tree.push(parent);
    let ids: Vec<&str> = tree.iter_all().map(|n| n.id()).collect();
    assert_eq!(ids, vec!["p", "c", "gc"]);
}

#[test]
fn iter_all_mut_modifies_level() {
    let mut tree = ComponentTree::new();
    let child = ComponentNode::leaf(MockComponent::new("child", "子"));
    let parent = ComponentNode::composite(MockComponent::new("parent", "父"), vec![child]);
    tree.push(parent);

    for node in tree.iter_all_mut() {
        node.mark_dirty();
    }
    for node in tree.iter_all() {
        assert!(node.is_dirty());
    }
}

#[test]
fn iter_all_mut_hides_all() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    tree.push(ComponentNode::leaf(MockComponent::new("b", "B")));

    for node in tree.iter_all_mut() {
        node.set_level(RenderLevel::Hidden);
    }
    for node in tree.iter_all() {
        assert_eq!(node.level(), RenderLevel::Hidden);
    }
}

// ── 渲染方法测试 ───────────────────────────────────

#[test]
fn tree_render_method() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));

    let output = tree.render(5000, None, 0);
    assert!(output.contains("A"));
}

#[test]
fn tree_render_with_stats() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));

    let (_output, _stats) = tree.render_with_stats(5000, None, 0);
}

// ── TreeSnapshot 测试 ────────────────────────────────

#[test]
fn snapshot_empty_tree() {
    let tree = ComponentTree::new();
    let snap = tree.snapshot();
    assert!(snap.roots.is_empty());
    assert_eq!(snap.stats.total_nodes, 0);
}

#[test]
fn snapshot_single_leaf() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    let snap = tree.snapshot();
    assert_eq!(snap.roots.len(), 1);
    assert_eq!(snap.roots[0].id, "a");
    assert_eq!(snap.roots[0].kind, "leaf");
    assert_eq!(snap.stats.total_nodes, 1);
    assert_eq!(snap.stats.leaf_nodes, 1);
    assert_eq!(snap.stats.composite_nodes, 0);
}

#[test]
fn snapshot_composite_tree() {
    let mut tree = ComponentTree::new();
    let child = ComponentNode::leaf(MockComponent::new("c", "C"));
    let parent = ComponentNode::composite(MockComponent::new("p", "P"), vec![child]);
    tree.push(parent);
    let snap = tree.snapshot();
    assert_eq!(snap.roots.len(), 1);
    assert_eq!(snap.roots[0].id, "p");
    assert_eq!(snap.roots[0].kind, "composite");
    assert_eq!(snap.roots[0].children.len(), 1);
    assert_eq!(snap.roots[0].children[0].id, "c");
    assert_eq!(snap.stats.total_nodes, 2);
    assert_eq!(snap.stats.leaf_nodes, 1);
    assert_eq!(snap.stats.composite_nodes, 1);
}

#[test]
fn snapshot_serde_roundtrip() {
    let mut tree = ComponentTree::new();
    tree.push(ComponentNode::leaf(MockComponent::new("a", "A")));
    let snap = tree.snapshot();
    let json = serde_json::to_string(&snap).unwrap();
    let restored: TreeSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.roots.len(), 1);
    assert_eq!(restored.roots[0].id, "a");
    assert_eq!(restored.stats.total_nodes, 1);
}

#[test]
fn snapshot_node_details() {
    let comp = MockComponent::new("test", "测试组件")
        .with_priority(PriorityLevel::High)
        .with_static();
    let mut node = ComponentNode::leaf(comp);
    node.set_level(RenderLevel::Summary);
    let mut tree = ComponentTree::new();
    tree.push(node);
    let snap = tree.snapshot();
    assert_eq!(snap.roots[0].id, "test");
    assert_eq!(snap.roots[0].title, "测试组件");
    assert_eq!(snap.roots[0].level, "summary");
    assert_eq!(snap.roots[0].priority, "high");
    assert!(snap.roots[0].is_static);
}

// ── collapsible 冷态钳制测试 ──────────────────────────

#[test]
fn clamp_cold_foldable_cold_leaf_to_summary() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Standard);
    schedule::clamp_cold_foldable(&mut node);
    assert_eq!(node.level(), RenderLevel::Summary);
}

#[test]
fn clamp_cold_foldable_hot_leaf_not_clamped() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Standard);
    node.mark_dirty();
    schedule::clamp_cold_foldable(&mut node);
    assert_eq!(node.level(), RenderLevel::Standard);
}

#[test]
fn clamp_cold_foldable_non_foldable_not_clamped() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "不可折叠"));
    node.set_level(RenderLevel::Standard);
    schedule::clamp_cold_foldable(&mut node);
    assert_eq!(node.level(), RenderLevel::Standard);
}

#[test]
fn clamp_cold_foldable_already_summary_noop() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Summary);
    schedule::clamp_cold_foldable(&mut node);
    assert_eq!(node.level(), RenderLevel::Summary);
}

// ── collapsible expand/collapse 动作按钮测试 ──────────

#[test]
fn foldable_actions_shows_expand_when_summary() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Summary);
    let actions = node.actions(RenderLevel::Summary);
    assert!(actions.iter().any(|a| a.id() == "expand"));
    assert!(!actions.iter().any(|a| a.id() == "collapse"));
}

#[test]
fn foldable_actions_shows_collapse_when_standard() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠").with_no_actions());
    node.set_collapsible(true);
    node.set_level(RenderLevel::Standard);
    let actions = node.actions(RenderLevel::Standard);
    assert!(actions.iter().any(|a| a.id() == "collapse"));
    assert!(!actions.iter().any(|a| a.id() == "expand"));
}

#[test]
fn non_foldable_actions_no_expand_collapse() {
    let node = ComponentNode::leaf(MockComponent::new("f", "不可折叠").with_no_actions());
    let actions = node.actions(RenderLevel::Summary);
    assert!(!actions.iter().any(|a| a.id() == "expand"));
    assert!(!actions.iter().any(|a| a.id() == "collapse"));
}

// ── collapsible expand/collapse handle_action 测试 ─────

#[test]
fn handle_action_expand_foldable_leaf() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Summary);
    let result = node.handle_action("expand", "");
    assert!(result.is_success());
    assert_eq!(result.new_level(), Some(RenderLevel::Standard));
    assert_eq!(node.level(), RenderLevel::Standard);
    assert_eq!(node.heat(), 4);
}

#[test]
fn handle_action_collapse_foldable_leaf() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Standard);
    let result = node.handle_action("collapse", "");
    assert!(result.is_success());
    assert_eq!(result.new_level(), Some(RenderLevel::Summary));
    assert_eq!(node.level(), RenderLevel::Summary);
}

#[test]
fn handle_action_expand_ignored_when_standard() {
    let mut node = ComponentNode::leaf(
        MockComponent::new("f", "可折叠")
            .with_no_actions()
            .with_action_result(ActionResult::error("f", "expand", "ignored")),
    );
    node.set_collapsible(true);
    node.set_level(RenderLevel::Standard);
    let result = node.handle_action("expand", "");
    assert!(!result.is_success());
}

#[test]
fn handle_action_collapse_ignored_when_summary() {
    let mut node = ComponentNode::leaf(MockComponent::new("f", "可折叠"));
    node.set_collapsible(true);
    node.set_level(RenderLevel::Summary);
    let result = node.handle_action("collapse", "");
    assert!(!result.is_success());
}

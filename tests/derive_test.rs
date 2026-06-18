//! 验证 cui_derive::CuiComponent 宏在 integration test 中正常工作。
//! 单元测试无法使用 derive 宏因为路径在 crate 内不匹配。

use cui::{
    CuiComponent, ComponentKind, DataMode, PriorityLevel, RenderLevel, VisibilityCondition,
};

/// 常量模式。
#[derive(CuiComponent)]
#[cui(id = "derive_test", title = "派生测试", priority = "low")]
struct DeriveTestComponent;

#[test]
fn derive_constant_mode() {
    let c = DeriveTestComponent;
    assert_eq!(c.id(), "derive_test");
    assert_eq!(c.title(), "派生测试");
    assert_eq!(c.priority(), PriorityLevel::Low);
}

/// 字段代理模式。
#[derive(CuiComponent)]
#[cui(id_field = "key", title_field = "display", priority = "normal")]
struct DeriveFieldComponent {
    key: String,
    display: String,
}

#[test]
fn derive_field_mode() {
    let c = DeriveFieldComponent {
        key: "field_key".into(),
        display: "字段标题".into(),
    };
    assert_eq!(c.id(), "field_key");
    assert_eq!(c.title(), "字段标题");
    assert_eq!(c.priority(), PriorityLevel::Normal);
}

/// 默认字段名（id/title）。
#[derive(CuiComponent)]
struct DeriveDefaultComponent {
    id: String,
    title: String,
}

#[test]
fn derive_default_fields() {
    let c = DeriveDefaultComponent {
        id: "def_id".into(),
        title: "默认标题".into(),
    };
    assert_eq!(c.id(), "def_id");
    assert_eq!(c.title(), "默认标题");
    assert_eq!(c.priority(), PriorityLevel::Normal);
}

// ── 新增属性测试 ──────────────────────────────────────────────

/// kind 属性。
#[derive(CuiComponent)]
#[cui(id = "kind_test", title = "kind_test", kind = "inline")]
struct DeriveKindComponent;

#[test]
fn derive_kind() {
    let c = DeriveKindComponent;
    assert_eq!(c.kind(), ComponentKind::Inline);
}

/// write 属性（默认 write_field = "content"）。
#[derive(CuiComponent)]
#[cui(id = "write_test", title = "write_test", write)]
struct DeriveWriteComponent {
    content: String,
}

#[test]
fn derive_write() {
    let mut c = DeriveWriteComponent {
        content: String::new(),
    };
    c.write(DataMode::Overwrite, "hello");
    assert_eq!(c.content, "hello");
    c.write(DataMode::Append, " world");
    assert_eq!(c.content, "hello world");
    c.write(DataMode::Clear, "");
    assert_eq!(c.content, "");
}

/// write_field 属性指定字段名。
#[derive(CuiComponent)]
#[cui(id = "wf_test", title = "wf_test", write, write_field = "data")]
struct DeriveWriteFieldComponent {
    data: String,
}

#[test]
fn derive_write_field() {
    let mut c = DeriveWriteFieldComponent {
        data: String::new(),
    };
    c.write(DataMode::Overwrite, "value");
    assert_eq!(c.data, "value");
}

/// inert 属性。
#[derive(CuiComponent)]
#[cui(id = "inert_test", title = "inert_test", inert)]
struct DeriveInertComponent;

#[test]
fn derive_inert() {
    let c = DeriveInertComponent;
    assert!(c.is_inert());
}

/// is_static 属性。
#[derive(CuiComponent)]
#[cui(id = "static_test", title = "static_test", is_static)]
struct DeriveStaticComponent;

#[test]
fn derive_is_static() {
    let c = DeriveStaticComponent;
    assert!(c.is_static());
}

/// visibility_field 属性。
#[derive(CuiComponent)]
#[cui(id = "vis_test", title = "vis_test", visibility_field = "condition")]
struct DeriveVisibilityComponent {
    condition: VisibilityCondition,
}

#[test]
fn derive_visibility_field() {
    let c = DeriveVisibilityComponent {
        condition: VisibilityCondition::when("test"),
    };
    assert_eq!(c.visibility_condition(), VisibilityCondition::when("test"));
}

/// render_from 属性。
#[derive(CuiComponent)]
#[cui(id = "render_test", title = "render_test", render_from = "content")]
struct DeriveRenderComponent {
    content: String,
}

#[test]
fn derive_render_from() {
    let c = DeriveRenderComponent {
        content: "第一行\n第二行".into(),
    };
    assert_eq!(c.render(RenderLevel::Hidden), "");
    assert_eq!(c.render(RenderLevel::Title), "");
    assert_eq!(c.render(RenderLevel::Summary), "第一行");
    assert_eq!(c.render(RenderLevel::Standard), "第一行\n第二行");
    assert_eq!(c.render(RenderLevel::Detailed), "第一行\n第二行");
}

/// 组合属性：write + render_from + kind + visibility_field。
#[derive(CuiComponent)]
#[cui(
    id_field = "name",
    title_field = "label",
    priority = "high",
    kind = "block",
    write,
    write_field = "body",
    render_from = "body",
    visibility_field = "cond"
)]
struct DeriveCombinedComponent {
    name: String,
    label: String,
    body: String,
    cond: VisibilityCondition,
}

#[test]
fn derive_combined() {
    let mut c = DeriveCombinedComponent {
        name: "combined".into(),
        label: "组合测试".into(),
        body: "内容".into(),
        cond: VisibilityCondition::Always,
    };
    assert_eq!(c.id(), "combined");
    assert_eq!(c.title(), "组合测试");
    assert_eq!(c.priority(), PriorityLevel::High);
    assert_eq!(c.kind(), ComponentKind::Block);
    assert_eq!(c.render(RenderLevel::Standard), "内容");

    c.write(DataMode::Overwrite, "更新内容");
    assert_eq!(c.render(RenderLevel::Standard), "更新内容");
}

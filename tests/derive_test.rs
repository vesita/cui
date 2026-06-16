//! 验证 cui_derive::BaseComponent 宏在 integration test 中正常工作。
//! 单元测试无法使用 derive 宏因为路径在 crate 内不匹配。

use cui::{BaseComponent, PriorityLevel};

/// 常量模式。
#[derive(BaseComponent)]
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
#[derive(BaseComponent)]
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
#[derive(BaseComponent)]
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

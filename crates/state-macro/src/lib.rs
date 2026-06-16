//! # Machine — 状态机代码生成工具
//!
//! 提供三个 proc-macro，用于定义类型安全的状态机。
//!
//! ## 快速入门
//!
//! ```ignore
//! use state_macro::{states, transitions, facade};
//!
//! states! {
//!     enum TrafficLight {
//!         Green { count: u8 },
//!         Orange,
//!         Red,
//!     }
//! }
//!
//! transitions!(TrafficLight, [
//!     (Green, Advance) => Orange,
//!     (Orange, Advance) => Red,
//!     (Red, Advance) => Green,
//! ]);
//!
//! facade!(TrafficLight, [
//!     Green => get count: u8,
//!     Green, Orange, Red => fn can_pass(&self) -> bool,
//! ]);
//! ```
//!
//! ## 宏参考
//!
//! | 宏 | 用途 |
//! |---|---|
//! | [`states!`] | 定义状态机枚举及每个变体的 struct |
//! | [`transitions!`] | 定义状态之间的转移及消息枚举 |
//! | [`facade!`] | 为状态机生成 getter / setter / 方法分发 |

extern crate proc_macro;

mod codegen;

use proc_macro::TokenStream;

/// 定义状态机枚举及其变体的 struct。
///
/// 示例：
/// ```ignore
/// states! {
///   enum TrafficLight {
///     Green { count: u8 },
///     Orange,
///     Red,
///   }
/// }
/// ```
#[proc_macro]
pub fn states(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as codegen::machine::Machine);
    codegen::machine::expand(&parsed).into()
}

/// 定义状态机转移规则。
///
/// 示例：
/// ```ignore
/// transitions!(TrafficLight, [
///   (Green, Advance) => Orange,
///   (Orange, Advance) => Red,
///   (Red, Advance) => Green,
/// ]);
/// ```
#[proc_macro]
pub fn transitions(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as codegen::transitions::Transitions);
    parsed.expand().into()
}

/// 为状态机生成 getter / setter / 方法分发（门面模式）。
///
/// 示例：
/// ```ignore
/// facade!(TrafficLight, [
///   Green => get count: u8,
///   Green => set count: u8,
///   Green, Orange, Red => fn can_pass(&self) -> bool,
/// ]);
/// ```
#[proc_macro]
pub fn facade(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as codegen::facade::Methods);
    codegen::facade::expand(&parsed).into()
}

/// 为状态机生成有向图元数据（在调用处定义 `{Machine}Graph` 等类型 + `graph()` 方法）。
///
/// 语法：
/// ```ignore
/// state_graph!(MachineName, {
///     Variant => ("中文标签", "IconName"),
///     ...
/// }, [
///     (FromVariant, MessageType) => ToVariant => ("边标签", "IconName"),
///     ...
/// ]);
/// ```
#[proc_macro]
pub fn state_graph(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as codegen::graph::StateGraphInput);
    codegen::graph::expand(&parsed).into()
}

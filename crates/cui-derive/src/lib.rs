//! CUI 框架 derive 宏。
//!
//! ## `#[derive(BaseComponent)]`
//!
//! 为 CUI 组件生成 `BaseComponent` trait 样板代码。
//!
//! ```ignore
//! #[derive(BaseComponent)]
//! #[cui(id = "my_component", title = "我的组件", priority = "high")]
//! struct MyComponent { ... }
//! ```
//!
//! ## `#[derive(ActionHandler)]`
//!
//! 为动作处理器生成 `ActionHandler` trait 样板代码。
//! 约定：结构体需实现 `fn handle(&self, params: &str, ctx: &mut dyn ActionContext) -> Result<ActionOutput, String>`，
//! derive 自动将其委托到 `execute`。
//!
//! ```ignore
//! #[derive(ActionHandler)]
//! #[handler(id = "tool.bash", name = "Bash 执行器")]
//! struct BashHandler;
//!
//! impl BashHandler {
//!     fn handle(&self, params: &str, ctx: &mut dyn ActionContext) -> Result<ActionOutput, String> {
//!         // 自定义逻辑
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Lit, parse_macro_input};

struct CuiAttrs {
    id_expr: proc_macro2::TokenStream,
    title_expr: proc_macro2::TokenStream,
    priority: proc_macro2::TokenStream,
}

fn parse_priority(s: &str) -> proc_macro2::TokenStream {
    match s {
        "minimal" => quote! { ::cui::PriorityLevel::Minimal },
        "low" => quote! { ::cui::PriorityLevel::Low },
        "normal" => quote! { ::cui::PriorityLevel::Normal },
        "high" => quote! { ::cui::PriorityLevel::High },
        "critical" => quote! { ::cui::PriorityLevel::Critical },
        _ => quote! { ::cui::PriorityLevel::Normal },
    }
}

fn parse_cui_attrs(input: &DeriveInput) -> syn::Result<CuiAttrs> {
    // 默认：从 self.id / self.title 字段读取
    let mut id_expr: Option<proc_macro2::TokenStream> = None;
    let mut title_expr: Option<proc_macro2::TokenStream> = None;
    let mut priority: proc_macro2::TokenStream = quote! { ::cui::PriorityLevel::Normal };

    for attr in &input.attrs {
        if !attr.path().is_ident("cui") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let s_val = s.value();
                    id_expr = Some(quote! { #s_val });
                }
            } else if meta.path.is_ident("id_field") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let field = syn::Ident::new(&s.value(), s.span());
                    id_expr = Some(quote! { &self.#field });
                }
            } else if meta.path.is_ident("title") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let s_val = s.value();
                    title_expr = Some(quote! { #s_val });
                }
            } else if meta.path.is_ident("title_field") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let field = syn::Ident::new(&s.value(), s.span());
                    title_expr = Some(quote! { &self.#field });
                }
            } else if meta.path.is_ident("priority") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    priority = parse_priority(&s.value());
                }
            }
            Ok(())
        })?;
    }

    let id_expr = id_expr.unwrap_or_else(|| quote! { &self.id });
    let title_expr = title_expr.unwrap_or_else(|| quote! { &self.title });

    Ok(CuiAttrs {
        id_expr,
        title_expr,
        priority,
    })
}

#[proc_macro_derive(BaseComponent, attributes(cui))]
pub fn derive_base_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let attrs = match parse_cui_attrs(&input) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let name = &input.ident;
    let id_expr = &attrs.id_expr;
    let title_expr = &attrs.title_expr;
    let priority = attrs.priority;

    let expanded = quote! {
        impl ::cui::BaseComponent for #name {
            fn id(&self) -> &str { #id_expr }
            fn title(&self) -> &str { #title_expr }
            fn priority(&self) -> ::cui::PriorityLevel { #priority }

            fn render(&self, _level: ::cui::RenderLevel) -> String {
                String::new()
            }

            fn handle_action(&mut self, action: &str, _params: &str) -> ::cui::action::ActionResult {
                ::cui::action::ActionResult::error(self.id(), action, "derive component has no actions")
            }
        }
    };

    expanded.into()
}

// ── ActionHandler derive ────────────────────────────────────

struct HandlerAttrs {
    id: String,
    name: String,
    schema: Option<String>,
}

fn parse_handler_attrs(input: &DeriveInput) -> syn::Result<HandlerAttrs> {
    let mut id = None;
    let mut name = None;
    let mut schema = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("handler") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    id = Some(s.value());
                }
            } else if meta.path.is_ident("name") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    name = Some(s.value());
                }
            } else if meta.path.is_ident("schema") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    schema = Some(s.value());
                }
            }
            Ok(())
        })?;
    }

    let id = id.unwrap_or_default();
    let name = name.unwrap_or_else(|| input.ident.to_string());

    Ok(HandlerAttrs { id, name, schema })
}

/// 为动作处理器生成 `ActionHandler` trait 实现。
///
/// 约定：目标结构体需实现 `fn handle(&self, params: &str, ctx: &mut dyn ActionContext) -> Result<ActionOutput, String>`，
/// derive 将其委托到 `execute` 方法。
#[proc_macro_derive(ActionHandler, attributes(handler))]
pub fn derive_action_handler(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let attrs = match parse_handler_attrs(&input) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let name = &input.ident;
    let id = &attrs.id;
    let display_name = &attrs.name;

    let schema_body = if let Some(ref s) = attrs.schema {
        let s_val = s;
        quote! { fn params_schema(&self) -> Option<String> { Some(#s_val.to_string()) } }
    } else {
        quote! {}
    };

    let expanded = quote! {
        impl ::cui::ActionHandler for #name {
            fn execute(
                &self,
                params: &str,
                ctx: &mut dyn ::cui::ActionContext,
            ) -> ::std::result::Result<::cui::ActionOutput, String> {
                self.handle(params, ctx)
            }

            fn id(&self) -> &str {
                #id
            }

            fn display_name(&self) -> &str {
                #display_name
            }

            #schema_body
        }
    };

    expanded.into()
}

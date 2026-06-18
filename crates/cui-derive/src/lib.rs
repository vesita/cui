//! CUI 框架 derive 宏。
//!
//! ## `#[derive(CuiComponent)]`
//!
//! 为 CUI 组件生成 `CuiComponent` trait 样板代码。
//!
//! ```ignore
//! #[derive(CuiComponent)]
//! #[cui(
//!     id = "my_component",
//!     title = "My Title",
//!     priority = "high",
//!     kind = "block",
//!     write,
//!     write_field = "data",
//!     render_from = "data",
//!     inert,
//!     is_static,
//!     visibility_field = "condition",
//! )]
//! struct MyComponent {
//!     data: String,
//!     condition: ::cui::VisibilityCondition,
//! }
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
    kind: Option<proc_macro2::TokenStream>,
    write: bool,
    write_field: proc_macro2::TokenStream,
    inert: bool,
    is_static: bool,
    visibility_field: Option<proc_macro2::TokenStream>,
    render_from: Option<proc_macro2::TokenStream>,
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

fn parse_kind(s: &str) -> proc_macro2::TokenStream {
    match s {
        "block" => quote! { ::cui::ComponentKind::Block },
        "inline" => quote! { ::cui::ComponentKind::Inline },
        "action" => quote! { ::cui::ComponentKind::Action },
        "group" => quote! { ::cui::ComponentKind::Group },
        _ => quote! { ::cui::ComponentKind::Block },
    }
}

fn parse_cui_attrs(input: &DeriveInput) -> syn::Result<CuiAttrs> {
    let mut id_expr: Option<proc_macro2::TokenStream> = None;
    let mut title_expr: Option<proc_macro2::TokenStream> = None;
    let mut priority: proc_macro2::TokenStream = quote! { ::cui::PriorityLevel::Normal };
    let mut kind: Option<proc_macro2::TokenStream> = None;
    let mut write_enabled = false;
    let mut write_field: proc_macro2::TokenStream = quote! { content };
    let mut inert = false;
    let mut is_static = false;
    let mut visibility_field: Option<proc_macro2::TokenStream> = None;
    let mut render_from: Option<proc_macro2::TokenStream> = None;

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
            } else if meta.path.is_ident("kind") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    kind = Some(parse_kind(&s.value()));
                }
            } else if meta.path.is_ident("write") {
                write_enabled = true;
            } else if meta.path.is_ident("write_field") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let field = syn::Ident::new(&s.value(), s.span());
                    write_field = quote! { #field };
                }
            } else if meta.path.is_ident("inert") {
                inert = true;
            } else if meta.path.is_ident("is_static") {
                is_static = true;
            } else if meta.path.is_ident("visibility_field") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let field = syn::Ident::new(&s.value(), s.span());
                    visibility_field = Some(quote! { self.#field.clone() });
                }
            } else if meta.path.is_ident("render_from") {
                let value: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = value {
                    let field = syn::Ident::new(&s.value(), s.span());
                    render_from = Some(quote! { #field });
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
        kind,
        write: write_enabled,
        write_field,
        inert,
        is_static,
        visibility_field,
        render_from,
    })
}

#[proc_macro_derive(CuiComponent, attributes(cui))]
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

    let kind_impl = attrs.kind.map(|k| {
        quote! {
            fn kind(&self) -> ::cui::ComponentKind { #k }
        }
    });

    let write_impl = if attrs.write {
        let wf = &attrs.write_field;
        Some(quote! {
            fn write(&mut self, mode: ::cui::DataMode, data: &str) {
                match mode {
                    ::cui::DataMode::Overwrite => self.#wf = data.to_string(),
                    ::cui::DataMode::Append => { self.#wf.push_str(data); }
                    ::cui::DataMode::Clear => self.#wf.clear(),
                }
            }
        })
    } else {
        None
    };

    let inert_impl = if attrs.inert {
        Some(quote! { fn is_inert(&self) -> bool { true } })
    } else {
        None
    };

    let static_impl = if attrs.is_static {
        Some(quote! { fn is_static(&self) -> bool { true } })
    } else {
        None
    };

    let visibility_impl = attrs.visibility_field.map(|vf| {
        quote! {
            fn visibility_condition(&self) -> ::cui::VisibilityCondition { #vf }
        }
    });

    let render_impl = if let Some(rf) = &attrs.render_from {
        quote! {
            fn render(&self, level: ::cui::RenderLevel) -> String {
                match level {
                    ::cui::RenderLevel::Hidden | ::cui::RenderLevel::Title => String::new(),
                    ::cui::RenderLevel::Summary => {
                        self.#rf.lines().next().map(|l| l.to_string()).unwrap_or_default()
                    }
                    ::cui::RenderLevel::Standard | ::cui::RenderLevel::Detailed => {
                        self.#rf.clone()
                    }
                }
            }
        }
    } else {
        quote! {
            fn render(&self, _level: ::cui::RenderLevel) -> String {
                String::new()
            }
        }
    };

    let expanded = quote! {
        impl ::cui::CuiComponent for #name {
            fn id(&self) -> &str { #id_expr }
            fn title(&self) -> &str { #title_expr }
            fn priority(&self) -> ::cui::PriorityLevel { #priority }

            #render_impl

            fn handle_action(&mut self, action: &str, _params: &str) -> ::cui::action::ActionResult {
                ::cui::action::ActionResult::error(self.id(), action, "derive component has no actions")
            }

            #kind_impl
            #write_impl
            #inert_impl
            #static_impl
            #visibility_impl
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
            ) -> ::std::result::Result<::cui::ActionOutput, Box<dyn ::std::error::Error + Send + Sync>> {
                self.handle(params, ctx).map_err(|e| Box::new(e) as Box<dyn ::std::error::Error + Send + Sync>)
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

use case::CaseExt;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{Attribute, Ident, ItemEnum};

/// `states!(enum Name { ... })` 的解析输出。
pub(crate) struct Machine {
    pub attributes: Vec<Attribute>,
    pub data: ItemEnum,
}

impl Parse for Machine {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Machine {
            attributes: input.call(Attribute::parse_outer)?,
            data: input.parse()?,
        })
    }
}

/// 展开 `states!` 宏。
///
/// 为每个变体生成同名的 struct 及构造函数，并添加 Error 变体。
pub(crate) fn expand(input: &Machine) -> TokenStream {
    let Machine { attributes, data } = input;
    let machine_name = &data.ident;

    let variant_names: Vec<&Ident> = data.variants.iter().map(|v| &v.ident).collect();
    let struct_names = variant_names.clone();

    // ── 主枚举 ──────────────────────────────────────────────
    let enum_tokens = quote! {
        #[derive(Clone, Debug, PartialEq)]
        #(#attributes)*
        pub enum #machine_name {
            Error(String),
            #(#variant_names(#struct_names)),*
        }
    };
    let mut output = enum_tokens;

    // ── 每个变体的 struct ───────────────────────────────────
    for variant in &data.variants {
        let name = &variant.ident;
        let fields: Vec<_> = variant
            .fields
            .iter()
            .map(|f| {
                let vis = &f.vis;
                let ident = &f.ident;
                let ty = &f.ty;
                quote! { #vis #ident: #ty }
            })
            .collect();

        output.extend(quote! {
            #[derive(Clone, Debug, PartialEq)]
            pub struct #name {
                #(#fields),*
            }
        });
    }

    // ── 构造函数 ────────────────────────────────────────────
    let ctors: Vec<_> = data
        .variants
        .iter()
        .map(|variant| {
            let fn_name = Ident::new(&variant.ident.to_string().to_snake(), Span::call_site());
            let struct_name = &variant.ident;
            let args: Vec<_> = variant
                .fields
                .iter()
                .map(|f| {
                    let ident = &f.ident;
                    let ty = &f.ty;
                    quote! { #ident: #ty }
                })
                .collect();
            let arg_names: Vec<_> = variant.fields.iter().map(|f| &f.ident).collect();

            quote! {
                pub fn #fn_name(#(#args),*) -> #machine_name {
                    #machine_name::#struct_name(#struct_name { #(#arg_names),* })
                }
            }
        })
        .collect();

    output.extend(quote! {
        impl #machine_name {
            #(#ctors)*
            pub fn error(msg: impl Into<String>) -> #machine_name {
                #machine_name::Error(msg.into())
            }
        }
    });

    output
}

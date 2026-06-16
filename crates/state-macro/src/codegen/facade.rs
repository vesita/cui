use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{Abi, Expr, FnArg, Generics, Ident, ReturnType, Signature, Token, Type, parenthesized};

/// `facade!(Machine, [ ... ])` 的解析输出。
pub(crate) struct Methods {
    pub machine_name: Ident,
    pub methods: Vec<Method>,
}

pub(crate) struct Method {
    pub states: Vec<Ident>,
    pub method_type: MethodType,
    pub default: DefaultValue,
}

pub(crate) enum MethodType {
    Get(Ident, Type),
    Set(Ident, Type),
    Fn(Signature),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum DefaultValue {
    None,
    Default,
    Val(Expr),
}

impl Parse for Methods {
    fn parse(input: ParseStream) -> Result<Self> {
        let machine_name: Ident = input.parse()?;
        let _: Token![,] = input.parse()?;

        let content;
        syn::bracketed!(content in input);

        let mut methods = Vec::new();
        methods.push(content.parse()?);

        loop {
            let lookahead = content.lookahead1();
            if lookahead.peek(Token![,]) {
                let _: Token![,] = content.parse()?;
                if content.is_empty() {
                    break;
                }
                methods.push(content.parse()?);
            } else {
                break;
            }
        }

        Ok(Methods {
            machine_name,
            methods,
        })
    }
}

impl Parse for Method {
    fn parse(input: ParseStream) -> Result<Self> {
        // ── 解析状态列表 ──────────────────────────────────
        let mut states = Vec::new();
        states.push(input.parse()?);

        loop {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
                if input.peek(Token![=>]) {
                    break;
                }
                states.push(input.parse()?);
            } else {
                break;
            }
        }

        let _: Token![=>] = input.parse()?;

        // ── 默认值 ────────────────────────────────────────
        let default = match input.parse::<Option<Token![default]>>() {
            Ok(Some(_)) => {
                if input.peek(syn::token::Paren) {
                    let content;
                    parenthesized!(content in input);
                    DefaultValue::Val(content.parse()?)
                } else {
                    DefaultValue::Default
                }
            }
            _ => DefaultValue::None,
        };

        // ── get / set / fn ────────────────────────────────
        let method_type = match parse_method_sig(input) {
            Ok(sig) => MethodType::Fn(sig),
            Err(_) => {
                let kw: Ident = input.parse()?;
                let name: Ident = input.parse()?;
                let _: Token![:] = input.parse()?;
                let ty: Type = input.parse()?;

                let kw_str = kw.to_string();
                if kw_str == "get" {
                    MethodType::Get(name, ty)
                } else if kw_str == "set" {
                    MethodType::Set(name, ty)
                } else {
                    return Err(syn::Error::new(kw.span(), "expected `get` or `set`"));
                }
            }
        };

        Ok(Method {
            states,
            method_type,
            default,
        })
    }
}

/// 手动解析方法签名。
fn parse_method_sig(input: ParseStream) -> Result<Signature> {
    let constness: Option<Token![const]> = input.parse()?;
    let unsafety: Option<Token![unsafe]> = input.parse()?;
    let asyncness: Option<Token![async]> = input.parse()?;
    let abi: Option<Abi> = input.parse()?;
    let fn_token: Token![fn] = input.parse()?;
    let ident: Ident = input.parse()?;

    // 只在遇到 `<` 时才解析泛型参数，避免 `fn foo(&self)` 被误认
    let generics: Generics = if input.peek(Token![<]) {
        input.parse()?
    } else {
        Generics::default()
    };
    // 合并之前捕获的 where_clause 到 generics
    let generics = Generics {
        where_clause: generics.where_clause,
        ..generics
    };

    let content;
    let paren_token = parenthesized!(content in input);
    let inputs = content.parse_terminated(FnArg::parse, Token![,])?;
    let output: ReturnType = input.parse()?;
    let where_clause = input.parse()?;

    Ok(Signature {
        constness,
        unsafety,
        asyncness,
        abi,
        fn_token,
        ident,
        generics: Generics {
            where_clause,
            ..generics
        },
        paren_token,
        inputs,
        variadic: None,
        output,
    })
}

/// 展开 `facade!` 宏，生成 state struct 上的方法及枚举 wrapper。
pub(crate) fn expand(input: &Methods) -> TokenStream {
    let machine_name = &input.machine_name;
    let mut output = TokenStream::new();

    // 按 state 分组方法
    let mut state_methods: HashMap<&Ident, Vec<&MethodType>> = HashMap::new();
    for method in &input.methods {
        for state in &method.states {
            state_methods
                .entry(state)
                .or_default()
                .push(&method.method_type);
        }
    }

    // ── 为每个 state 生成 impl ────────────────────────────
    for (state, types) in &state_methods {
        let impls: Vec<_> = types
            .iter()
            .map(|mt| match mt {
                MethodType::Get(ident, ty) => {
                    quote! {
                        pub fn #ident(&self) -> &#ty {
                            &self.#ident
                        }
                    }
                }
                MethodType::Set(ident, ty) => {
                    let mut_ident = Ident::new(&format!("{}_mut", ident), ident.span());
                    quote! {
                        pub fn #mut_ident(&mut self) -> &mut #ty {
                            &mut self.#ident
                        }
                    }
                }
                MethodType::Fn(_) => quote! {},
            })
            .collect();

        output.extend(quote! {
            impl #state {
                #(#impls)*
            }
        });
    }

    // ── 主枚举上的 wrapper 方法 ──────────────────────────
    let wrappers: Vec<_> = input
        .methods
        .iter()
        .map(|method| {
            let state_idents = &method.states;
            match &method.method_type {
                MethodType::Get(ident, ty) => {
                    let arms: Vec<_> = state_idents
                        .iter()
                        .map(|s| {
                            quote! {
                                #machine_name::#s(v) => Some(v.#ident()),
                            }
                        })
                        .collect();
                    quote! {
                        pub fn #ident(&self) -> Option<&#ty> {
                            match self {
                                #(#arms)*
                                _ => None,
                            }
                        }
                    }
                }
                MethodType::Set(ident, ty) => {
                    let mut_ident = Ident::new(&format!("{}_mut", ident), ident.span());
                    let arms: Vec<_> = state_idents
                        .iter()
                        .map(|s| {
                            quote! {
                                #machine_name::#s(v) => Some(v.#mut_ident()),
                            }
                        })
                        .collect();
                    quote! {
                        pub fn #mut_ident(&mut self) -> Option<&mut #ty> {
                            match self {
                                #(#arms)*
                                _ => None,
                            }
                        }
                    }
                }
                MethodType::Fn(sig) => {
                    let ident = &sig.ident;
                    let inputs = &sig.inputs;
                    let arg_names: Vec<_> = sig
                        .inputs
                        .iter()
                        .filter_map(|arg| match arg {
                            FnArg::Typed(pat_type) => Some(&pat_type.pat),
                            _ => None,
                        })
                        .collect();

                    let dispatch_body = match &method.default {
                        DefaultValue::None => {
                            let arms: Vec<_> = state_idents
                                .iter()
                                .map(|s| {
                                    quote! {
                                        #machine_name::#s(v) => Some(v.#ident(#(#arg_names),*)),
                                    }
                                })
                                .collect();
                            quote! {
                                match self {
                                    #(#arms)*
                                    _ => None,
                                }
                            }
                        }
                        DefaultValue::Default => {
                            let arms: Vec<_> = state_idents
                                .iter()
                                .map(|s| {
                                    quote! {
                                        #machine_name::#s(v) => v.#ident(#(#arg_names),*),
                                    }
                                })
                                .collect();
                            quote! {
                                match self {
                                    #(#arms)*
                                    _ => std::default::Default::default(),
                                }
                            }
                        }
                        DefaultValue::Val(expr) => {
                            let arms: Vec<_> = state_idents
                                .iter()
                                .map(|s| {
                                    quote! {
                                        #machine_name::#s(v) => v.#ident(#(#arg_names),*),
                                    }
                                })
                                .collect();
                            quote! {
                                match self {
                                    #(#arms)*
                                    _ => #expr,
                                }
                            }
                        }
                    };

                    let wrapper_output = match (&method.default, &sig.output) {
                        (DefaultValue::None, ReturnType::Default) => quote! {},
                        (DefaultValue::None, ReturnType::Type(_, ty)) => quote! { -> Option<#ty> },
                        (_, ReturnType::Default) => quote! {},
                        (_, ReturnType::Type(_, ty)) => quote! { -> #ty },
                    };

                    quote! {
                        pub fn #ident(#inputs) #wrapper_output {
                            #dispatch_body
                        }
                    }
                }
            }
        })
        .collect();

    output.extend(quote! {
        impl #machine_name {
            #(#wrappers)*
        }
    });

    output
}

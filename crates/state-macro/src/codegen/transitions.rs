use std::collections::{HashMap, HashSet};

#[cfg(feature = "dot")]
use std::fs::{self, File};
#[cfg(feature = "dot")]
use std::io::Write;

use proc_macro2::{Span, TokenStream};
#[cfg(feature = "dot")]
use quote::ToTokens;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{GenericArgument, Ident, Token, Type, bracketed, parenthesized};

use crate::codegen::util::{reorder_type_arguments, type_args, type_last_ident, type_to_snake};

/// `transitions!(Machine, [ ... ])` 或 `transitions!(Machine, mut, [ ... ])` 的解析输出。
pub(crate) struct Transitions {
    pub machine_name: Ident,
    pub transitions: Vec<Transition>,
    pub is_mut: bool,
}

pub(crate) struct Transition {
    pub start: Ident,
    pub message: Type,
    pub end: Vec<Ident>,
}

impl Parse for Transitions {
    fn parse(input: ParseStream) -> Result<Self> {
        let machine_name: Ident = input.parse()?;
        let _: Token![,] = input.parse()?;

        // 可选 mut 关键字：transitions!(Machine, mut, [...])
        let is_mut = input.peek(Token![mut]);
        if is_mut {
            let _: Token![mut] = input.parse()?;
            let _: Token![,] = input.parse()?;
        }

        let content;
        bracketed!(content in input);

        let mut transitions = Vec::new();
        transitions.push(content.parse()?);

        loop {
            let lookahead = content.lookahead1();
            if lookahead.peek(Token![,]) {
                let _: Token![,] = content.parse()?;
                if content.is_empty() {
                    break;
                }
                transitions.push(content.parse()?);
            } else {
                break;
            }
        }

        Ok(Transitions {
            machine_name,
            transitions,
            is_mut,
        })
    }
}

impl Parse for Transition {
    fn parse(input: ParseStream) -> Result<Self> {
        let left;
        parenthesized!(left in input);
        let start: Ident = left.parse()?;
        let _: Token![,] = left.parse()?;
        let message: Type = left.parse()?;

        let _: Token![=>] = input.parse()?;

        let end = if input.peek(syn::token::Bracket) {
            let content;
            bracketed!(content in input);
            let mut states = Vec::new();
            states.push(content.parse()?);
            loop {
                let lookahead = content.lookahead1();
                if lookahead.peek(Token![,]) {
                    let _: Token![,] = content.parse()?;
                    if content.is_empty() {
                        break;
                    }
                    states.push(content.parse()?);
                } else {
                    break;
                }
            }
            states
        } else {
            vec![input.parse()?]
        };

        Ok(Transition {
            start,
            message,
            end,
        })
    }
}

impl Transitions {
    /// 渲染 DOT 文件到 `target/machine/`。
    #[cfg(feature = "dot")]
    pub fn render_dot(&self) {
        let name = self.machine_name.to_string().to_lowercase();
        let dir = "target/machine";
        let path = format!("{}/{}.dot", dir, name);

        let _ = fs::create_dir_all(dir);
        let mut file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("machine: 无法创建 DOT 文件 {}: {}", path, e);
                return;
            }
        };

        let _ = writeln!(file, "digraph {} {{", self.machine_name);
        for t in &self.transitions {
            for state in &t.end {
                let msg = t.message.to_token_stream().to_string();
                let _ = writeln!(file, "{} -> {} [ label = \"{}\" ];", t.start, state, msg);
            }
        }
        let _ = writeln!(file, "}}");
    }

    /// 展开 `transitions!` 宏。
    pub fn expand(&self) -> TokenStream {
        // DOT 渲染仅在启用 feature 时执行
        #[cfg(feature = "dot")]
        self.render_dot();

        let machine_name = &self.machine_name;

        // 按 message 分组
        let mut message_map: HashMap<&Type, Vec<(&Ident, &[Ident])>> = HashMap::new();
        for t in &self.transitions {
            message_map
                .entry(&t.message)
                .or_default()
                .push((&t.start, &t.end));
        }

        // 收集泛型参数
        let type_args_all: Vec<GenericArgument> = {
            let mut set = HashSet::new();
            for t in &self.transitions {
                set.extend(type_args(&t.message));
            }
            reorder_type_arguments(set.into_iter().collect())
        };
        let type_arg_toks = (!type_args_all.is_empty()).then(|| {
            quote! { < #(#type_args_all),* > }
        });

        // ── 消息枚举 ─────────────────────────────────────
        let message_enum = Ident::new(&format!("{}Messages", machine_name), Span::call_site());
        let msg_types: Vec<&Type> = message_map.keys().copied().collect();
        let msg_variants: Vec<Ident> = msg_types
            .iter()
            .map(|t| type_last_ident(t).clone())
            .collect();

        let mut output = quote! {
            #[derive(Clone, Debug, PartialEq)]
            pub enum #message_enum #type_arg_toks {
                #(#msg_variants(#msg_types)),*
            }
        };

        // ── 每个消息的 on_xxx 方法 ──────────────────────
        let functions: Vec<_> = message_map
            .iter()
            .map(|(msg, moves)| {
                let fn_name = Ident::new(
                    &format!("on_{}", type_to_snake(msg)),
                    Span::call_site(),
                );
                let arms: Vec<_> = moves
                    .iter()
                    .map(|(start, end)| {
                        if self.is_mut {
                            if end.len() == 1 {
                                let end_state = &end[0];
                                quote! {
                                    #machine_name::#start(state) => {
                                        *self = #machine_name::#end_state(state.#fn_name(input));
                                    }
                                }
                            } else {
                                quote! {
                                    #machine_name::#start(state) => {
                                        *self = state.#fn_name(input);
                                    }
                                }
                            }
                        } else {
                            if end.len() == 1 {
                                let end_state = &end[0];
                                quote! {
                                    #machine_name::#start(state) => #machine_name::#end_state(state.#fn_name(input)),
                                }
                            } else {
                                quote! {
                                    #machine_name::#start(state) => state.#fn_name(input),
                                }
                            }
                        }
                    })
                    .collect();

                let msg_type_args = reorder_type_arguments(type_args(msg));
                let msg_type_arg_toks = (!msg_type_args.is_empty()).then(|| {
                    quote! { < #(#msg_type_args),* > }
                });

                if self.is_mut {
                    quote! {
                        pub fn #fn_name #msg_type_arg_toks(&mut self, input: #msg) {
                            let current = std::mem::replace(self, #machine_name::Error(String::new()));
                            match current {
                                #(#arms)*
                                other => { *self = other; }
                            }
                        }
                    }
                } else {
                    quote! {
                        pub fn #fn_name #msg_type_arg_toks(self, input: #msg) -> #machine_name {
                            match self {
                                #(#arms)*
                                _ => #machine_name::Error(String::new()),
                            }
                        }
                    }
                }
            })
            .collect();

        // ── execute 分发 ────────────────────────────────
        let dispatch_arms: Vec<_> = message_map
            .keys()
            .map(|msg| {
                let fn_name = Ident::new(&format!("on_{}", type_to_snake(msg)), Span::call_site());
                let variant = type_last_ident(msg);
                quote! {
                    #message_enum::#variant(msg) => self.#fn_name(msg),
                }
            })
            .collect();

        let execute_fn = if self.is_mut {
            quote! {
                pub fn execute #type_arg_toks(&mut self, input: #message_enum #type_arg_toks) {
                    match input {
                        #(#dispatch_arms)*
                    }
                }
            }
        } else {
            quote! {
                pub fn execute #type_arg_toks(self, input: #message_enum #type_arg_toks) -> #machine_name {
                    match input {
                        #(#dispatch_arms)*
                    }
                }
            }
        };

        // ── can_handle ──────────────────────────────────────
        // 构建状态 → 消息映射
        let mut state_msg_map: HashMap<&Ident, Vec<&Type>> = HashMap::new();
        for t in &self.transitions {
            state_msg_map.entry(&t.start).or_default().push(&t.message);
        }
        let can_handle_arms: Vec<_> = state_msg_map
            .iter()
            .flat_map(|(state, msgs)| {
                let me = &message_enum;
                let mn = &machine_name;
                msgs.iter().map(move |msg| {
                    let variant = type_last_ident(msg);
                    quote! {
                        (#mn::#state(_), #me::#variant(_)) => true,
                    }
                })
            })
            .collect();
        let can_handle_fn = quote! {
            pub fn can_handle #type_arg_toks (&self, msg: &#message_enum #type_arg_toks) -> bool {
                match (self, msg) {
                    #(#can_handle_arms)*
                    _ => false,
                }
            }
        };

        // ── valid_messages ──────────────────────────────────
        let valid_msg_arms: Vec<_> = state_msg_map
            .iter()
            .map(|(state, msgs)| {
                let names: Vec<_> = msgs
                    .iter()
                    .map(|msg| {
                        let name = type_to_snake(msg);
                        syn::LitStr::new(&name, Span::call_site())
                    })
                    .collect();
                quote! {
                    #machine_name::#state(_) => &[#(#names),*],
                }
            })
            .collect();
        let valid_msgs_fn = quote! {
            pub fn valid_messages(&self) -> &'static [&'static str] {
                match self {
                    #(#valid_msg_arms)*
                    _ => &[],
                }
            }
        };

        output.extend(quote! {
            impl #machine_name {
                #(#functions)*
                #execute_fn
                #can_handle_fn
                #valid_msgs_fn
            }
        });

        output
    }
}

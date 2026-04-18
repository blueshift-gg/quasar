use {
    proc_macro2::TokenStream,
    quasar_schema::{known_address_for_type, pascal_to_snake, IdlAccountItem, IdlPda, IdlSeed},
    quote::{format_ident, quote, ToTokens},
};

pub fn generate_accounts_macro(
    name: &syn::Ident,
    semantics: &[crate::accounts::resolve::FieldSemantics],
) -> TokenStream {
    let descriptors = describe_accounts(semantics);
    let macro_name = format_ident!("__{}_instruction", pascal_to_snake(&name.to_string()));
    let account_fields: Vec<_> = descriptors.iter().map(emit_account_field).collect();
    let account_fields_with_remaining = account_fields.clone();
    let account_metas: Vec<_> = descriptors.iter().map(emit_account_meta).collect();
    let account_metas_with_remaining = account_metas.clone();

    quote! {
        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        #[doc(hidden)]
        #[macro_export]
        macro_rules! #macro_name {
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                }

                impl From<$struct_name> for quasar_lang::client::Instruction {
                    fn from(ix: $struct_name) -> quasar_lang::client::Instruction {
                        let accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as quasar_lang::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        quasar_lang::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, remaining) => {
                pub struct $struct_name {
                    #(#account_fields_with_remaining)*
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<quasar_lang::client::AccountMeta>,
                }

                impl From<$struct_name> for quasar_lang::client::Instruction {
                    fn from(ix: $struct_name) -> quasar_lang::client::Instruction {
                        let mut accounts = ::alloc::vec![
                            #(#account_metas_with_remaining)*
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as quasar_lang::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        quasar_lang::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
        }
    }
}

fn emit_account_field(descriptor: &IdlAccountItem) -> TokenStream {
    let ident: syn::Ident = syn::parse_str(&descriptor.name).expect("valid account field name");
    quote! { pub #ident: quasar_lang::prelude::Address, }
}

fn emit_account_meta(descriptor: &IdlAccountItem) -> TokenStream {
    let ident: syn::Ident = syn::parse_str(&descriptor.name).expect("valid account field name");
    if descriptor.writable {
        let signer = descriptor.signer;
        quote! {
            quasar_lang::client::AccountMeta::new(ix.#ident, #signer),
        }
    } else {
        let signer = descriptor.signer;
        quote! {
            quasar_lang::client::AccountMeta::new_readonly(ix.#ident, #signer),
        }
    }
}

fn describe_accounts(
    semantics: &[crate::accounts::resolve::FieldSemantics],
) -> Vec<IdlAccountItem> {
    semantics
        .iter()
        .map(|sem| IdlAccountItem {
            name: sem.core.ident.to_string(),
            writable: sem.is_writable(),
            signer: matches!(sem.core.shape, crate::accounts::resolve::FieldShape::Signer)
                || sem.client_requires_signer(),
            pda: sem.pda.as_ref().map(describe_pda),
            address: known_address(&sem.core.shape).map(str::to_owned),
        })
        .collect()
}

fn describe_pda(pda: &crate::accounts::resolve::PdaConstraint) -> IdlPda {
    let seeds = match &pda.source {
        crate::accounts::resolve::PdaSource::Raw { seeds } => seeds,
        crate::accounts::resolve::PdaSource::Typed { args, .. } => args,
    };

    IdlPda {
        seeds: seeds.iter().map(describe_seed).collect(),
    }
}

fn describe_seed(seed: &crate::accounts::resolve::SeedNode) -> IdlSeed {
    match seed {
        crate::accounts::resolve::SeedNode::Literal(bytes) => IdlSeed::Const {
            value: bytes.clone(),
        },
        crate::accounts::resolve::SeedNode::AccountAddress { field } => IdlSeed::Account {
            path: field.to_string(),
        },
        crate::accounts::resolve::SeedNode::FieldBytes { root, path, .. } => IdlSeed::Account {
            path: join_path(root, path),
        },
        crate::accounts::resolve::SeedNode::InstructionArg { name, .. } => IdlSeed::Arg {
            path: name.to_string(),
        },
        crate::accounts::resolve::SeedNode::FieldRootedExpr { expr, .. }
        | crate::accounts::resolve::SeedNode::OpaqueExpr(expr) => IdlSeed::Arg {
            path: expr.to_token_stream().to_string(),
        },
    }
}

fn join_path(root: &syn::Ident, path: &[syn::Ident]) -> String {
    let mut joined = root.to_string();
    for segment in path {
        joined.push('.');
        joined.push_str(&segment.to_string());
    }
    joined
}

fn known_address(shape: &crate::accounts::resolve::FieldShape) -> Option<&'static str> {
    match shape {
        crate::accounts::resolve::FieldShape::Program { .. } => {
            let inner = shape.inner_base_name().map(|name| name.to_string());
            known_address_for_type("Program", inner.as_deref())
        }
        crate::accounts::resolve::FieldShape::Sysvar { .. } => {
            let inner = shape.inner_base_name().map(|name| name.to_string());
            known_address_for_type("Sysvar", inner.as_deref())
        }
        _ => None,
    }
}

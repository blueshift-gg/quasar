//! Client instruction macro generation — adapted from v1.
//! No FieldShape references.

use {
    proc_macro2::TokenStream,
    quasar_schema::pascal_to_snake,
    quote::{format_ident, quote},
};

/// Internal account descriptor for client macro generation.
struct AccountDescriptor {
    name: String,
    writable: bool,
    signer: bool,
}

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
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, compact) => {
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
                                    &<$arg_ty as quasar_lang::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as quasar_lang::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
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
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, compact, remaining) => {
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
                                    &<$arg_ty as quasar_lang::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as quasar_lang::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
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

fn emit_account_field(descriptor: &AccountDescriptor) -> TokenStream {
    let ident: syn::Ident = syn::parse_str(&descriptor.name).expect("valid account field name");
    quote! { pub #ident: quasar_lang::prelude::Address, }
}

fn emit_account_meta(descriptor: &AccountDescriptor) -> TokenStream {
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
) -> Vec<AccountDescriptor> {
    semantics
        .iter()
        .map(|sem| {
            let ty = &sem.core.effective_ty;
            // Detect signer/program/sysvar from the type, not from FieldShape
            let is_signer = is_signer_type(ty);

            AccountDescriptor {
                name: sem.core.ident.to_string(),
                writable: sem.is_writable(),
                signer: is_signer || client_requires_signer(sem),
            }
        })
        .collect()
}

fn client_requires_signer(sem: &crate::accounts::resolve::FieldSemantics) -> bool {
    // init without address = keypair signer (non-PDA init)
    sem.has_init() && sem.address.is_none()
}

fn is_signer_type(ty: &syn::Type) -> bool {
    type_base_name(ty).is_some_and(|n| n == "Signer")
}

fn type_base_name(ty: &syn::Type) -> Option<&syn::Ident> {
    match ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| &s.ident),
        _ => None,
    }
}

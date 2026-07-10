//! Client instruction macro generation for `#[derive(Accounts)]` structs.

use {
    proc_macro2::TokenStream,
    quasar_schema::pascal_to_snake,
    quote::{format_ident, quote},
};

/// Internal account descriptor for client macro generation.
struct AccountDescriptor {
    name: syn::Ident,
    writable: bool,
    signer: bool,
}

pub fn generate_accounts_macro(
    name: &syn::Ident,
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> TokenStream {
    let krate = crate::krate::lang_path();
    let descriptors = describe_accounts(plan);
    let macro_name = format_ident!("__{}_instruction", pascal_to_snake(&name.to_string()));
    let account_fields: Vec<_> = descriptors.iter().map(emit_account_field).collect();
    let account_metas: Vec<_> = descriptors.iter().map(emit_account_meta).collect();

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

                impl From<$struct_name> for #krate::client::Instruction {
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
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

                impl From<$struct_name> for #krate::client::Instruction {
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, remaining) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<#krate::client::AccountMeta>,
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let mut accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, compact, remaining) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<#krate::client::AccountMeta>,
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let mut accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
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
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    quote! { pub #ident: #krate::prelude::Address, }
}

fn emit_account_meta(descriptor: &AccountDescriptor) -> TokenStream {
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    let signer = descriptor.signer;
    if descriptor.writable {
        quote! {
            #krate::client::AccountMeta::new(ix.#ident, #signer),
        }
    } else {
        quote! {
            #krate::client::AccountMeta::new_readonly(ix.#ident, #signer),
        }
    }
}

fn describe_accounts(
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> Vec<AccountDescriptor> {
    plan.fields
        .iter()
        .map(|fp| AccountDescriptor {
            name: fp.ident.clone(),
            writable: fp.writable,
            signer: fp.signer,
        })
        .collect()
}

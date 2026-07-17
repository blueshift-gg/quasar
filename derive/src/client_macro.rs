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
    signer: TokenStream,
    /// Const address expression for `Program<T>`/`Sysvar<T>` fields. These
    /// accounts have exactly one canonical address, so the client fills it
    /// in and the instruction struct drops the field.
    fixed_address: Option<TokenStream>,
}

pub fn generate_accounts_macro(
    name: &syn::Ident,
    generics: &syn::Generics,
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> TokenStream {
    let krate = crate::krate::lang_path();
    let descriptors = describe_accounts(name, generics, plan);
    let macro_name = format_ident!("__{}_instruction", pascal_to_snake(&name.to_string()));
    let module_name = format_ident!("__{}_client_macro", pascal_to_snake(&name.to_string()));
    let account_fields: Vec<_> = descriptors.iter().map(emit_account_field).collect();
    let account_metas: Vec<_> = descriptors.iter().map(emit_account_meta).collect();

    quote! {
        #[doc(hidden)]
        #[allow(unexpected_cfgs)]
        mod #module_name {
            #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
            #[macro_export]
            macro_rules! #macro_name {
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    #[allow(unused_variables)]
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
                    #[allow(unused_variables)]
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
                    #[allow(unused_variables)]
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
                    #[allow(unused_variables)]
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
}

fn emit_account_field(descriptor: &AccountDescriptor) -> TokenStream {
    if descriptor.fixed_address.is_some() {
        return TokenStream::new();
    }
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    quote! { pub #ident: #krate::prelude::Address, }
}

fn emit_account_meta(descriptor: &AccountDescriptor) -> TokenStream {
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    let signer = &descriptor.signer;
    let address = match &descriptor.fixed_address {
        Some(fixed) => fixed.clone(),
        None => quote! { ix.#ident },
    };
    if descriptor.writable {
        quote! {
            #krate::client::AccountMeta::new(#address, #signer),
        }
    } else {
        quote! {
            #krate::client::AccountMeta::new_readonly(#address, #signer),
        }
    }
}

fn describe_accounts(
    name: &syn::Ident,
    generics: &syn::Generics,
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> Vec<AccountDescriptor> {
    let static_lifetimes = generics.lifetimes().map(|_| quote! { 'static });
    // The macro only expands inside the generated `cpi` module, whose parent
    // is the `#[program]` module. `super::` reaches the accounts struct even
    // when a client struct in `cpi` shadows its name.
    let account_type = if generics.lifetimes().next().is_some() {
        quote! { super::#name::<#(#static_lifetimes),*> }
    } else {
        quote! { super::#name }
    };

    plan.fields
        .iter()
        .enumerate()
        .map(|(index, fp)| {
            let fixed_address = if fixed_address_expr(fp).is_some() {
                let const_ident = fixed_address_const(&fp.ident);
                Some(quote! { #account_type::#const_ident })
            } else if let Some((_, seeds)) = client_derivable_pda(plan, fp) {
                let fn_ident = pda_address_fn(&fp.ident);
                let args = seeds.iter().filter_map(|seed| match seed {
                    crate::accounts::resolve::specs::IdlSeedPlan::AccountAddr { base } => {
                        Some(quote! { &ix.#base })
                    }
                    _ => None,
                });
                Some(quote! { #account_type::#fn_ident(#(#args,)* &$crate::ID) })
            } else {
                None
            };
            AccountDescriptor {
                name: fp.ident.clone(),
                writable: fp.writable,
                signer: if fp.behavior_init_signer {
                    quote! { #account_type::__QUASAR_ACCOUNT_SIGNERS[#index] }
                } else {
                    let signer = fp.signer;
                    quote! { #signer }
                },
                fixed_address,
            }
        })
        .collect()
}

/// A PDA field the client can derive: every seed is either the address of a
/// plain (non-derived) account field or a constant expression.
pub(crate) fn client_derivable_pda<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    fp: &'p crate::accounts::resolve::specs::FieldPlan,
) -> Option<(
    &'p syn::Path,
    &'p [crate::accounts::resolve::specs::IdlSeedPlan],
)> {
    use crate::accounts::resolve::specs::{IdlResolverPlan, IdlSeedPlan};
    let IdlResolverPlan::Pda { account_ty, seeds } = fp.idl_resolver.as_ref()? else {
        return None;
    };
    let plain_field = |base: &syn::Ident| {
        plan.fields.iter().any(|other| {
            other.ident == *base
                && fixed_address_expr(other).is_none()
                && !matches!(other.idl_resolver, Some(IdlResolverPlan::Pda { .. }))
        })
    };
    let derivable = seeds.iter().all(|seed| match seed {
        IdlSeedPlan::AccountAddr { base } => plain_field(base),
        IdlSeedPlan::Const { .. } => true,
        IdlSeedPlan::AccountField { .. } | IdlSeedPlan::IxArg { .. } => false,
    });
    derivable.then_some((account_ty, seeds.as_slice()))
}

/// The hidden associated fn deriving one PDA field's address.
pub(crate) fn pda_address_fn(field: &syn::Ident) -> syn::Ident {
    format_ident!("__quasar_pda_{}", field)
}

/// The canonical-address expression for a `Program<T>`/`Sysvar<T>` field,
/// valid where the accounts struct (and `T`) is in scope.
pub(crate) fn fixed_address_expr(
    fp: &crate::accounts::resolve::specs::FieldPlan,
) -> Option<TokenStream> {
    use crate::accounts::resolve::specs::{FixedAddressSource, IdlResolverPlan};
    let krate = crate::krate::lang_path();
    match fp.idl_resolver.as_ref()? {
        IdlResolverPlan::FixedAddress { inner_ty, source } => Some(match source {
            FixedAddressSource::Program => quote! { <#inner_ty as #krate::traits::Id>::ID },
            FixedAddressSource::Sysvar => quote! { <#inner_ty as #krate::sysvars::Sysvar>::ID },
        }),
        IdlResolverPlan::Pda { .. } => None,
    }
}

/// The hidden associated const carrying one field's canonical address.
pub(crate) fn fixed_address_const(field: &syn::Ident) -> syn::Ident {
    format_ident!("__QUASAR_FIXED_ADDRESS_{}", field)
}

//! Off-chain instruction builder codegen for `#[derive(Accounts)]`.
//!
//! Generates a `build_instruction()` function that constructs a Solana
//! `Instruction` from typed account addresses — only compiled for non-SBF
//! targets.

use {
    super::attrs::AccountFieldAttrs,
    crate::helpers::{is_signer_type, pascal_to_snake},
    syn::Type,
};

pub(super) fn generate_client_macro(
    name: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_attrs: &[AccountFieldAttrs],
) -> proc_macro2::TokenStream {
    let snake_name = pascal_to_snake(&name.to_string());
    let macro_name_str = format!("__{}_instruction", snake_name);

    let account_fields_str: String = fields
        .iter()
        .map(|f| {
            let field_name = f.ident.as_ref().expect("account field must have an identifier").to_string();
            format!("pub {}: quasar_lang::prelude::Address,", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n                ");

    let account_metas_str: String = fields
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let field_name = f.ident.as_ref().expect("account field must have an identifier").to_string();
            let writable = field_attrs[fi].is_mut
                || matches!(&f.ty, Type::Reference(r) if r.mutability.is_some());
            let is_init_without_seeds = (field_attrs[fi].is_init || field_attrs[fi].init_if_needed)
                && field_attrs[fi].seeds.is_none()
                && field_attrs[fi].associated_token_mint.is_none();
            let signer = is_signer_type(&f.ty) || is_init_without_seeds;
            if writable {
                format!(
                    "quasar_lang::client::AccountMeta::new(ix.{}, {}),",
                    field_name, signer
                )
            } else {
                format!(
                    "quasar_lang::client::AccountMeta::new_readonly(ix.{}, {}),",
                    field_name, signer
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n                        ");

    let macro_def_str = format!(
        r#"
        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        #[doc(hidden)]
        #[macro_export]
        macro_rules! {macro_name} {{
            ($struct_name:ident, [$($disc:expr),*], {{$($arg_name:ident : $arg_ty:ty),*}}) => {{
                pub struct $struct_name {{
                    {account_fields}
                    $(pub $arg_name: $arg_ty,)*
                }}

                impl From<$struct_name> for quasar_lang::client::Instruction {{
                    fn from(ix: $struct_name) -> quasar_lang::client::Instruction {{
                        let accounts = ::alloc::vec![
                            {account_metas}
                        ];
                        let data = {{
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &quasar_lang::client::wincode::serialize(&ix.$arg_name)
                                        .expect("instruction arg serialization")
                                );
                            )*
                            _data
                        }};
                        quasar_lang::client::Instruction {{
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }}
                    }}
                }}
            }};
            ($struct_name:ident, [$($disc:expr),*], {{$($arg_name:ident : $arg_ty:ty),*}}, remaining) => {{
                pub struct $struct_name {{
                    {account_fields}
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<quasar_lang::client::AccountMeta>,
                }}

                impl From<$struct_name> for quasar_lang::client::Instruction {{
                    fn from(ix: $struct_name) -> quasar_lang::client::Instruction {{
                        let mut accounts = ::alloc::vec![
                            {account_metas}
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {{
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &quasar_lang::client::wincode::serialize(&ix.$arg_name)
                                        .expect("instruction arg serialization")
                                );
                            )*
                            _data
                        }};
                        quasar_lang::client::Instruction {{
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }}
                    }}
                }}
            }};
        }}
        "#,
        macro_name = macro_name_str,
        account_fields = account_fields_str,
        account_metas = account_metas_str,
    );

    macro_def_str
        .parse()
        .expect("failed to parse client instruction macro")
}

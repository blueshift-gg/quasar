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
            let field_name = f.ident.as_ref().unwrap().to_string();
            format!("pub {}: quasar_core::prelude::Address,", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n                ");

    let account_metas_str: String = fields
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let field_name = f.ident.as_ref().unwrap().to_string();
            let writable = field_attrs[fi].is_mut
                || matches!(&f.ty, Type::Reference(r) if r.mutability.is_some());
            let signer = is_signer_type(&f.ty);
            if writable {
                format!(
                    "quasar_core::client::AccountMeta::new(ix.{}, {}),",
                    field_name, signer
                )
            } else {
                format!(
                    "quasar_core::client::AccountMeta::new_readonly(ix.{}, {}),",
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

                impl From<$struct_name> for quasar_core::client::Instruction {{
                    fn from(ix: $struct_name) -> quasar_core::client::Instruction {{
                        let accounts = ::alloc::vec![
                            {account_metas}
                        ];
                        let data = quasar_core::client::build_instruction_data(
                            &[$($disc),*],
                            |_data| {{ $(quasar_core::client::WriteBytes::write_bytes(&ix.$arg_name, _data);)* }}
                        );
                        quasar_core::client::Instruction {{
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
                    pub remaining_accounts: ::alloc::vec::Vec<quasar_core::client::AccountMeta>,
                }}

                impl From<$struct_name> for quasar_core::client::Instruction {{
                    fn from(ix: $struct_name) -> quasar_core::client::Instruction {{
                        let mut accounts = ::alloc::vec![
                            {account_metas}
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = quasar_core::client::build_instruction_data(
                            &[$($disc),*],
                            |_data| {{ $(quasar_core::client::WriteBytes::write_bytes(&ix.$arg_name, _data);)* }}
                        );
                        quasar_core::client::Instruction {{
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

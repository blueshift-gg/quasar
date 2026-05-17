//! `#[error_code]`: generates `ProgramError` conversion for custom error
//! enums. Each variant is assigned an error code starting at 6000
//! (Anchor-compatible offset).

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, Data, DeriveInput},
};

pub(crate) fn error_code(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return syn::Error::new_spanned(&input, "#[error_code] can only be used on enums")
                .to_compile_error()
                .into();
        }
    };

    let mut next_discriminant: u32 = 0;
    let mut match_arms = Vec::new();
    let mut idl_error_entries = Vec::new();
    for v in variants.iter() {
        let ident = &v.ident;
        if let Some((_, expr)) = &v.discriminant {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(lit_int),
                ..
            }) = expr
            {
                match lit_int.base10_parse::<u32>() {
                    Ok(val) => next_discriminant = val,
                    Err(_) => {
                        return syn::Error::new_spanned(
                            lit_int,
                            "#[error_code] discriminant must be a valid u32",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            } else {
                return syn::Error::new_spanned(
                    expr,
                    "#[error_code] discriminant must be an integer literal",
                )
                .to_compile_error()
                .into();
            }
        }
        let value = next_discriminant;
        next_discriminant = match next_discriminant.checked_add(1) {
            Some(n) => n,
            None => {
                return syn::Error::new_spanned(
                    &v.ident,
                    "error code overflow: discriminant exceeds u32::MAX",
                )
                .to_compile_error()
                .into();
            }
        };
        match_arms.push(quote! { #value => Ok(#name::#ident) });

        let variant_name = ident.to_string();
        // Extract doc comments from variant attrs
        let docs: Vec<String> = v
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .filter_map(|a| {
                if let syn::Meta::NameValue(nv) = &a.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        return Some(s.value().trim().to_owned());
                    }
                }
                None
            })
            .collect();
        let msg_expr = if docs.is_empty() {
            quote! { None }
        } else {
            let joined = docs.join(" ");
            quote! { Some(quasar_lang::idl_build::s(#joined)) }
        };
        idl_error_entries.push(quote! {
            quasar_lang::idl_build::__reexport::IdlErrorDef {
                code: #value,
                name: quasar_lang::idl_build::s(#variant_name),
                msg: #msg_expr,
            }
        });
    }

    let idl_fragment = quote! {
        #[cfg(feature = "idl-build")]
        quasar_lang::__private_inventory::submit! {
            quasar_lang::idl_build::ErrorFragment {
                build: {
                    fn __build() -> quasar_lang::idl_build::Vec<quasar_lang::idl_build::__reexport::IdlErrorDef> {
                        quasar_lang::idl_build::vec![#(#idl_error_entries),*]
                    }
                    __build
                },
            }
        }
    };

    quote! {
        #[repr(u32)]
        #input

        impl From<#name> for ProgramError {
            #[inline(always)]
            fn from(e: #name) -> Self {
                ProgramError::Custom(e as u32)
            }
        }

        impl TryFrom<u32> for #name {
            type Error = ProgramError;

            #[inline(always)]
            fn try_from(error: u32) -> Result<Self, Self::Error> {
                match error {
                    #(#match_arms,)*
                    _ => Err(ProgramError::InvalidArgument),
                }
            }
        }

        #idl_fragment
    }
    .into()
}

//! `#[error_code]`: generates `ProgramError` conversion for custom error
//! enums. Auto-assigned variants start at error code 6000 (the
//! Anchor-compatible offset); a variant with an explicit integer discriminant
//! keeps that literal value and re-bases the auto-increment from there. Two
//! variants that resolve to the same code — explicit, or via an
//! auto-increment collision — are a hard, spanned error naming both.

use {
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    std::collections::HashMap,
    syn::{Data, DeriveInput},
};

pub(crate) fn error_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    error_code_inner(attr.into(), item.into()).into()
}

pub(crate) fn error_code_inner(_attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    let input = match syn::parse2::<DeriveInput>(item) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return syn::Error::new_spanned(&input, "#[error_code] can only be used on enums")
                .to_compile_error();
        }
    };

    let mut next_discriminant: u32 = 6000;
    let mut match_arms = Vec::new();
    let mut idl_error_entries = Vec::new();
    // Maps an assigned error code back to the variant that claimed it, so a
    // second variant landing on the same code can name both in the diagnostic.
    let mut assigned: HashMap<u32, String> = HashMap::new();
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
                        .to_compile_error();
                    }
                }
            } else {
                return syn::Error::new_spanned(
                    expr,
                    "#[error_code] discriminant must be an integer literal",
                )
                .to_compile_error();
            }
        }
        let value = next_discriminant;
        if let Some(prev) = assigned.get(&value) {
            return syn::Error::new_spanned(
                &v.ident,
                format!(
                    "duplicate error code {value}: variants `{prev}` and `{ident}` both resolve \
                     to the same discriminant",
                ),
            )
            .to_compile_error();
        }
        assigned.insert(value, ident.to_string());
        next_discriminant = match next_discriminant.checked_add(1) {
            Some(n) => n,
            None => {
                return syn::Error::new_spanned(
                    &v.ident,
                    "error code overflow: discriminant exceeds u32::MAX",
                )
                .to_compile_error();
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
            quote! { Some(#krate::idl_build::s(#joined)) }
        };
        idl_error_entries.push(quote! {
            #krate::idl_build::__reexport::IdlErrorDef {
                code: #value,
                name: #krate::idl_build::s(#variant_name),
                msg: #msg_expr,
            }
        });
    }

    let idl_fragment = quote! {
        #[cfg(feature = "idl-build")]
        #krate::__private_inventory::submit! {
            #krate::idl_build::ErrorFragment {
                build: {
                    fn __build() -> #krate::idl_build::Vec<#krate::idl_build::__reexport::IdlErrorDef> {
                        #krate::idl_build::vec![#(#idl_error_entries),*]
                    }
                    __build
                },
            }
        }
    };

    quote! {
        #[repr(u32)]
        #input

        impl From<#name> for #krate::__solana_program_error::ProgramError {
            #[inline(always)]
            fn from(e: #name) -> Self {
                #krate::__solana_program_error::ProgramError::Custom(e as u32)
            }
        }

        impl TryFrom<u32> for #name {
            type Error = #krate::__solana_program_error::ProgramError;

            #[inline(always)]
            fn try_from(error: u32) -> Result<Self, Self::Error> {
                match error {
                    #(#match_arms,)*
                    _ => Err(#krate::__solana_program_error::ProgramError::InvalidArgument),
                }
            }
        }

        #idl_fragment
    }
}

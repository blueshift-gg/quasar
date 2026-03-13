//! `#[error_code]` — generates `ProgramError` conversion for custom error
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
        next_discriminant += 1;
        match_arms.push(quote! { #value => Ok(#name::#ident) });
    }

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
    }
    .into()
}

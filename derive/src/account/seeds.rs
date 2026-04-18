//! Parse `#[seeds(b"prefix", name: Type, ...)]` on account types.

use {
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, Ident, Lit, Token, Type,
    },
};

/// Parsed #[seeds] attribute.
pub struct SeedsAttr {
    pub prefix: Vec<u8>,
    pub dynamic_seed_count: usize,
}

impl Parse for SeedsAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // First element: byte string literal
        let prefix_expr: Expr = input.parse()?;
        let prefix = match &prefix_expr {
            Expr::Lit(ExprLit {
                lit: Lit::ByteStr(b),
                ..
            }) => {
                let bytes = b.value();
                if bytes.len() > 32 {
                    return Err(syn::Error::new_spanned(
                        b,
                        format!(
                            "seed prefix is {} bytes, exceeds MAX_SEED_LEN of 32",
                            bytes.len()
                        ),
                    ));
                }
                bytes
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    prefix_expr,
                    "#[seeds] first argument must be a byte string literal (e.g., b\"vault\")",
                ))
            }
        };

        let mut dynamic_seed_count = 0usize;
        while !input.is_empty() {
            let _: Token![,] = input.parse()?;
            if input.is_empty() {
                break;
            }
            let _name: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            let _ty: Type = input.parse()?;
            dynamic_seed_count += 1;
        }

        Ok(SeedsAttr {
            prefix,
            dynamic_seed_count,
        })
    }
}

/// Extract #[seeds(...)] from attributes, if present.
pub fn parse_seeds_attr(attrs: &[syn::Attribute]) -> Option<syn::Result<SeedsAttr>> {
    attrs
        .iter()
        .find(|a| a.path().is_ident("seeds"))
        .map(|a| a.parse_args::<SeedsAttr>())
}

/// Generate the `HasSeeds` impl for an account type.
///
/// Uses the full generics from the input struct so that arbitrary lifetime
/// and type parameters (not just a single `'a`) are handled correctly.
pub fn generate_seeds_impl(
    name: &syn::Ident,
    generics: &syn::Generics,
    seeds_attr: &SeedsAttr,
) -> proc_macro2::TokenStream {
    let prefix_bytes = &seeds_attr.prefix;
    let dynamic_count = seeds_attr.dynamic_seed_count;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics HasSeeds for #name #ty_generics #where_clause {
            const SEED_PREFIX: &'static [u8] = &[#(#prefix_bytes),*];
            const SEED_DYNAMIC_COUNT: usize = #dynamic_count;
        }
    }
}

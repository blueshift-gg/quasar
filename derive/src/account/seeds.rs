//! Parse `#[seeds(b"prefix", name: Type, ...)]` on account types.

use {
    crate::seed_param::{parse_seed_type, SeedType},
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, Ident, Lit, LitByteStr, Token, Type,
    },
};

/// A single typed seed parameter (e.g. `maker: Address`).
pub struct SeedParam {
    pub name: Ident,
    ty: SeedType,
}

/// Parsed #[seeds] attribute.
pub struct SeedsAttr {
    pub prefix: Vec<u8>,
    pub params: Vec<SeedParam>,
}

impl SeedsAttr {
    pub fn dynamic_seed_count(&self) -> usize {
        self.params.len()
    }
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

        let mut params = Vec::new();
        while !input.is_empty() {
            let _: Token![,] = input.parse()?;
            if input.is_empty() {
                break;
            }
            let name: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            let ty: Type = input.parse()?;
            let ty = parse_seed_type(ty)?;
            params.push(SeedParam { name, ty });
        }

        Ok(SeedsAttr { prefix, params })
    }
}

/// Extract #[seeds(...)] from attributes, if present.
pub fn parse_seeds_attr(attrs: &[syn::Attribute]) -> Option<syn::Result<SeedsAttr>> {
    attrs
        .iter()
        .find(|a| a.path().is_ident("seeds"))
        .map(|a| a.parse_args::<SeedsAttr>())
}

/// Generate the `HasSeeds` impl + `SeedSet` + `SeedSetWithBump` +
/// `AddressVerify` impls for an account type.
///
/// Uses the full generics from the input struct so that arbitrary lifetime
/// and type parameters (not just a single `'a`) are handled correctly.
pub fn generate_seeds_impl(
    name: &syn::Ident,
    generics: &syn::Generics,
    seeds_attr: &SeedsAttr,
) -> proc_macro2::TokenStream {
    let prefix_bytes = &seeds_attr.prefix;
    let prefix_lit = LitByteStr::new(prefix_bytes, proc_macro2::Span::call_site());
    let dynamic_count = seeds_attr.dynamic_seed_count();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let has_seeds_impl = quote! {
        impl #impl_generics HasSeeds for #name #ty_generics #where_clause {
            const SEED_PREFIX: &'static [u8] = &[#(#prefix_bytes),*];
            const SEED_DYNAMIC_COUNT: usize = #dynamic_count;
        }
    };

    // --- SeedSet + SeedSetWithBump + AddressVerify ---
    let seed_set = format_ident!("{}SeedSet", name);
    let seed_set_bump = format_ident!("{}SeedSetWithBump", name);

    // Total number of seed slices (prefix + params).
    let n_slices = 1 + seeds_attr.params.len();
    let n_slices_with_bump = n_slices + 1;

    // SeedSet fields: Address params borrow (&'a Address), scalars own [u8; N].
    // Prefix is NOT stored — as_slices() references the static SEED_PREFIX.
    let param_field_names: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|p| format_ident!("_{}", p.name))
        .collect();
    let param_field_types: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|p| p.ty.field_type())
        .collect();

    // Constructor parameters.
    let param_names: Vec<_> = seeds_attr.params.iter().map(|p| &p.name).collect();
    let param_types: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|p| p.ty.param_type())
        .collect();
    let param_conversions: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|p| p.ty.to_stored_expr(&p.name))
        .collect();

    // as_slices() — prefix from static literal, params from fields.
    let slice_exprs: Vec<_> = {
        let mut v = vec![quote! { #prefix_lit }];
        for (i, field_name) in param_field_names.iter().enumerate() {
            v.push(seeds_attr.params[i].ty.slice_expr(field_name, ""));
        }
        v
    };
    let slice_exprs_bump: Vec<_> = {
        let mut v = vec![quote! { #prefix_lit }];
        for (i, field_name) in param_field_names.iter().enumerate() {
            v.push(seeds_attr.params[i].ty.slice_expr(field_name, "inner"));
        }
        v.push(quote! { &self._bump });
        v
    };
    let signer_seed_exprs: Vec<_> = slice_exprs
        .iter()
        .map(|expr| quote! { quasar_lang::cpi::Seed::from(#expr) })
        .collect();
    let signer_seed_exprs_bump: Vec<_> = slice_exprs_bump
        .iter()
        .map(|expr| quote! { quasar_lang::cpi::Seed::from(#expr) })
        .collect();

    // When no Address params exist, the seed lifetime is unused — add
    // PhantomData to anchor it.
    let has_address_param = seeds_attr
        .params
        .iter()
        .any(|p| matches!(p.ty, SeedType::Address));
    let phantom_field = if has_address_param {
        quote! {}
    } else {
        quote! { _lt: core::marker::PhantomData<&'__quasar_seed ()>, }
    };
    let phantom_init = if has_address_param {
        quote! {}
    } else {
        quote! { _lt: core::marker::PhantomData, }
    };

    quote! {
        #has_seeds_impl

        /// Zero-copy seed storage (without bump).
        pub struct #seed_set<'__quasar_seed> {
            #( #param_field_names: #param_field_types, )*
            #phantom_field
        }

        /// Seed set with explicit bump appended.
        pub struct #seed_set_bump<'__quasar_seed> {
            inner: #seed_set<'__quasar_seed>,
            _bump: [u8; 1],
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #[inline(always)]
            pub fn seeds<'__quasar_seed>(
                #( #param_names: #param_types ),*
            ) -> #seed_set<'__quasar_seed> {
                #seed_set {
                    #( #param_field_names: #param_conversions, )*
                    #phantom_init
                }
            }
        }

        impl<'__quasar_seed> #seed_set<'__quasar_seed> {
            #[inline(always)]
            pub fn with_bump(self, bump: u8) -> #seed_set_bump<'__quasar_seed> {
                #seed_set_bump {
                    inner: self,
                    _bump: [bump],
                }
            }

            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #n_slices] {
                [ #( #slice_exprs ),* ]
            }
        }

        impl<'__quasar_seed> #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #n_slices_with_bump] {
                [ #( #slice_exprs_bump ),* ]
            }
        }

        impl<'__quasar_seed> quasar_lang::cpi::CpiSignerSeeds for #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            fn with_signers<R, F>(&self, f: F) -> R
            where
                F: FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
            {
                let seeds = [#(#signer_seed_exprs_bump),*];
                let signer = quasar_lang::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }

        // AddressVerify: auto-find bump (full derivation, safe for init).
        impl<'__quasar_seed> quasar_lang::address::AddressVerify for #seed_set<'__quasar_seed> {
            #[inline(always)]
            fn verify(
                &self,
                actual: &quasar_lang::prelude::Address,
                program_id: &quasar_lang::prelude::Address,
            ) -> Result<u8, quasar_lang::prelude::ProgramError> {
                let slices = self.as_slices();
                quasar_lang::pda::verify_canonical_program_address(
                    &slices, program_id, actual,
                )
            }

            #[inline(always)]
            fn verify_existing(
                &self,
                actual: &quasar_lang::prelude::Address,
                program_id: &quasar_lang::prelude::Address,
            ) -> Result<u8, quasar_lang::prelude::ProgramError> {
                let slices = self.as_slices();
                let bump = quasar_lang::pda::find_bump_for_address(
                    &slices, program_id, actual,
                ).map_err(|_| quasar_lang::prelude::ProgramError::from(
                    quasar_lang::error::QuasarError::InvalidPda,
                ))?;
                Ok(bump)
            }

            #[inline(always)]
            fn verify_existing_from_account(
                &self,
                actual: &quasar_lang::prelude::Address,
                program_id: &quasar_lang::prelude::Address,
                account: &quasar_lang::__internal::AccountView,
                bump_offset: usize,
            ) -> Result<u8, quasar_lang::prelude::ProgramError> {
                let bump = quasar_lang::pda::read_bump_from_account(account, bump_offset)?;
                let __bump_ref = [bump];
                let slices: [&[u8]; #n_slices_with_bump] = [#(#slice_exprs,)* __bump_ref.as_ref()];
                quasar_lang::pda::verify_program_address(
                    &slices, program_id, actual,
                ).map_err(|_| quasar_lang::prelude::ProgramError::from(
                    quasar_lang::error::QuasarError::InvalidPda,
                ))?;
                Ok(bump)
            }

            #[inline(always)]
            fn with_signer_seeds<R>(
                &self,
                bump: &[u8],
                f: impl FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
            ) -> R {
                let seeds = [
                    #(#signer_seed_exprs,)*
                    quasar_lang::cpi::Seed::from(bump),
                ];
                let signer = quasar_lang::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }

        // AddressVerify: explicit bump (faster, no search).
        impl<'__quasar_seed> quasar_lang::address::AddressVerify for #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            fn verify(
                &self,
                actual: &quasar_lang::prelude::Address,
                program_id: &quasar_lang::prelude::Address,
            ) -> Result<u8, quasar_lang::prelude::ProgramError> {
                let slices = self.as_slices();
                quasar_lang::pda::verify_program_address(
                    &slices, program_id, actual,
                )?;
                Ok(self._bump[0])
            }

            #[inline(always)]
            fn with_signer_seeds<R>(
                &self,
                _bump: &[u8],
                f: impl FnOnce(&[quasar_lang::cpi::Signer<'_, '_>]) -> R,
            ) -> R {
                let seeds = [#(#signer_seed_exprs_bump),*];
                let signer = quasar_lang::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }
    }
}

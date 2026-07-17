//! `#[derive(Seeds)]` and account-local `#[seeds(...)]` codegen.

use {
    crate::seed_param::{parse_seed_type, SeedType, MAX_SEED_LEN},
    proc_macro2::{Span, TokenStream},
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        parse2,
        spanned::Spanned,
        Data, DeriveInput, Error, Generics, Ident, LitByteStr, Result, Token, Type, Visibility,
    },
};

const MAX_PDA_SEEDS_WITHOUT_BUMP: usize = 16;

/// A single typed seed parameter (e.g. `authority: Address`).
pub(crate) struct SeedParam {
    name: Ident,
    ty: SeedType,
}

/// Parsed `#[seeds(b"prefix", name: Type, ...)]` or prefixless
/// `#[seeds(name: Type, ...)]` content.
pub(crate) struct SeedsAttr {
    prefix: Option<Vec<u8>>,
    params: Vec<SeedParam>,
}

impl Parse for SeedsAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let attr_span = input.span();
        if input.is_empty() {
            return Err(Error::new(attr_span, "#[seeds] requires at least one seed"));
        }

        let prefix = if input.peek(LitByteStr) {
            let bytes: LitByteStr = input.parse()?;
            let prefix = bytes.value();
            if prefix.len() > MAX_SEED_LEN {
                return Err(Error::new_spanned(
                    bytes,
                    format!(
                        "seed prefix is {} bytes, exceeds MAX_SEED_LEN of {MAX_SEED_LEN}",
                        prefix.len(),
                    ),
                ));
            }
            if !input.is_empty() {
                let _: Token![,] = input.parse()?;
            }
            Some(prefix)
        } else {
            None
        };

        let mut params = Vec::new();
        while !input.is_empty() {
            let name: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            let ty: Type = input.parse()?;
            let ty = parse_seed_type(ty)?;
            params.push(SeedParam { name, ty });
            if !input.is_empty() {
                let _: Token![,] = input.parse()?;
            }
        }

        let seed_count_without_bump = usize::from(prefix.is_some()) + params.len();
        if seed_count_without_bump > MAX_PDA_SEEDS_WITHOUT_BUMP {
            return Err(Error::new(
                attr_span,
                format!(
                    "PDA seed list has {seed_count_without_bump} seeds before bump; Quasar \
                     supports at most {MAX_PDA_SEEDS_WITHOUT_BUMP}"
                ),
            ));
        }

        Ok(SeedsAttr { prefix, params })
    }
}

/// Extract `#[seeds(...)]` from attributes, if present.
pub(crate) fn parse_seeds_attr(attrs: &[syn::Attribute]) -> Option<Result<SeedsAttr>> {
    let mut seeds_attr = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("seeds")) {
        if seeds_attr.replace(attr).is_some() {
            return Some(Err(Error::new_spanned(
                attr,
                "duplicate #[seeds(...)] attribute",
            )));
        }
    }
    seeds_attr.map(|attr| attr.parse_args::<SeedsAttr>())
}

pub fn derive_seeds(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_seeds_inner(input.into()).into()
}

pub(crate) fn derive_seeds_inner(input: TokenStream) -> TokenStream {
    match derive_seeds_result(input) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error(),
    }
}

fn derive_seeds_result(input: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;

    match &input.data {
        Data::Struct(data) => {
            if !data.fields.is_empty() {
                return Err(Error::new(
                    data.fields.span(),
                    "#[derive(Seeds)] requires a unit struct (no fields)",
                ));
            }
        }
        _ => {
            return Err(Error::new_spanned(
                &input.ident,
                "#[derive(Seeds)] can only be applied to a unit struct",
            ));
        }
    }

    let seeds_attr = parse_seeds_attr(&input.attrs)
        .ok_or_else(|| Error::new_spanned(&input.ident, "missing #[seeds(...)] attribute"))??;

    Ok(generate_seeds_impl(
        &input.ident,
        &input.generics,
        &input.vis,
        &seeds_attr,
    ))
}

/// Generate the `HasSeeds` impl + `SeedSet` + `SeedSetWithBump` +
/// `AddressVerify` impls for either a standalone seed spec or account type.
pub(crate) fn generate_seeds_impl(
    name: &Ident,
    generics: &Generics,
    vis: &Visibility,
    seeds_attr: &SeedsAttr,
) -> TokenStream {
    let krate = crate::krate::lang_path();
    let has_prefix = seeds_attr.prefix.is_some();
    let prefix_bytes = seeds_attr.prefix.as_deref().unwrap_or(&[]);
    let prefix_lit = LitByteStr::new(prefix_bytes, Span::call_site());
    let dynamic_count = seeds_attr.params.len();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let seed_set = format_ident!("{}SeedSet", name);
    let seed_set_bump = format_ident!("{}SeedSetWithBump", name);

    let n_slices = usize::from(has_prefix) + seeds_attr.params.len();
    let n_slices_with_bump = n_slices + 1;

    let param_field_names: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| format_ident!("_{}", param.name))
        .collect();
    let param_field_types: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| param.ty.field_type())
        .collect();

    let param_names: Vec<_> = seeds_attr.params.iter().map(|param| &param.name).collect();
    let param_types: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| param.ty.param_type())
        .collect();
    let param_conversions: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| param.ty.to_stored_expr(&param.name))
        .collect();
    let owned_param_types: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| param.ty.owned_param_type())
        .collect();
    let owned_seed_args: Vec<_> = seeds_attr
        .params
        .iter()
        .map(|param| param.ty.owned_to_seed_arg(&param.name))
        .collect();
    let seed_indices: Vec<_> = (0..seeds_attr.params.len())
        .map(|index| quote! { #index })
        .collect();

    let slice_exprs: Vec<_> = {
        let mut slices = Vec::new();
        if has_prefix {
            slices.push(quote! { #prefix_lit });
        }
        for (idx, field_name) in param_field_names.iter().enumerate() {
            slices.push(seeds_attr.params[idx].ty.slice_expr(field_name, ""));
        }
        slices
    };
    let slice_exprs_bump: Vec<_> = {
        let mut slices = Vec::new();
        if has_prefix {
            slices.push(quote! { #prefix_lit });
        }
        for (idx, field_name) in param_field_names.iter().enumerate() {
            slices.push(seeds_attr.params[idx].ty.slice_expr(field_name, "inner"));
        }
        slices.push(quote! { &self._bump });
        slices
    };
    let signer_seed_exprs: Vec<_> = slice_exprs
        .iter()
        .map(|expr| quote! { #krate::cpi::Seed::from(#expr) })
        .collect();
    let signer_seed_exprs_bump: Vec<_> = slice_exprs_bump
        .iter()
        .map(|expr| quote! { #krate::cpi::Seed::from(#expr) })
        .collect();

    let has_address_param = seeds_attr
        .params
        .iter()
        .any(|param| matches!(param.ty, SeedType::Address));
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
        impl #impl_generics #krate::traits::HasSeeds for #name #ty_generics #where_clause {
            const HAS_SEED_PREFIX: bool = #has_prefix;
            const SEED_PREFIX: &'static [u8] = &[#(#prefix_bytes),*];
            const SEED_DYNAMIC_COUNT: usize = #dynamic_count;
            type WithBump<'__quasar_seed> = #seed_set_bump<'__quasar_seed>;
        }

        /// Zero-copy seed storage (without bump).
        #vis struct #seed_set<'__quasar_seed> {
            #( #param_field_names: #param_field_types, )*
            #phantom_field
        }

        /// Seed set with explicit bump appended.
        #vis struct #seed_set_bump<'__quasar_seed> {
            inner: #seed_set<'__quasar_seed>,
            _bump: [u8; 1],
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #[inline(always)]
            #vis fn seeds<'__quasar_seed>(
                #( #param_names: #param_types ),*
            ) -> #seed_set<'__quasar_seed> {
                #seed_set {
                    #( #param_field_names: #param_conversions, )*
                    #phantom_init
                }
            }

            /// Derive this PDA's canonical address from owned seed values.
            #[inline]
            #vis fn find_address(
                #( #param_names: #owned_param_types, )*
                program_id: &#krate::prelude::Address,
            ) -> #krate::prelude::Address {
                let seeds = Self::seeds(#(#owned_seed_args),*);
                #krate::pda::find_program_address_const(&seeds.as_slices(), program_id).0
            }
        }

        #(
            impl #impl_generics #krate::traits::SeedParam<#seed_indices> for #name #ty_generics #where_clause {
                type Ty = #owned_param_types;
            }
        )*

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

        impl<'__quasar_seed> #krate::traits::SeedSlices for #seed_set<'__quasar_seed> {
            #[inline(always)]
            fn with_slices<R>(&self, f: impl FnOnce(&[&[u8]]) -> R) -> R {
                f(&self.as_slices())
            }
        }

        impl<'__quasar_seed> #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #n_slices_with_bump] {
                [ #( #slice_exprs_bump ),* ]
            }

            /// Materialize the signer-seed array once.
            ///
            /// `invoke_signed(&set)` rebuilds the array on every call (the
            /// pointer escapes into the syscall, so the rebuild cannot be
            /// elided). Bind this before signing several CPIs to pay the
            /// construction cost once.
            #[inline(always)]
            pub fn signer_seeds(&self) -> [#krate::cpi::Seed<'_>; #n_slices_with_bump] {
                [ #( #krate::cpi::Seed::from(#slice_exprs_bump) ),* ]
            }
        }

        impl<'__quasar_seed> #krate::cpi::CpiSignerSeeds for #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            fn with_signers<R, F>(&self, f: F) -> R
            where
                F: FnOnce(&[#krate::cpi::Signer<'_, '_>]) -> R,
            {
                let seeds = [#(#signer_seed_exprs_bump),*];
                let signer = #krate::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }

        impl<'__quasar_seed> #krate::address::AddressVerify for #seed_set<'__quasar_seed> {
            #[inline(always)]
            fn verify(
                &self,
                actual: &#krate::prelude::Address,
                program_id: &#krate::prelude::Address,
            ) -> Result<u8, #krate::prelude::ProgramError> {
                let slices = self.as_slices();
                #krate::pda::verify_canonical_program_address(
                    &slices, program_id, actual,
                )
            }

            #[inline(always)]
            fn verify_existing(
                &self,
                actual: &#krate::prelude::Address,
                program_id: &#krate::prelude::Address,
            ) -> Result<u8, #krate::prelude::ProgramError> {
                let slices = self.as_slices();
                let bump = #krate::pda::find_bump_for_address(
                    &slices, program_id, actual,
                ).map_err(|_| #krate::prelude::ProgramError::from(
                    #krate::error::QuasarError::InvalidPda,
                ))?;
                Ok(bump)
            }

            #[inline(always)]
            fn verify_existing_from_account(
                &self,
                actual: &#krate::prelude::Address,
                program_id: &#krate::prelude::Address,
                account: &#krate::__internal::AccountView,
                bump_offset: usize,
            ) -> Result<u8, #krate::prelude::ProgramError> {
                let bump = #krate::pda::read_bump_from_account(account, bump_offset)?;
                let __bump_ref = [bump];
                let slices: [&[u8]; #n_slices_with_bump] = [#(#slice_exprs,)* __bump_ref.as_ref()];
                #krate::pda::verify_program_address(
                    &slices, program_id, actual,
                ).map_err(|_| #krate::prelude::ProgramError::from(
                    #krate::error::QuasarError::InvalidPda,
                ))?;
                Ok(bump)
            }

            #[inline(always)]
            fn with_signer_seeds<R>(
                &self,
                bump: &[u8],
                f: impl FnOnce(&[#krate::cpi::Signer<'_, '_>]) -> R,
            ) -> R {
                let seeds = [
                    #(#signer_seed_exprs,)*
                    #krate::cpi::Seed::from(bump),
                ];
                let signer = #krate::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }

        impl<'__quasar_seed> #krate::address::AddressVerify for #seed_set_bump<'__quasar_seed> {
            #[inline(always)]
            fn verify(
                &self,
                actual: &#krate::prelude::Address,
                program_id: &#krate::prelude::Address,
            ) -> Result<u8, #krate::prelude::ProgramError> {
                let slices = self.as_slices();
                #krate::pda::verify_program_address(
                    &slices, program_id, actual,
                )?;
                Ok(self._bump[0])
            }

            #[inline(always)]
            fn with_signer_seeds<R>(
                &self,
                _bump: &[u8],
                f: impl FnOnce(&[#krate::cpi::Signer<'_, '_>]) -> R,
            ) -> R {
                let seeds = [#(#signer_seed_exprs_bump),*];
                let signer = #krate::cpi::Signer::from(&seeds);
                f(core::slice::from_ref(&signer))
            }
        }
    }
}

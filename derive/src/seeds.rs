//! `#[derive(Seeds)]` — typed PDA seed specs.
//!
//! Given:
//! ```ignore
//! #[derive(Seeds)]
//! #[seeds(b"vault_token", escrow: Address)]
//! pub struct VaultTokenPda;
//! ```
//!
//! Generates `VaultTokenPdaSeedSet` and `VaultTokenPdaSeedSetWithBump` structs
//! with `AddressVerify` impls.

use {
    proc_macro2::{Span, TokenStream},
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        parse2,
        spanned::Spanned,
        Data, DeriveInput, Error, Ident, LitByteStr, Result, Token,
    },
};

/// A single typed seed parameter (e.g. `authority: Address`).
struct SeedParam {
    name: Ident,
    ty: SeedType,
}

/// Supported seed parameter types.
enum SeedType {
    Address,
    U8,
    U16,
    U32,
    U64,
}

impl SeedType {
    /// The field storage type in SeedSet.
    /// Address: borrowed reference (zero-copy).
    /// Scalars: owned byte array (needs backing storage for to_le_bytes).
    fn field_type(&self) -> TokenStream {
        match self {
            SeedType::Address => quote! { &'__quasar_seed quasar_lang::prelude::Address },
            SeedType::U8 => quote! { [u8; 1] },
            SeedType::U16 => quote! { [u8; 2] },
            SeedType::U32 => quote! { [u8; 4] },
            SeedType::U64 => quote! { [u8; 8] },
        }
    }

    /// The constructor parameter type. Address uses the generated seed lifetime
    /// to tie the borrow to the SeedSet.
    fn param_type(&self) -> TokenStream {
        match self {
            SeedType::Address => quote! { &'__quasar_seed quasar_lang::prelude::Address },
            SeedType::U8 => quote! { u8 },
            SeedType::U16 => quote! { u16 },
            SeedType::U32 => quote! { u32 },
            SeedType::U64 => quote! { u64 },
        }
    }

    /// Expression to store the parameter in the SeedSet field.
    fn to_stored_expr(&self, param: &Ident) -> TokenStream {
        match self {
            SeedType::Address => quote! { #param },
            SeedType::U8 => quote! { [#param] },
            _ => quote! { #param.to_le_bytes() },
        }
    }

    /// Expression for as_slices() — how to get a `&[u8]` from the field.
    fn slice_expr(&self, field_name: &Ident, prefix: &str) -> TokenStream {
        let prefix_ident: Option<Ident> = if prefix.is_empty() {
            None
        } else {
            Some(Ident::new(prefix, field_name.span()))
        };
        let access = match prefix_ident {
            None => quote! { self.#field_name },
            Some(p) => quote! { self.#p.#field_name },
        };
        match self {
            SeedType::Address => quote! { #access.as_ref() },
            _ => quote! { &#access },
        }
    }
}

/// Parsed `#[seeds(...)]` attribute content.
struct SeedsAttr {
    prefix: LitByteStr,
    params: Vec<SeedParam>,
}

impl Parse for SeedsAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let prefix: LitByteStr = input.parse()?;

        let mut params = Vec::new();
        while input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
            // Allow trailing comma
            if input.is_empty() {
                break;
            }
            let name: Ident = input.parse()?;
            let _: Token![:] = input.parse()?;
            let ty_ident: Ident = input.parse()?;
            let ty = match ty_ident.to_string().as_str() {
                "Address" => SeedType::Address,
                "u8" => SeedType::U8,
                "u16" => SeedType::U16,
                "u32" => SeedType::U32,
                "u64" => SeedType::U64,
                _ => {
                    return Err(Error::new(
                        ty_ident.span(),
                        "unsupported seed type; expected Address, u8, u16, u32, or u64",
                    ))
                }
            };
            params.push(SeedParam { name, ty });
        }

        Ok(SeedsAttr { prefix, params })
    }
}

pub fn derive_seeds(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match derive_seeds_inner(input.into()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn derive_seeds_inner(input: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;

    // Must be a unit struct.
    match &input.data {
        Data::Struct(ds) => {
            if !ds.fields.is_empty() {
                return Err(Error::new(
                    ds.fields.span(),
                    "#[derive(Seeds)] requires a unit struct (no fields)",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "#[derive(Seeds)] can only be applied to a unit struct",
            ));
        }
    }

    // Find the #[seeds(...)] attribute.
    let seeds_attr = input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("seeds"))
        .ok_or_else(|| Error::new(Span::call_site(), "missing #[seeds(...)] attribute"))?;

    let parsed: SeedsAttr = seeds_attr.parse_args()?;
    let prefix = &parsed.prefix;
    let struct_name = &input.ident;
    let seed_set = format_ident!("{}SeedSet", struct_name);
    let seed_set_bump = format_ident!("{}SeedSetWithBump", struct_name);

    // Total number of seed slices (prefix + params).
    let n_slices = 1 + parsed.params.len();
    let n_slices_with_bump = n_slices + 1;

    let param_field_names: Vec<_> = parsed
        .params
        .iter()
        .map(|p| format_ident!("_{}", p.name))
        .collect();
    let param_field_types: Vec<_> = parsed.params.iter().map(|p| p.ty.field_type()).collect();

    // Constructor parameters.
    let param_names: Vec<_> = parsed.params.iter().map(|p| &p.name).collect();
    let param_types: Vec<_> = parsed.params.iter().map(|p| p.ty.param_type()).collect();
    let param_conversions: Vec<_> = parsed
        .params
        .iter()
        .map(|p| p.ty.to_stored_expr(&p.name))
        .collect();

    // as_slices() — prefix from static literal, params from fields.
    let slice_exprs: Vec<_> = {
        let mut v = vec![quote! { #prefix }];
        for (i, name) in param_field_names.iter().enumerate() {
            v.push(parsed.params[i].ty.slice_expr(name, ""));
        }
        v
    };
    let slice_exprs_bump: Vec<_> = {
        let mut v = vec![quote! { #prefix }];
        for (i, name) in param_field_names.iter().enumerate() {
            v.push(parsed.params[i].ty.slice_expr(name, "inner"));
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

    let vis = &input.vis;

    let has_address_param = parsed
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

    Ok(quote! {
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

        impl #struct_name {
            #[inline(always)]
            #vis fn seeds<'__quasar_seed>(
                #( #param_names: #param_types ),*
            ) -> #seed_set<'__quasar_seed> {
                #seed_set {
                    #( #param_field_names: #param_conversions, )*
                    #phantom_init
                }
            }
        }

        impl quasar_lang::traits::HasSeeds for #struct_name {
            const SEED_PREFIX: &'static [u8] = #prefix;
            const SEED_DYNAMIC_COUNT: usize = #n_slices - 1;
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

            /// Fast path for existing validated accounts. Skips on-curve check,
            /// uses keys_eq instead (~90 CU savings per PDA).
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
    })
}

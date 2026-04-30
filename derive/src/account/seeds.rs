//! Parse `#[seeds(b"prefix", name: Type, ...)]` on account types.

use {
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, Ident, Lit, Token,
    },
};

/// Supported seed parameter types.
enum SeedType {
    Address,
    U8,
    U16,
    U32,
    U64,
}

impl SeedType {
    fn byte_len(&self) -> usize {
        match self {
            SeedType::Address => 32,
            SeedType::U8 => 1,
            SeedType::U16 => 2,
            SeedType::U32 => 4,
            SeedType::U64 => 8,
        }
    }

    /// The field storage type: `[u8; N]`.
    fn field_type(&self) -> proc_macro2::TokenStream {
        let n = self.byte_len();
        quote! { [u8; #n] }
    }

    /// The constructor parameter type (by-ref for Address, by-value for
    /// scalars).
    fn param_type(&self) -> proc_macro2::TokenStream {
        match self {
            SeedType::Address => quote! { &quasar_lang::prelude::Address },
            SeedType::U8 => quote! { u8 },
            SeedType::U16 => quote! { u16 },
            SeedType::U32 => quote! { u32 },
            SeedType::U64 => quote! { u64 },
        }
    }

    /// Expression to convert the parameter into `[u8; N]`.
    fn to_bytes_expr(&self, param: &Ident) -> proc_macro2::TokenStream {
        match self {
            SeedType::Address => quote! { #param.to_bytes() },
            SeedType::U8 => quote! { [#param] },
            _ => quote! { #param.to_le_bytes() },
        }
    }
}

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
            let ty_ident: Ident = input.parse()?;
            let ty = match ty_ident.to_string().as_str() {
                "Address" => SeedType::Address,
                "u8" => SeedType::U8,
                "u16" => SeedType::U16,
                "u32" => SeedType::U32,
                "u64" => SeedType::U64,
                _ => {
                    return Err(syn::Error::new(
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
    let prefix_len = prefix_bytes.len();
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

    // Build struct fields for SeedSet.
    let prefix_field_type = quote! { [u8; #prefix_len] };

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
        .map(|p| p.ty.to_bytes_expr(&p.name))
        .collect();

    // as_slices() body for SeedSet (without bump).
    let slice_exprs: Vec<_> = {
        let mut v = vec![quote! { &self._prefix }];
        for field_name in &param_field_names {
            v.push(quote! { &self.#field_name });
        }
        v
    };

    // as_slices() body for SeedSetWithBump (with bump).
    let slice_exprs_bump: Vec<_> = {
        let mut v = vec![quote! { &self.inner._prefix }];
        for field_name in &param_field_names {
            v.push(quote! { &self.inner.#field_name });
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

    quote! {
        #has_seeds_impl

        /// Owned seed storage (without bump).
        pub struct #seed_set {
            _prefix: #prefix_field_type,
            #( #param_field_names: #param_field_types, )*
        }

        /// Seed set with explicit bump appended.
        pub struct #seed_set_bump {
            inner: #seed_set,
            _bump: [u8; 1],
        }

        impl #impl_generics #name #ty_generics #where_clause {
            /// Build a seed set (bump will be found automatically during verification).
            #[inline(always)]
            pub fn seeds(#( #param_names: #param_types ),*) -> #seed_set {
                #seed_set {
                    _prefix: [#(#prefix_bytes),*],
                    #( #param_field_names: #param_conversions, )*
                }
            }
        }

        impl #seed_set {
            /// Append an explicit bump byte.
            #[inline(always)]
            pub fn with_bump(self, bump: u8) -> #seed_set_bump {
                #seed_set_bump {
                    inner: self,
                    _bump: [bump],
                }
            }

            /// Borrow as seed slices (without bump).
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #n_slices] {
                [ #( #slice_exprs ),* ]
            }
        }

        impl #seed_set_bump {
            /// Borrow as seed slices (with bump).
            #[inline(always)]
            pub fn as_slices(&self) -> [&[u8]; #n_slices_with_bump] {
                [ #( #slice_exprs_bump ),* ]
            }
        }

        // AddressVerify: auto-find bump (full derivation, safe for init).
        impl quasar_lang::address::AddressVerify for #seed_set {
            #[inline(always)]
            fn verify(
                &self,
                actual: &quasar_lang::prelude::Address,
                program_id: &quasar_lang::prelude::Address,
            ) -> Result<u8, quasar_lang::prelude::ProgramError> {
                let slices = self.as_slices();
                let (expected, bump) = quasar_lang::pda::based_try_find_program_address(
                    &slices, program_id,
                )?;
                if quasar_lang::keys_eq(actual, &expected) {
                    Ok(bump)
                } else {
                    Err(quasar_lang::prelude::ProgramError::from(
                        quasar_lang::error::QuasarError::InvalidPda,
                    ))
                }
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
            fn with_signer_seeds<R>(
                &self,
                bump: &[u8],
                f: impl FnOnce(Option<quasar_lang::cpi::Signer<'_, '_>>) -> R,
            ) -> R {
                let seeds = [
                    #(#signer_seed_exprs,)*
                    quasar_lang::cpi::Seed::from(bump),
                ];
                f(Some(quasar_lang::cpi::Signer::from(&seeds)))
            }
        }

        // AddressVerify: explicit bump (faster, no search).
        impl quasar_lang::address::AddressVerify for #seed_set_bump {
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
                f: impl FnOnce(Option<quasar_lang::cpi::Signer<'_, '_>>) -> R,
            ) -> R {
                let seeds = [#(#signer_seed_exprs_bump),*];
                f(Some(quasar_lang::cpi::Signer::from(&seeds)))
            }
        }
    }
}

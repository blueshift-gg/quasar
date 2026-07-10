use {
    quote::quote,
    syn::{Error, Expr, Ident, Lit, Result, Type},
};

pub(crate) const MAX_SEED_LEN: usize = 32;

/// Supported typed PDA seed parameter types.
pub(crate) enum SeedType {
    Address,
    U8,
    U16,
    U32,
    U64,
    Bytes(usize),
}

impl SeedType {
    /// The field storage type in generated SeedSet structs.
    pub(crate) fn field_type(&self) -> proc_macro2::TokenStream {
        let krate = crate::krate::lang_path();
        match self {
            SeedType::Address => quote! { &'__quasar_seed #krate::prelude::Address },
            SeedType::U8 => quote! { [u8; 1] },
            SeedType::U16 => quote! { [u8; 2] },
            SeedType::U32 => quote! { [u8; 4] },
            SeedType::U64 => quote! { [u8; 8] },
            SeedType::Bytes(len) => quote! { [u8; #len] },
        }
    }

    /// The public constructor parameter type for a typed seed.
    pub(crate) fn param_type(&self) -> proc_macro2::TokenStream {
        let krate = crate::krate::lang_path();
        match self {
            SeedType::Address => quote! { &'__quasar_seed #krate::prelude::Address },
            SeedType::U8 => quote! { u8 },
            SeedType::U16 => quote! { u16 },
            SeedType::U32 => quote! { u32 },
            SeedType::U64 => quote! { u64 },
            SeedType::Bytes(len) => quote! { [u8; #len] },
        }
    }

    /// Expression to store the constructor parameter in the SeedSet field.
    pub(crate) fn to_stored_expr(&self, param: &Ident) -> proc_macro2::TokenStream {
        match self {
            SeedType::Address | SeedType::Bytes(_) => quote! { #param },
            SeedType::U8 => quote! { [#param] },
            SeedType::U16 | SeedType::U32 | SeedType::U64 => quote! { #param.to_le_bytes() },
        }
    }

    /// Expression for turning a generated SeedSet field into a seed slice.
    pub(crate) fn slice_expr(&self, field_name: &Ident, prefix: &str) -> proc_macro2::TokenStream {
        let prefix_ident = (!prefix.is_empty()).then(|| Ident::new(prefix, field_name.span()));
        let access = match prefix_ident {
            None => quote! { self.#field_name },
            Some(p) => quote! { self.#p.#field_name },
        };
        match self {
            SeedType::Address => quote! { #access.as_ref() },
            SeedType::U8 | SeedType::U16 | SeedType::U32 | SeedType::U64 | SeedType::Bytes(_) => {
                quote! { &#access }
            }
        }
    }
}

pub(crate) fn parse_seed_type(ty: Type) -> Result<SeedType> {
    if let Type::Path(type_path) = &ty {
        if let Some(ident) = type_path.path.get_ident() {
            return match ident.to_string().as_str() {
                "Address" => Ok(SeedType::Address),
                "u8" => Ok(SeedType::U8),
                "u16" => Ok(SeedType::U16),
                "u32" => Ok(SeedType::U32),
                "u64" => Ok(SeedType::U64),
                _ => Err(Error::new(
                    ident.span(),
                    format!(
                        "unsupported seed type; expected Address, u8, u16, u32, u64, or [u8; N] \
                         where N <= {MAX_SEED_LEN}"
                    ),
                )),
            };
        }
    }

    if let Type::Array(array) = &ty {
        let Type::Path(elem_path) = array.elem.as_ref() else {
            return Err(Error::new_spanned(
                &array.elem,
                "unsupported seed array element; expected u8",
            ));
        };
        if !elem_path.path.is_ident("u8") {
            return Err(Error::new_spanned(
                &array.elem,
                "unsupported seed array element; expected u8",
            ));
        }
        let Expr::Lit(expr_lit) = &array.len else {
            return Err(Error::new_spanned(
                &array.len,
                "seed byte array length must be an integer literal",
            ));
        };
        let Lit::Int(lit_int) = &expr_lit.lit else {
            return Err(Error::new_spanned(
                &array.len,
                "seed byte array length must be an integer literal",
            ));
        };
        let len = lit_int.base10_parse::<usize>()?;
        if len > MAX_SEED_LEN {
            return Err(Error::new_spanned(
                &array.len,
                format!("seed byte array length exceeds MAX_SEED_LEN of {MAX_SEED_LEN}"),
            ));
        }
        return Ok(SeedType::Bytes(len));
    }

    Err(Error::new_spanned(
        ty,
        format!(
            "unsupported seed type; expected Address, u8, u16, u32, u64, or [u8; N] where N <= \
             {MAX_SEED_LEN}"
        ),
    ))
}

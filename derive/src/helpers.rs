use {
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, GenericArgument, Ident, Lit, LitInt, PathArguments, Token, Type,
    },
};

// --- Discriminator argument parsing (shared by instruction, account, event, program) ---

pub(crate) struct InstructionArgs {
    pub discriminator: Vec<LitInt>,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident != "discriminator" {
            return Err(syn::Error::new(ident.span(), "expected `discriminator`"));
        }
        let _: Token![=] = input.parse()?;
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let lits = content.parse_terminated(LitInt::parse, Token![,])?;
            let discriminator: Vec<LitInt> = lits.into_iter().collect();
            if discriminator.is_empty() {
                return Err(syn::Error::new(
                    input.span(),
                    "discriminator must have at least one byte",
                ));
            }
            Ok(Self { discriminator })
        } else {
            let lit: LitInt = input.parse()?;
            Ok(Self {
                discriminator: vec![lit],
            })
        }
    }
}

// --- Type helpers ---

/// Expand a seed expression into a byte slice for use inside parse (fields are local variables).
pub(crate) fn seed_slice_expr_for_parse(
    expr: &Expr,
    field_names: &[String],
) -> proc_macro2::TokenStream {
    if let Expr::Path(ep) = expr {
        if ep.path.segments.len() == 1 && ep.qself.is_none() {
            let ident = &ep.path.segments[0].ident;
            if field_names.contains(&ident.to_string()) {
                return quote! { #ident.to_account_view().address().as_ref() };
            }
        }
    }
    quote! { #expr as &[u8] }
}

/// Check if a field type's base type is `Signer`.
pub(crate) fn is_signer_type(ty: &Type) -> bool {
    let inner = match ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    if let Type::Path(p) = inner {
        if let Some(last) = p.path.segments.last() {
            return last.ident == "Signer";
        }
    }
    false
}

pub(crate) fn strip_generics(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(type_path) => {
            let segments: Vec<_> = type_path
                .path
                .segments
                .iter()
                .map(|seg| &seg.ident)
                .collect();
            quote! { #(#segments)::* }
        }
        _ => panic!("Unsupported field type"),
    }
}

pub(crate) fn pascal_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

pub(crate) fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.collect::<String>(),
            }
        })
        .collect()
}

// --- Dynamic field detection ---

/// Detects `String<'a, N>` and returns `Some(N)` (the max byte length).
pub(crate) fn is_dynamic_string(ty: &Type) -> Option<usize> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "String" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    if !matches!(iter.next(), Some(GenericArgument::Lifetime(_))) {
                        return None;
                    }
                    if let Some(GenericArgument::Const(Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }))) = iter.next()
                    {
                        return lit_int.base10_parse::<usize>().ok();
                    }
                }
            }
        }
    }
    None
}

/// Detects `Vec<'a, T, N>` and returns `Some((T, N))` (element type, max count).
pub(crate) fn is_dynamic_vec(ty: &Type) -> Option<(Type, usize)> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    if !matches!(iter.next(), Some(GenericArgument::Lifetime(_))) {
                        return None;
                    }
                    let elem_ty = match iter.next() {
                        Some(GenericArgument::Type(ty)) => ty.clone(),
                        _ => return None,
                    };
                    if let Some(GenericArgument::Const(Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }))) = iter.next()
                    {
                        let max = lit_int.base10_parse::<usize>().ok()?;
                        return Some((elem_ty, max));
                    }
                }
            }
        }
    }
    None
}

// --- Instruction-level dynamic field detection (no lifetime) ---

/// Detects `String<N>` (no lifetime) in instruction args and returns `Some(N)`.
pub(crate) fn is_ix_dynamic_string(ty: &Type) -> Option<usize> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "String" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    if let Some(GenericArgument::Const(Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }))) = iter.next()
                    {
                        return lit_int.base10_parse::<usize>().ok();
                    }
                }
            }
        }
    }
    None
}

/// Detects `Vec<T, N>` (no lifetime) in instruction args and returns `Some((T, N))`.
pub(crate) fn is_ix_dynamic_vec(ty: &Type) -> Option<(Type, usize)> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    let elem_ty = match iter.next() {
                        Some(GenericArgument::Type(ty)) => ty.clone(),
                        _ => return None,
                    };
                    if let Some(GenericArgument::Const(Expr::Lit(ExprLit {
                        lit: Lit::Int(lit_int),
                        ..
                    }))) = iter.next()
                    {
                        let max = lit_int.base10_parse::<usize>().ok()?;
                        return Some((elem_ty, max));
                    }
                }
            }
        }
    }
    None
}

// --- Zc (zero-copy) companion struct helpers ---

pub(crate) fn map_to_pod_type(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let ident_str = seg.ident.to_string();
            return match ident_str.as_str() {
                "u128" => quote! { quasar_core::pod::PodU128 },
                "u64" => quote! { quasar_core::pod::PodU64 },
                "u32" => quote! { quasar_core::pod::PodU32 },
                "u16" => quote! { quasar_core::pod::PodU16 },
                "i128" => quote! { quasar_core::pod::PodI128 },
                "i64" => quote! { quasar_core::pod::PodI64 },
                "i32" => quote! { quasar_core::pod::PodI32 },
                "i16" => quote! { quasar_core::pod::PodI16 },
                "bool" => quote! { quasar_core::pod::PodBool },
                _ => quote! { #ty },
            };
        }
    }
    quote! { #ty }
}

pub(crate) fn zc_serialize_field(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" => quote! { __zc.#field_name = self.#field_name; },
                "bool" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodBool::from(self.#field_name); }
                }
                "u16" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU16::from(self.#field_name); }
                }
                "u32" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU32::from(self.#field_name); }
                }
                "u64" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU64::from(self.#field_name); }
                }
                "u128" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU128::from(self.#field_name); }
                }
                "i16" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI16::from(self.#field_name); }
                }
                "i32" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI32::from(self.#field_name); }
                }
                "i64" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI64::from(self.#field_name); }
                }
                "i128" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI128::from(self.#field_name); }
                }
                _ => quote! { __zc.#field_name = self.#field_name; },
            };
        }
    }
    quote! { __zc.#field_name = self.#field_name; }
}

pub(crate) fn zc_deserialize_expr(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" => quote! { __zc.#field_name },
                "bool" | "u16" | "u32" | "u64" | "u128" | "i16" | "i32" | "i64" | "i128" => {
                    quote! { __zc.#field_name.get() }
                }
                _ => quote! { __zc.#field_name },
            };
        }
    }
    quote! { __zc.#field_name }
}

pub(crate) fn zc_deserialize_field(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    let expr = zc_deserialize_expr(field_name, ty);
    quote! { #field_name: #expr }
}

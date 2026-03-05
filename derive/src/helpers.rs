use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    Expr, ExprLit, GenericArgument, Ident, Lit, LitInt, PathArguments, Token, Type,
};

// --- Dynamic field classification (shared by account, instruction) ---

#[derive(Clone, Copy)]
pub(crate) enum PrefixType {
    U8,
    U16,
    U32,
}

impl PrefixType {
    pub fn bytes(&self) -> usize {
        match self {
            PrefixType::U8 => 1,
            PrefixType::U16 => 2,
            PrefixType::U32 => 4,
        }
    }

    /// Expression to read the inline prefix from `__data` at `__offset` as usize.
    pub fn gen_read_len(&self) -> proc_macro2::TokenStream {
        match self {
            PrefixType::U8 => quote! { __data[__offset] as usize },
            PrefixType::U16 => quote! {
                u16::from_le_bytes([__data[__offset], __data[__offset + 1]]) as usize
            },
            PrefixType::U32 => quote! {
                u32::from_le_bytes([
                    __data[__offset],
                    __data[__offset + 1],
                    __data[__offset + 2],
                    __data[__offset + 3],
                ]) as usize
            },
        }
    }

    /// Statement to write a usize value as the inline prefix to `__data` at `__offset`.
    pub fn gen_write_prefix(
        &self,
        value_expr: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        match self {
            PrefixType::U8 => quote! {
                __data[__offset] = #value_expr as u8;
            },
            PrefixType::U16 => quote! {
                {
                    let __pb = (#value_expr as u16).to_le_bytes();
                    __data[__offset] = __pb[0];
                    __data[__offset + 1] = __pb[1];
                }
            },
            PrefixType::U32 => quote! {
                {
                    let __pb = (#value_expr as u32).to_le_bytes();
                    __data[__offset] = __pb[0];
                    __data[__offset + 1] = __pb[1];
                    __data[__offset + 2] = __pb[2];
                    __data[__offset + 3] = __pb[3];
                }
            },
        }
    }
}

pub(crate) enum TailElement {
    Str,
    Bytes,
}

pub(crate) enum DynKind {
    Fixed,
    Str { prefix: PrefixType, max: usize },
    Vec {
        elem: Box<Type>,
        prefix: PrefixType,
        max: usize,
    },
    Tail { element: TailElement },
}

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

// --- Discriminator validation ---

/// Parse discriminator `LitInt`s into byte values.
pub(crate) fn parse_discriminator_bytes(disc_bytes: &[LitInt]) -> syn::Result<Vec<u8>> {
    disc_bytes
        .iter()
        .map(|lit| {
            lit.base10_parse::<u8>()
                .map_err(|_| syn::Error::new_spanned(lit, "discriminator byte must be 0-255"))
        })
        .collect()
}

/// Parse discriminator bytes and validate that at least one is non-zero.
/// Rejects all-zero discriminators which are indistinguishable from
/// uninitialized account data. Used for `#[account]` only (not instructions).
pub(crate) fn validate_discriminator_not_zero(disc_bytes: &[LitInt]) -> syn::Result<Vec<u8>> {
    let values = parse_discriminator_bytes(disc_bytes)?;
    if values.iter().all(|&b| b == 0) {
        return Err(syn::Error::new_spanned(
            &disc_bytes[0],
            "discriminator must contain at least one non-zero byte; all-zero discriminators are indistinguishable from uninitialized account data",
        ));
    }
    Ok(values)
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

/// Extract the first generic type argument from a named wrapper type.
/// E.g. `extract_generic_inner_type(ty, "Option")` returns `Some(&T)` for `Option<T>`.
pub(crate) fn extract_generic_inner_type<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if last.ident == wrapper {
                if let PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Check if a type is a composite (non-reference, non-Option type with a lifetime parameter).
pub(crate) fn is_composite_type(ty: &Type) -> bool {
    if matches!(ty, Type::Reference(_)) {
        return false;
    }
    if extract_generic_inner_type(ty, "Option").is_some() {
        return false;
    }
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|arg| matches!(arg, GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}

pub(crate) fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
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
        _ => syn::Error::new_spanned(ty, "unsupported field type: expected a path type")
            .to_compile_error(),
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

fn extract_const_usize(arg: &GenericArgument) -> Option<usize> {
    if let GenericArgument::Const(Expr::Lit(ExprLit {
        lit: Lit::Int(lit_int),
        ..
    })) = arg
    {
        lit_int.base10_parse::<usize>().ok()
    } else {
        None
    }
}

fn parse_prefix_type(ty: &Type) -> Option<PrefixType> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" => Some(PrefixType::U8),
                "u16" => Some(PrefixType::U16),
                "u32" => Some(PrefixType::U32),
                _ => None,
            };
        }
    }
    None
}

/// Classifies a type as a dynamic string. Returns `Some((prefix, max))`.
///
/// Handles:
/// - `String` → (U32, 1024) — defaults
/// - `String<P>` → (P, 1024) — custom prefix, default max
/// - `String<P, MAX>` → (P, MAX) — fully specified
/// - `String<'a, MAX>` → (U32, MAX) — backward-compat
pub(crate) fn classify_dynamic_string(ty: &Type) -> Option<(PrefixType, usize)> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "String" {
                return match &seg.arguments {
                    PathArguments::None => Some((PrefixType::U32, 1024)),
                    PathArguments::AngleBracketed(args) => {
                        let mut iter = args.args.iter();
                        match iter.next()? {
                            GenericArgument::Lifetime(_) => {
                                let max = extract_const_usize(iter.next()?)?;
                                Some((PrefixType::U32, max))
                            }
                            GenericArgument::Type(prefix_ty) => {
                                let prefix = parse_prefix_type(prefix_ty)?;
                                match iter.next() {
                                    Some(arg) => {
                                        let max = extract_const_usize(arg)?;
                                        Some((prefix, max))
                                    }
                                    None => Some((prefix, 1024)),
                                }
                            }
                            other => {
                                let max = extract_const_usize(other)?;
                                Some((PrefixType::U32, max))
                            }
                        }
                    }
                    _ => None,
                };
            }
        }
    }
    None
}

/// Classifies a type as a dynamic vec. Returns `Some((elem, prefix, max))`.
///
/// Handles:
/// - `Vec<T>` → (T, U32, 8) — defaults
/// - `Vec<T, P>` → (T, P, 8) — custom prefix, default max
/// - `Vec<T, P, MAX>` → (T, P, MAX) — fully specified
/// - `Vec<'a, T, MAX>` → (T, U32, MAX) — backward-compat
pub(crate) fn classify_dynamic_vec(ty: &Type) -> Option<(Type, PrefixType, usize)> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    let first = iter.next()?;

                    if let GenericArgument::Lifetime(_) = first {
                        let elem_ty = match iter.next()? {
                            GenericArgument::Type(ty) => ty.clone(),
                            _ => return None,
                        };
                        return match iter.next() {
                            Some(arg) => {
                                let max = extract_const_usize(arg)?;
                                Some((elem_ty, PrefixType::U32, max))
                            }
                            None => Some((elem_ty, PrefixType::U32, 8)),
                        };
                    }

                    let elem_ty = match first {
                        GenericArgument::Type(ty) => ty.clone(),
                        _ => return None,
                    };

                    return match iter.next() {
                        None => Some((elem_ty, PrefixType::U32, 8)),
                        Some(GenericArgument::Type(prefix_ty)) => {
                            let prefix = parse_prefix_type(prefix_ty)?;
                            match iter.next() {
                                Some(arg) => {
                                    let max = extract_const_usize(arg)?;
                                    Some((elem_ty, prefix, max))
                                }
                                None => Some((elem_ty, prefix, 8)),
                            }
                        }
                        Some(arg) => {
                            let max = extract_const_usize(arg)?;
                            Some((elem_ty, PrefixType::U32, max))
                        }
                    };
                } else {
                    return None;
                }
            }
        }
    }
    None
}

/// Classifies bare `&str` / `&'a str` and `&[u8]` / `&'a [u8]` as tail fields.
///
/// Tail fields have no length prefix — remaining account/instruction data IS the field.
/// Must be the last dynamic field in the struct.
pub(crate) fn classify_tail(ty: &Type) -> Option<TailElement> {
    if let Type::Reference(ref_ty) = ty {
        match &*ref_ty.elem {
            Type::Path(type_path) => {
                if let Some(seg) = type_path.path.segments.last() {
                    if seg.ident == "str" && type_path.path.segments.len() == 1 {
                        return Some(TailElement::Str);
                    }
                }
            }
            Type::Slice(slice_ty) => {
                if let Type::Path(type_path) = &*slice_ty.elem {
                    if let Some(seg) = type_path.path.segments.last() {
                        if seg.ident == "u8" && type_path.path.segments.len() == 1 {
                            return Some(TailElement::Bytes);
                        }
                    }
                }
            }
            _ => {}
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

fn zc_assign_expr(
    field_name: &Ident,
    ty: &Type,
    value: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" => quote! { __zc.#field_name = #value; },
                "bool" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodBool::from(#value); }
                }
                "u16" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU16::from(#value); }
                }
                "u32" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU32::from(#value); }
                }
                "u64" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU64::from(#value); }
                }
                "u128" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodU128::from(#value); }
                }
                "i16" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI16::from(#value); }
                }
                "i32" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI32::from(#value); }
                }
                "i64" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI64::from(#value); }
                }
                "i128" => {
                    quote! { __zc.#field_name = quasar_core::pod::PodI128::from(#value); }
                }
                _ => quote! { __zc.#field_name = #value; },
            };
        }
    }
    quote! { __zc.#field_name = #value; }
}

pub(crate) fn zc_assign_from_value(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    zc_assign_expr(field_name, ty, quote! { #field_name })
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


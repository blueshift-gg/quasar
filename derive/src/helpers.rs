pub(crate) use quasar_schema::{pascal_to_snake, snake_to_pascal};
use {
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, GenericArgument, Ident, Lit, LitInt, PathArguments, Token, Type,
    },
};

pub(crate) struct AccountAttr {
    pub disc_bytes: Vec<LitInt>,
    pub unsafe_no_disc: bool,
    pub set_inner: bool,
    pub fixed_capacity: bool,
}

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut disc_bytes = Vec::new();
        let mut unsafe_no_disc = false;
        let mut set_inner = false;
        let mut fixed_capacity = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "unsafe_no_disc" {
                unsafe_no_disc = true;
            } else if ident == "set_inner" {
                set_inner = true;
            } else if ident == "fixed_capacity" {
                fixed_capacity = true;
            } else if ident == "discriminator" {
                disc_bytes = parse_discriminator_value(input)?;
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator`, `unsafe_no_disc`, `set_inner`, or `fixed_capacity`",
                ));
            }
            let _ = input.parse::<Option<Token![,]>>();
        }

        if disc_bytes.is_empty() && !unsafe_no_disc {
            return Err(syn::Error::new(
                input.span(),
                "expected `discriminator` or `unsafe_no_disc`",
            ));
        }

        Ok(Self {
            disc_bytes,
            unsafe_no_disc,
            set_inner,
            fixed_capacity,
        })
    }
}

pub(crate) struct InstructionArgs {
    pub discriminator: Option<Vec<LitInt>>,
    pub heap: bool,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut discriminator = None;
        let mut heap = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "heap" {
                heap = true;
            } else if ident == "discriminator" {
                discriminator = Some(parse_discriminator_value(input)?);
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator` or `heap`",
                ));
            }
            let _ = input.parse::<Option<Token![,]>>();
        }

        Ok(Self {
            discriminator,
            heap,
        })
    }
}

fn parse_discriminator_value(input: ParseStream) -> syn::Result<Vec<LitInt>> {
    let _: Token![=] = input.parse()?;
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let lits = content.parse_terminated(LitInt::parse, Token![,])?;
        let disc_bytes: Vec<LitInt> = lits.into_iter().collect();
        if disc_bytes.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "discriminator must have at least one byte",
            ));
        }
        Ok(disc_bytes)
    } else {
        let lit: LitInt = input.parse()?;
        Ok(vec![lit])
    }
}

pub(crate) fn parse_discriminator_bytes(disc_bytes: &[LitInt]) -> syn::Result<Vec<u8>> {
    disc_bytes
        .iter()
        .map(|lit| {
            lit.base10_parse::<u8>()
                .map_err(|_| syn::Error::new_spanned(lit, "discriminator byte must be 0-255"))
        })
        .collect()
}

pub(crate) fn validate_discriminator_not_zero(disc_bytes: &[LitInt]) -> syn::Result<Vec<u8>> {
    let values = parse_discriminator_bytes(disc_bytes)?;
    if values.iter().all(|&b| b == 0) {
        return Err(syn::Error::new_spanned(
            &disc_bytes[0],
            "discriminator must contain at least one non-zero byte; all-zero discriminators are \
             indistinguishable from uninitialized account data",
        ));
    }
    Ok(values)
}

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

pub(crate) enum PodDynField {
    Str {
        max: usize,
        prefix_bytes: usize,
    },
    Vec {
        elem: Box<Type>,
        max: usize,
        prefix_bytes: usize,
    },
}

pub(crate) fn classify_lifetime_arg(ty: &Type) -> bool {
    use syn::{GenericArgument, PathArguments};
    if let Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|a| matches!(a, GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}

fn parse_prefix_arg(arg: &GenericArgument) -> Option<usize> {
    match arg {
        GenericArgument::Type(Type::Path(type_path)) => {
            if let Some(seg) = type_path.path.segments.last() {
                if seg.ident == "u8" {
                    Some(1)
                } else if seg.ident == "u16" {
                    Some(2)
                } else if seg.ident == "u32" {
                    Some(4)
                } else if seg.ident == "u64" {
                    Some(8)
                } else {
                    None
                }
            } else {
                None
            }
        }
        GenericArgument::Const(Expr::Lit(ExprLit {
            lit: Lit::Int(n), ..
        })) => n.base10_parse::<usize>().ok(),
        _ => None,
    }
}

pub(crate) fn classify_pod_string(ty: &Type) -> Option<PodDynField> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if (seg.ident == "PodString" || seg.ident == "String")
                && type_path.path.segments.len() == 1
            {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    let max = extract_const_usize(iter.next()?)?;
                    let prefix_bytes = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
                    return Some(PodDynField::Str { max, prefix_bytes });
                }
            }
        }
    }
    None
}

pub(crate) fn classify_pod_vec(ty: &Type) -> Option<PodDynField> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if (seg.ident == "PodVec" || seg.ident == "Vec") && type_path.path.segments.len() == 1 {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    let mut iter = args.args.iter();
                    let elem = match iter.next()? {
                        GenericArgument::Type(ty) => ty.clone(),
                        _ => return None,
                    };
                    let max = extract_const_usize(iter.next()?)?;
                    let prefix_bytes = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
                    return Some(PodDynField::Vec {
                        elem: Box::new(elem),
                        max,
                        prefix_bytes,
                    });
                }
            }
        }
    }
    None
}

pub(crate) fn classify_pod_dynamic(ty: &Type) -> Option<PodDynField> {
    classify_pod_string(ty).or_else(|| classify_pod_vec(ty))
}

/// Classify a borrowed reference type as a compact schema field.
/// `&str` maps to PodDynField::Str, `&[T]` maps to PodDynField::Vec.
/// Returns None if the type is not a supported reference type.
pub(crate) fn classify_borrowed_as_compact(
    ty: &Type,
    max_n: usize,
    pfx_override: usize,
) -> Option<PodDynField> {
    if let Type::Reference(ref_ty) = ty {
        if matches!(&*ref_ty.elem, Type::Path(tp) if tp.path.is_ident("str")) {
            let pfx = if pfx_override == 0 { 1 } else { pfx_override };
            return Some(PodDynField::Str {
                max: max_n,
                prefix_bytes: pfx,
            });
        }
        if let Type::Slice(s) = &*ref_ty.elem {
            let pfx = if pfx_override == 0 { 2 } else { pfx_override };
            return Some(PodDynField::Vec {
                elem: Box::new((*s.elem).clone()),
                max: max_n,
                prefix_bytes: pfx,
            });
        }
    }
    None
}

pub(crate) fn prefix_bytes_to_rust_type(prefix_bytes: usize) -> proc_macro2::TokenStream {
    match prefix_bytes {
        1 => quote! { u8 },
        2 => quote! { u16 },
        4 => quote! { u32 },
        8 => quote! { u64 },
        _ => quote! { u16 },
    }
}

/// Map a PodDynField to the zeropod compact field type tokens.
/// Used by both #[account] and #[instruction] codegen.
pub(crate) fn pod_dyn_to_compact_type(dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => {
            quote! { zeropod::pod::PodString<#max, #prefix_bytes> }
        }
        PodDynField::Vec {
            elem,
            max,
            prefix_bytes,
        } => {
            quote! { zeropod::pod::PodVec<#elem, #max, #prefix_bytes> }
        }
    }
}

pub(crate) fn map_to_pod_type(ty: &Type) -> proc_macro2::TokenStream {
    pod_alias_type(ty, true)
        .unwrap_or_else(|| quote! { <#ty as quasar_lang::instruction_arg::InstructionArg>::Zc })
}

pub(crate) fn canonical_instruction_arg_type(ty: &Type) -> proc_macro2::TokenStream {
    pod_alias_type(ty, false).unwrap_or_else(|| quote! { #ty })
}

pub(crate) fn zc_assign_from_value(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    let canonical = canonical_instruction_arg_type(ty);
    quote! {
        __zc.#field_name =
            <#canonical as quasar_lang::instruction_arg::InstructionArg>::to_zc(&#field_name);
    }
}

fn pod_alias_type(ty: &Type, accept_pod_aliases: bool) -> Option<proc_macro2::TokenStream> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let is_string = seg.ident == "String" || seg.ident == "PodString" && accept_pod_aliases;
            let is_vec = seg.ident == "Vec" || seg.ident == "PodVec" && accept_pod_aliases;

            if is_string {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    let mut it = ab.args.iter();
                    if let Some(n_arg) = it.next() {
                        let pfx: usize = it.next().and_then(parse_prefix_arg).unwrap_or(1);
                        return Some(quote! { quasar_lang::pod::PodString<#n_arg, #pfx> });
                    }
                }
                if accept_pod_aliases {
                    return Some(quote! { quasar_lang::pod::PodString });
                }
            } else if is_vec {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    let mut it = ab.args.iter();
                    if let (Some(t_arg), Some(n_arg)) = (it.next(), it.next()) {
                        let pfx: usize = it.next().and_then(parse_prefix_arg).unwrap_or(2);
                        return Some(quote! { quasar_lang::pod::PodVec<#t_arg, #n_arg, #pfx> });
                    }
                }
                if accept_pod_aliases {
                    return Some(quote! { quasar_lang::pod::PodVec });
                }
            }
        }
    }
    None
}

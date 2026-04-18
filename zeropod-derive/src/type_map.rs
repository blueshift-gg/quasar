use {
    proc_macro2::TokenStream,
    quote::quote,
    syn::{Expr, ExprLit, GenericArgument, Lit, PathArguments, Type},
};

// ---------------------------------------------------------------------------
// Field classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FieldKind {
    Inline,
    Tail(TailField),
}

#[derive(Debug, Clone)]
pub enum TailField {
    String {
        max: usize,
        pfx: usize,
    },
    Vec {
        elem: Box<Type>,
        max: usize,
        pfx: usize,
    },
}

pub fn classify_field(ty: &Type) -> FieldKind {
    if let Some(tail) = classify_string(ty) {
        return FieldKind::Tail(tail);
    }
    if let Some(tail) = classify_vec(ty) {
        return FieldKind::Tail(tail);
    }
    FieldKind::Inline
}

fn classify_string(ty: &Type) -> Option<TailField> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "String" && seg.ident != "PodString" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let max = extract_const_usize(iter.next()?)?;
    let pfx = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
    Some(TailField::String { max, pfx })
}

fn classify_vec(ty: &Type) -> Option<TailField> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Vec" && seg.ident != "PodVec" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let elem = match iter.next()? {
        GenericArgument::Type(t) => t.clone(),
        _ => return None,
    };
    let max = extract_const_usize(iter.next()?)?;
    let pfx = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
    Some(TailField::Vec {
        elem: Box::new(elem),
        max,
        pfx,
    })
}

// ---------------------------------------------------------------------------
// Type mapping: schema type → pod storage type
// ---------------------------------------------------------------------------

pub fn map_to_pod_type(ty: &Type) -> TokenStream {
    // 1. Primitives that are already align-1
    if let Some(ts) = try_primitive(ty) {
        return ts;
    }

    // 2. String / PodString
    if let Some(ts) = try_map_string(ty) {
        return ts;
    }

    // 3. Vec / PodVec
    if let Some(ts) = try_map_vec(ty) {
        return ts;
    }

    // 4. Option
    if let Some(ts) = try_map_option(ty) {
        return ts;
    }

    // 5. Array types → keep as-is
    if matches!(ty, Type::Array(_)) {
        return quote! { #ty };
    }

    // 6. Fallback: delegate via ZcField trait
    quote! { <#ty as zeropod::ZcField>::Pod }
}

fn try_primitive(ty: &Type) -> Option<TokenStream> {
    let seg = single_path_segment(ty)?;
    if !seg.arguments.is_none() {
        return None;
    }
    let name = seg.ident.to_string();
    match name.as_str() {
        "u8" => Some(quote! { u8 }),
        "i8" => Some(quote! { i8 }),
        "u16" => Some(quote! { zeropod::pod::PodU16 }),
        "u32" => Some(quote! { zeropod::pod::PodU32 }),
        "u64" => Some(quote! { zeropod::pod::PodU64 }),
        "u128" => Some(quote! { zeropod::pod::PodU128 }),
        "i16" => Some(quote! { zeropod::pod::PodI16 }),
        "i32" => Some(quote! { zeropod::pod::PodI32 }),
        "i64" => Some(quote! { zeropod::pod::PodI64 }),
        "i128" => Some(quote! { zeropod::pod::PodI128 }),
        "bool" => Some(quote! { zeropod::pod::PodBool }),
        _ => None,
    }
}

fn try_map_string(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "String" && seg.ident != "PodString" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
    Some(quote! { zeropod::pod::PodString<#n_arg, #pfx> })
}

fn try_map_vec(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Vec" && seg.ident != "PodVec" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let t_arg = match iter.next()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
    let mapped_t = map_to_pod_type(t_arg);
    Some(quote! { zeropod::pod::PodVec<#mapped_t, #n_arg, #pfx> })
}

fn try_map_option(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Option" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let inner = match args.first()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let mapped_inner = map_to_pod_type(inner);
    Some(quote! { zeropod::pod::PodOption<#mapped_inner> })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn single_path_segment(ty: &Type) -> Option<&syn::PathSegment> {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            return type_path.path.segments.last();
        }
    }
    None
}

/// Like `single_path_segment`, but also matches the *last* segment
/// of a multi-segment path (e.g. `zeropod::String<32>` → `String<32>`).
fn last_path_segment(ty: &Type) -> Option<&syn::PathSegment> {
    if let Type::Path(type_path) = ty {
        return type_path.path.segments.last();
    }
    None
}

fn angle_args(
    arguments: &PathArguments,
) -> Option<&syn::punctuated::Punctuated<GenericArgument, syn::token::Comma>> {
    if let PathArguments::AngleBracketed(ab) = arguments {
        Some(&ab.args)
    } else {
        None
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

fn parse_prefix_arg(arg: &GenericArgument) -> Option<usize> {
    match arg {
        GenericArgument::Type(Type::Path(type_path)) => {
            let seg = type_path.path.segments.last()?;
            match seg.ident.to_string().as_str() {
                "u8" => Some(1),
                "u16" => Some(2),
                "u32" => Some(4),
                "u64" => Some(8),
                _ => None,
            }
        }
        GenericArgument::Const(Expr::Lit(ExprLit {
            lit: Lit::Int(n), ..
        })) => n.base10_parse::<usize>().ok(),
        _ => None,
    }
}

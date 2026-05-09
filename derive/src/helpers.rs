pub(crate) use quasar_schema::{pascal_to_snake, snake_to_pascal};

/// Convert snake_case to camelCase (first word lowercase, subsequent words
/// capitalized).
pub(crate) fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    for (i, part) in s.split('_').enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            result.push_str(part);
        } else {
            let mut chars = part.chars();
            if let Some(c) = chars.next() {
                result.push(c.to_ascii_uppercase());
                result.extend(chars);
            }
        }
    }
    result
}
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
    /// `custom` — transparent wrapper over AccountView with user-provided
    /// check().
    pub custom: bool,
    /// `one_of` — polymorphic account on enum declarations.
    pub one_of: bool,
    /// `implements(TraitPath)` — trait all variants implement; generates Deref.
    pub implements: Option<syn::Path>,
}

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut disc_bytes = Vec::new();
        let mut unsafe_no_disc = false;
        let mut set_inner = false;
        let mut fixed_capacity = false;
        let mut custom = false;
        let mut one_of = false;
        let mut implements: Option<syn::Path> = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "unsafe_no_disc" {
                unsafe_no_disc = true;
            } else if ident == "set_inner" {
                set_inner = true;
            } else if ident == "fixed_capacity" {
                fixed_capacity = true;
            } else if ident == "custom" {
                custom = true;
            } else if ident == "one_of" {
                one_of = true;
            } else if ident == "discriminator" {
                disc_bytes = parse_discriminator_value(input)?;
            } else if ident == "implements" {
                let content;
                syn::parenthesized!(content in input);
                implements = Some(content.parse::<syn::Path>()?);
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator`, `unsafe_no_disc`, `set_inner`, `fixed_capacity`, \
                     `custom`, `one_of`, or `implements`",
                ));
            }
            let _ = input.parse::<Option<Token![,]>>();
        }

        // custom accounts don't have discriminators
        if custom && (!disc_bytes.is_empty() || unsafe_no_disc || set_inner || fixed_capacity) {
            return Err(syn::Error::new(
                input.span(),
                "`custom` cannot be combined with `discriminator`, `unsafe_no_disc`, `set_inner`, \
                 or `fixed_capacity`",
            ));
        }

        if !custom && !one_of && disc_bytes.is_empty() && !unsafe_no_disc {
            return Err(syn::Error::new(
                input.span(),
                "expected `discriminator` or `unsafe_no_disc`",
            ));
        }

        if implements.is_some() && !one_of {
            return Err(syn::Error::new(
                input.span(),
                "`implements` can only be used with `one_of`",
            ));
        }

        // one_of doesn't have its own discriminator
        if one_of && (!disc_bytes.is_empty() || unsafe_no_disc) {
            return Err(syn::Error::new(
                input.span(),
                "`one_of` cannot be combined with `discriminator` or `unsafe_no_disc`",
            ));
        }

        Ok(Self {
            disc_bytes,
            unsafe_no_disc,
            set_inner,
            fixed_capacity,
            custom,
            one_of,
            implements,
        })
    }
}

pub(crate) struct InstructionArgs {
    pub discriminator: Option<Vec<LitInt>>,
    pub heap: bool,
    pub raw: bool,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut discriminator = None;
        let mut heap = false;
        let mut raw = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "heap" {
                heap = true;
            } else if ident == "raw" {
                raw = true;
            } else if ident == "discriminator" {
                discriminator = Some(parse_discriminator_value(input)?);
            } else if ident == "pre_hook" || ident == "post_hook" {
                return Err(syn::Error::new(
                    ident.span(),
                    "pre_hook and post_hook have been removed. Attach a behavior check group to \
                     the relevant account field, or move logic into the handler body.",
                ));
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator`, `heap`, or `raw`",
                ));
            }
            let _ = input.parse::<Option<Token![,]>>();
        }

        Ok(Self {
            discriminator,
            heap,
            raw,
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

fn extract_const_expr(arg: &GenericArgument) -> Option<Expr> {
    match arg {
        GenericArgument::Const(expr) => Some(expr.clone()),
        GenericArgument::Type(Type::Path(type_path))
            if type_path.qself.is_none()
                && type_path.path.leading_colon.is_none()
                && type_path.path.segments.len() == 1 =>
        {
            let ident = &type_path.path.segments[0].ident;
            Some(syn::parse_quote!(#ident))
        }
        _ => None,
    }
}

pub(crate) enum PodDynField {
    Str {
        max: Expr,
        prefix_bytes: usize,
    },
    Vec {
        elem: Box<Type>,
        max: Expr,
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
                    let max = extract_const_expr(iter.next()?)?;
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
                    let max = extract_const_expr(iter.next()?)?;
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

/// Returns the dynamic inner field if the type is `Option<T>` where T is a
/// dynamic pod type.
pub(crate) fn classify_option_pod_dynamic(ty: &Type) -> Option<PodDynField> {
    if let Some(inner) = extract_generic_inner_type(ty, "Option") {
        classify_pod_dynamic(inner)
    } else {
        None
    }
}

/// Returns true if the type is `Option<T>` where T is a dynamic pod type.
pub(crate) fn classify_option_dynamic(ty: &Type) -> bool {
    classify_option_pod_dynamic(ty).is_some()
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
                max: syn::parse_quote!(#max_n),
                prefix_bytes: pfx,
            });
        }
        if let Type::Slice(s) = &*ref_ty.elem {
            let pfx = if pfx_override == 0 { 2 } else { pfx_override };
            return Some(PodDynField::Vec {
                elem: Box::new((*s.elem).clone()),
                max: syn::parse_quote!(#max_n),
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
            } else if seg.ident == "PodOption" {
                // PodOption<T, PFX> — map inner type, pass PFX through.
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    let mut it = ab.args.iter();
                    if let Some(syn::GenericArgument::Type(inner)) = it.next() {
                        let mapped = pod_alias_type(inner, accept_pod_aliases)
                            .unwrap_or_else(|| quote! { #inner });
                        let pfx = it.next();
                        return match pfx {
                            Some(pfx_arg) => {
                                Some(quote! { quasar_lang::pod::PodOption<#mapped, #pfx_arg> })
                            }
                            None => Some(quote! { quasar_lang::pod::PodOption<#mapped> }),
                        };
                    }
                }
            } else if seg.ident == "Option" {
                // Recursively unwrap Option<T> and apply pod alias to inner type.
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                        if let Some(inner_pod) = pod_alias_type(inner, accept_pod_aliases) {
                            return Some(quote! { Option<#inner_pod> });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Convert a Rust type to a `proc_macro2::TokenStream` that constructs an
/// `Option<IdlCodec>` at runtime (used by IDL fragment emission).
///
/// Returns `None` for fixed types (inferred), and `Some(IdlCodec::SizePrefixed
/// { .. })` for dynamic types (PodString, PodVec, String, Vec with const
/// generics).
pub(crate) fn type_to_idl_codec_tokens(ty: &Type) -> proc_macro2::TokenStream {
    if let Some(dyn_field) = classify_pod_dynamic(ty) {
        match dyn_field {
            PodDynField::Str { max, prefix_bytes } => {
                let pfx_ty_str = match prefix_bytes {
                    1 => "u8",
                    2 => "u16",
                    4 => "u32",
                    8 => "u64",
                    _ => "u8",
                };
                return quote! {
                    Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed {
                        prefix: quasar_lang::idl_build::__reexport::ScalarRepr {
                            ty: quasar_lang::idl_build::s(#pfx_ty_str),
                            endian: quasar_lang::idl_build::__reexport::Endian::Le,
                        },
                        storage: quasar_lang::idl_build::__reexport::Storage::Tail,
                        max_bytes: Some(#max),
                        max_items: None,
                        encoding: Some(quasar_lang::idl_build::s("utf8")),
                        item: None,
                    })
                };
            }
            PodDynField::Vec {
                max, prefix_bytes, ..
            } => {
                let pfx_ty_str = match prefix_bytes {
                    1 => "u8",
                    2 => "u16",
                    4 => "u32",
                    8 => "u64",
                    _ => "u16",
                };
                return quote! {
                    Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed {
                        prefix: quasar_lang::idl_build::__reexport::ScalarRepr {
                            ty: quasar_lang::idl_build::s(#pfx_ty_str),
                            endian: quasar_lang::idl_build::__reexport::Endian::Le,
                        },
                        storage: quasar_lang::idl_build::__reexport::Storage::Tail,
                        max_bytes: None,
                        max_items: Some(#max),
                        encoding: None,
                        item: None,
                    })
                };
            }
        }
    }
    // Check if it's Option<dynamic_type> — the option-wrapped dynamic field is also
    // dynamic
    if let Some(inner) = extract_generic_inner_type(ty, "Option") {
        if let Some(dyn_field) = classify_pod_dynamic(inner) {
            match dyn_field {
                PodDynField::Str { max, prefix_bytes } => {
                    let pfx_ty_str = match prefix_bytes {
                        1 => "u8",
                        2 => "u16",
                        4 => "u32",
                        8 => "u64",
                        _ => "u8",
                    };
                    return quote! {
                        Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed {
                            prefix: quasar_lang::idl_build::__reexport::ScalarRepr {
                                ty: quasar_lang::idl_build::s(#pfx_ty_str),
                                endian: quasar_lang::idl_build::__reexport::Endian::Le,
                            },
                            storage: quasar_lang::idl_build::__reexport::Storage::Tail,
                            max_bytes: Some(#max),
                            max_items: None,
                            encoding: Some(quasar_lang::idl_build::s("utf8")),
                            item: None,
                        })
                    };
                }
                PodDynField::Vec {
                    max, prefix_bytes, ..
                } => {
                    let pfx_ty_str = match prefix_bytes {
                        1 => "u8",
                        2 => "u16",
                        4 => "u32",
                        8 => "u64",
                        _ => "u16",
                    };
                    return quote! {
                        Some(quasar_lang::idl_build::__reexport::IdlCodec::SizePrefixed {
                            prefix: quasar_lang::idl_build::__reexport::ScalarRepr {
                                ty: quasar_lang::idl_build::s(#pfx_ty_str),
                                endian: quasar_lang::idl_build::__reexport::Endian::Le,
                            },
                            storage: quasar_lang::idl_build::__reexport::Storage::Tail,
                            max_bytes: None,
                            max_items: Some(#max),
                            encoding: None,
                            item: None,
                        })
                    };
                }
            }
        }
    }
    // Fixed types: codec is inferred, return None
    quote! { None }
}

/// Convert a Rust type to a `proc_macro2::TokenStream` that constructs an
/// `IdlType` at runtime (used by IDL fragment emission).
pub(crate) fn type_to_idl_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Array(array) = ty {
        let inner_tokens = type_to_idl_type_tokens(&array.elem);
        let len = &array.len;
        return quote! {
            quasar_lang::idl_build::__reexport::IdlType::Array {
                array: (
                    quasar_lang::idl_build::Box::new(#inner_tokens),
                    #len,
                ),
            }
        };
    }

    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            let primitive = match name.as_str() {
                "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128" | "i128"
                | "bool" | "f32" | "f64" => Some(name.clone()),
                "Address" | "Pubkey" => Some("pubkey".to_owned()),
                _ => None,
            };
            if let Some(prim) = primitive {
                return quote! {
                    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s(#prim))
                };
            }
            // Option<T>
            if name == "Option" {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        let inner_tokens = type_to_idl_type_tokens(inner);
                        return quote! {
                            quasar_lang::idl_build::__reexport::IdlType::Option {
                                option: quasar_lang::idl_build::Box::new(#inner_tokens),
                            }
                        };
                    }
                }
            }
            // Vec<T> / PodVec<T, N>
            if name == "Vec" || name == "PodVec" {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        let inner_tokens = type_to_idl_type_tokens(inner);
                        return quote! {
                            quasar_lang::idl_build::__reexport::IdlType::Vec {
                                vec: quasar_lang::idl_build::Box::new(#inner_tokens),
                            }
                        };
                    }
                }
            }
            // String / PodString
            if name == "String" || name == "PodString" {
                return quote! {
                    quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("string"))
                };
            }
            // Fall back to defined type reference
            return quote! {
                quasar_lang::idl_build::__reexport::IdlType::Defined {
                    defined: quasar_lang::idl_build::__reexport::IdlDefinedRef {
                        name: quasar_lang::idl_build::s(#name),
                        generics: quasar_lang::idl_build::Vec::new(),
                    },
                }
            };
        }
    }
    // Fallback: opaque bytes
    quote! {
        quasar_lang::idl_build::__reexport::IdlType::Primitive(quasar_lang::idl_build::s("bytes"))
    }
}

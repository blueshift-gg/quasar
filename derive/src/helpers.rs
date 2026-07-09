pub(crate) use quasar_schema::{pascal_to_snake, snake_to_pascal, to_camel_case as snake_to_camel};
use {
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, GenericArgument, Ident, Lit, LitInt, PathArguments, Token, Type,
    },
};

fn duplicate_arg_error(ident: &Ident) -> syn::Error {
    syn::Error::new(ident.span(), format!("duplicate `{ident}`"))
}

/// Parse `#[max(N)]` or `#[max(N, pfx = P)]` from an attribute list.
pub(crate) fn parse_max_attr(attrs: &[syn::Attribute]) -> Option<syn::Result<(usize, usize)>> {
    for attr in attrs {
        if attr.path().is_ident("max") {
            return Some(attr.parse_args_with(|stream: syn::parse::ParseStream| {
                let n: LitInt = stream.parse()?;
                let max_n: usize = n.base10_parse()?;
                let mut pfx = 0usize;
                if !stream.is_empty() {
                    let _: Token![,] = stream.parse()?;
                    let key: Ident = stream.parse()?;
                    if key != "pfx" {
                        return Err(syn::Error::new(key.span(), "expected `pfx`"));
                    }
                    let _: Token![=] = stream.parse()?;
                    let p: LitInt = stream.parse()?;
                    pfx = p.base10_parse()?;
                    if !matches!(pfx, 1 | 2 | 4 | 8) {
                        return Err(syn::Error::new(
                            p.span(),
                            "length-prefix width `pfx` must be `1`, `2`, `4`, or `8`",
                        ));
                    }
                }
                Ok((max_n, pfx))
            }));
        }
    }
    None
}

pub(crate) struct AccountAttr {
    pub disc_bytes: Vec<LitInt>,
    pub unsafe_no_disc: bool,
    pub set_inner: bool,
    pub fixed_capacity: bool,
    /// `one_of`: polymorphic account on enum declarations.
    pub one_of: bool,
    /// `implements(TraitPath)`: trait all variants implement; generates Deref.
    pub implements: Option<syn::Path>,
}

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut disc_bytes = Vec::new();
        let mut unsafe_no_disc = false;
        let mut set_inner = false;
        let mut fixed_capacity = false;
        let mut one_of = false;
        let mut implements: Option<syn::Path> = None;
        let mut has_discriminator = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "unsafe_no_disc" {
                if unsafe_no_disc {
                    return Err(duplicate_arg_error(&ident));
                }
                unsafe_no_disc = true;
            } else if ident == "set_inner" {
                if set_inner {
                    return Err(duplicate_arg_error(&ident));
                }
                set_inner = true;
            } else if ident == "fixed_capacity" {
                if fixed_capacity {
                    return Err(duplicate_arg_error(&ident));
                }
                fixed_capacity = true;
            } else if ident == "one_of" {
                if one_of {
                    return Err(duplicate_arg_error(&ident));
                }
                one_of = true;
            } else if ident == "discriminator" {
                if has_discriminator {
                    return Err(duplicate_arg_error(&ident));
                }
                disc_bytes = parse_discriminator_value(input)?;
                has_discriminator = true;
            } else if ident == "implements" {
                if implements.is_some() {
                    return Err(duplicate_arg_error(&ident));
                }
                let content;
                syn::parenthesized!(content in input);
                implements = Some(content.parse::<syn::Path>()?);
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator`, `unsafe_no_disc`, `set_inner`, `fixed_capacity`, \
                     `one_of`, or `implements`",
                ));
            }
            let _ = input.parse::<Option<Token![,]>>();
        }

        if !one_of && disc_bytes.is_empty() && !unsafe_no_disc {
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

        if !one_of && has_discriminator && unsafe_no_disc {
            return Err(syn::Error::new(
                input.span(),
                "`discriminator` cannot be combined with `unsafe_no_disc`",
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
                if heap {
                    return Err(duplicate_arg_error(&ident));
                }
                heap = true;
            } else if ident == "raw" {
                if raw {
                    return Err(duplicate_arg_error(&ident));
                }
                raw = true;
            } else if ident == "discriminator" {
                if discriminator.is_some() {
                    return Err(duplicate_arg_error(&ident));
                }
                discriminator = Some(parse_discriminator_value(input)?);
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
    if crate::accounts::resolve::wrapper::classify_wrapper(ty)
        == crate::accounts::resolve::wrapper::WrapperKind::AccountsArray
    {
        return true;
    }
    classify_lifetime_arg(ty)
}

pub(crate) fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}

/// Reduce a path type to its bare segment path, dropping generic arguments
/// (`foo::Bar<'a, T>` -> `foo::Bar`). Returns an error for non-path types so
/// callers can surface a clean diagnostic instead of splicing
/// `to_compile_error()` tokens into a *type* position (which cascades).
pub(crate) fn strip_generics(ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    match ty {
        Type::Path(type_path) => {
            let segments: Vec<_> = type_path
                .path
                .segments
                .iter()
                .map(|seg| &seg.ident)
                .collect();
            Ok(quote! { #(#segments)::* })
        }
        _ => Err(syn::Error::new_spanned(
            ty,
            "unsupported field type: expected a path type",
        )),
    }
}

/// Last path-segment identifier of a type, ignoring the module path and
/// generic arguments (`instructions::Deposit<'a>` -> `"Deposit"`). Falls back
/// to the whitespace-stripped token string for non-path types.
///
/// Used to name the IDL/client fragment for `Ctx<T>`: it must equal the bare
/// accounts-struct ident (`#[derive(Accounts)] struct Deposit`) so the two
/// sides of the join agree, and must never feed a `::`-bearing string into
/// `format_ident!` (which panics).
pub(crate) fn last_type_segment_name(ty: &Type) -> String {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident.to_string();
        }
    }
    quote!(#ty).to_string().replace(' ', "")
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

/// Resolve an explicit length-prefix generic argument to its byte width.
///
/// Returns an error (never a silent default) for anything that is not
/// `u8`/`u16`/`u32`/`u64` or an integer literal `1`/`2`/`4`/`8`.
fn parse_prefix_arg(arg: &GenericArgument) -> syn::Result<usize> {
    let invalid = || {
        syn::Error::new_spanned(
            arg,
            format!(
                "expected `u8`/`u16`/`u32`/`u64` or an integer literal `1`/`2`/`4`/`8` for the \
                 length-prefix width, found `{}`",
                quote!(#arg),
            ),
        )
    };
    match arg {
        GenericArgument::Type(Type::Path(type_path)) => {
            match type_path.path.segments.last().map(|seg| &seg.ident) {
                Some(id) if id == "u8" => Ok(1),
                Some(id) if id == "u16" => Ok(2),
                Some(id) if id == "u32" => Ok(4),
                Some(id) if id == "u64" => Ok(8),
                _ => Err(invalid()),
            }
        }
        GenericArgument::Const(Expr::Lit(ExprLit {
            lit: Lit::Int(n), ..
        })) => match n.base10_parse::<usize>() {
            Ok(v @ (1 | 2 | 4 | 8)) => Ok(v),
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

/// Validate an explicit length-prefix generic argument on
/// `String`/`PodString`/`Vec`/`PodVec` (and `Option<..>` of those), rejecting a
/// prefix that is not `u8`/`u16`/`u32`/`u64` or a literal `1`/`2`/`4`/`8`
/// instead of silently falling back to the default width. A field with no
/// explicit prefix keeps the type's default and is accepted.
pub(crate) fn validate_dynamic_prefix(ty: &Type) -> syn::Result<()> {
    let ty = extract_generic_inner_type(ty, "Option").unwrap_or(ty);
    let Type::Path(tp) = ty else {
        return Ok(());
    };
    if tp.path.segments.len() != 1 {
        return Ok(());
    }
    let Some(seg) = tp.path.segments.last() else {
        return Ok(());
    };
    let PathArguments::AngleBracketed(ab) = &seg.arguments else {
        return Ok(());
    };
    let prefix_arg = if seg.ident == "String" || seg.ident == "PodString" {
        ab.args.iter().nth(1)
    } else if seg.ident == "Vec" || seg.ident == "PodVec" {
        ab.args.iter().nth(2)
    } else {
        return Ok(());
    };
    if let Some(arg) = prefix_arg {
        parse_prefix_arg(arg)?;
    }
    Ok(())
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
                    // An invalid explicit prefix is rejected upstream by
                    // `validate_dynamic_prefix`; the default applies only when
                    // no prefix arg is present.
                    let prefix_bytes = iter
                        .next()
                        .and_then(|a| parse_prefix_arg(a).ok())
                        .unwrap_or(1);
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
                    let prefix_bytes = iter
                        .next()
                        .and_then(|a| parse_prefix_arg(a).ok())
                        .unwrap_or(2);
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

/// Does this instruction arg use the compact (dynamic) wire layout?
///
/// True for `String<N>`/`Vec<T, N>`, `Option<..>` of those, and borrowed
/// `&str`/`&[T]` args -- exactly the classes the `#[instruction]` handler
/// packs into the compact schema (`instruction.rs`). This is the single
/// predicate consumed by both the generated client
/// (`has_compact_client_layout`) and the IDL layout (`has_dynamic`), so all
/// three agree on which args are compact. A grouped borrowed struct
/// (`Foo<'a>`) is a single arg decoded whole and is deliberately *not* flagged.
pub(crate) fn instruction_arg_is_compact(ty: &Type) -> bool {
    classify_pod_dynamic(ty).is_some()
        || classify_option_pod_dynamic(ty).is_some()
        || classify_borrowed_as_compact(ty, 0, 0).is_some()
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
            let is_string =
                seg.ident == "String" || (accept_pod_aliases && seg.ident == "PodString");
            let is_vec = seg.ident == "Vec" || (accept_pod_aliases && seg.ident == "PodVec");

            if is_string {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    let mut it = ab.args.iter();
                    if let Some(n_arg) = it.next() {
                        let pfx: usize = it
                            .next()
                            .and_then(|a| parse_prefix_arg(a).ok())
                            .unwrap_or(1);
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
                        let pfx: usize = it
                            .next()
                            .and_then(|a| parse_prefix_arg(a).ok())
                            .unwrap_or(2);
                        return Some(quote! { quasar_lang::pod::PodVec<#t_arg, #n_arg, #pfx> });
                    }
                }
                if accept_pod_aliases {
                    return Some(quote! { quasar_lang::pod::PodVec });
                }
            } else if seg.ident == "PodOption" {
                // PodOption<T, PFX>: map inner type, pass PFX through.
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
    let dyn_field = classify_pod_dynamic(ty)
        .or_else(|| extract_generic_inner_type(ty, "Option").and_then(classify_pod_dynamic));

    match dyn_field {
        Some(dyn_field) => idl_codec_for_dynamic(dyn_field),
        None => quote! { None },
    }
}

fn idl_codec_for_dynamic(dyn_field: PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => {
            let pfx_ty_str = prefix_type_name(prefix_bytes, "u8");
            quote! {
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
            }
        }
        PodDynField::Vec {
            max, prefix_bytes, ..
        } => {
            let pfx_ty_str = prefix_type_name(prefix_bytes, "u16");
            quote! {
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
            }
        }
    }
}

fn prefix_type_name(prefix_bytes: usize, fallback: &'static str) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        8 => "u64",
        _ => fallback,
    }
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

/// Extract `///` doc-comment lines from attributes, trimmed, in source order.
/// Used to populate the IDL `docs` fields.
pub(crate) fn extract_doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| {
            if let syn::Meta::NameValue(nv) = &a.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    return Some(s.value().trim().to_owned());
                }
            }
            None
        })
        .collect()
}

/// Tokens constructing an IDL `docs` vec from pre-extracted doc lines.
pub(crate) fn docs_tokens_from_lines(lines: &[String]) -> proc_macro2::TokenStream {
    if lines.is_empty() {
        quote! { quasar_lang::idl_build::Vec::new() }
    } else {
        quote! { quasar_lang::idl_build::vec![#(quasar_lang::idl_build::s(#lines)),*] }
    }
}

/// Tokens constructing an IDL `docs` vec from an item's `///` comments.
pub(crate) fn docs_tokens(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    docs_tokens_from_lines(&extract_doc_lines(attrs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_dynamic_prefix_accepts_valid_and_absent() {
        for ty in [
            syn::parse_quote!(String<16>),
            syn::parse_quote!(String<16, u8>),
            syn::parse_quote!(String<16, 2>),
            syn::parse_quote!(PodString<16, u32>),
            syn::parse_quote!(Vec<u8, 16>),
            syn::parse_quote!(Vec<u8, 16, u16>),
            syn::parse_quote!(PodVec<u8, 16, 8>),
            syn::parse_quote!(Option<String<16, u16>>),
            // Non-dynamic / unrelated types are accepted untouched.
            syn::parse_quote!(u64),
            syn::parse_quote!(&str),
        ] {
            let ty: Type = ty;
            assert!(validate_dynamic_prefix(&ty).is_ok(), "{}", quote!(#ty));
        }
    }

    #[test]
    fn compact_predicate_matches_handler_for_borrowed_args() {
        // The client predicate (`has_compact_client_layout`) and the IDL
        // predicate (`has_dynamic`) both consume `instruction_arg_is_compact`;
        // the `#[instruction]` handler classifies a borrowed `&str`/`&[T]`
        // with `#[max]` as a compact arg via `classify_borrowed_as_compact`.
        // All three must agree these are compact.
        for ty in [syn::parse_quote!(&str), syn::parse_quote!(&[u8])] {
            let ty: Type = ty;
            assert!(
                instruction_arg_is_compact(&ty),
                "predicate: {}",
                quote!(#ty)
            );
            assert!(
                classify_borrowed_as_compact(&ty, 32, 0).is_some(),
                "handler: {}",
                quote!(#ty)
            );
            // The pod-dynamic arms alone (the pre-fix predicate) would miss it.
            assert!(classify_pod_dynamic(&ty).is_none());
            assert!(classify_option_pod_dynamic(&ty).is_none());
        }

        // A plain fixed arg is compact under none of the three.
        let fixed: Type = syn::parse_quote!(u64);
        assert!(!instruction_arg_is_compact(&fixed));
        assert!(classify_borrowed_as_compact(&fixed, 32, 0).is_none());

        // A pod-dynamic arg is compact under the predicate (String<N>).
        let s: Type = syn::parse_quote!(String<32>);
        assert!(instruction_arg_is_compact(&s));
    }

    #[test]
    fn validate_dynamic_prefix_rejects_bad_width() {
        for ty in [
            syn::parse_quote!(String<16, f32>),
            syn::parse_quote!(String<16, u128>),
            syn::parse_quote!(String<16, 3>),
            syn::parse_quote!(Vec<u8, 16, f64>),
            syn::parse_quote!(PodVec<u8, 16, 7>),
            syn::parse_quote!(Option<String<16, bool>>),
        ] {
            let ty: Type = ty;
            assert!(validate_dynamic_prefix(&ty).is_err(), "{}", quote!(#ty));
        }
    }
}

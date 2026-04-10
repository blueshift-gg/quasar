//! Shared codegen helpers used across all derive macros.
//!
//! Contains dynamic field classification (String/Vec/Tail), discriminator
//! parsing and validation, type inspection utilities, and zero-copy companion
//! struct helpers for mapping native types to Pod types.

use {
    quote::quote,
    syn::{
        parse::{Parse, ParseStream},
        Expr, ExprLit, GenericArgument, Ident, Lit, LitInt, PathArguments, Token, Type,
    },
};

// --- Dynamic field classification (shared by account, instruction) ---

/// Length-prefix type for dynamic fields (String, Vec).
#[derive(Clone, Copy)]
pub(crate) enum PrefixType {
    U8,
    U16,
    U32,
}

impl PrefixType {
    pub fn bytes(&self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }

    /// Returns the prefix integer as a `syn::Type` token for use in generic
    /// type parameters (e.g. `DynBytes<u8>`).
    pub fn to_type(self) -> syn::Type {
        match self {
            Self::U8 => syn::parse_quote!(u8),
            Self::U16 => syn::parse_quote!(u16),
            Self::U32 => syn::parse_quote!(u32),
        }
    }

    pub fn max_value(self) -> usize {
        match self {
            Self::U8 => u8::MAX as usize,
            Self::U16 => u16::MAX as usize,
            Self::U32 => u32::MAX as usize,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
        }
    }

    /// Expression to read the inline prefix from `__data` at `__offset` as
    /// usize.
    ///
    /// Uses `read_unaligned` for u16/u32 prefixes — SBF is little-endian so
    /// a single unaligned load replaces the multi-instruction byte reassembly
    /// that `from_le_bytes([data[i], data[i+1], ...])` compiles to.
    pub fn gen_read_len(&self) -> proc_macro2::TokenStream {
        match self {
            Self::U8 => quote! { __data[__offset] as usize },
            Self::U16 => quote! {
                unsafe { core::ptr::read_unaligned(__data.as_ptr().add(__offset) as *const u16) } as usize
            },
            Self::U32 => quote! {
                unsafe { core::ptr::read_unaligned(__data.as_ptr().add(__offset) as *const u32) } as usize
            },
        }
    }

    /// Statement to write a usize value as the inline prefix to `__data` at
    /// `__offset`.
    ///
    /// Uses `write_unaligned` for u16/u32 prefixes — single store instead of
    /// the multi-instruction byte decomposition from `to_le_bytes()` + per-byte
    /// writes.
    pub fn gen_write_prefix(
        &self,
        value_expr: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        match self {
            Self::U8 => quote! {
                __data[__offset] = #value_expr as u8;
            },
            Self::U16 => quote! {
                unsafe { core::ptr::write_unaligned(__data.as_mut_ptr().add(__offset) as *mut u16, #value_expr as u16) };
            },
            Self::U32 => quote! {
                unsafe { core::ptr::write_unaligned(__data.as_mut_ptr().add(__offset) as *mut u32, #value_expr as u32) };
            },
        }
    }
}

/// Element type for tail fields (last field consumes remaining data).
#[derive(Clone, Copy)]
pub(crate) enum TailElement {
    /// `&str` — remaining bytes interpreted as UTF-8.
    Str,
    /// `&[u8]` — remaining bytes as a raw slice.
    Bytes,
}

/// Classification of a field's dynamic layout behavior.
pub(crate) enum DynKind {
    Fixed,
    Str {
        prefix: PrefixType,
        max: usize,
    },
    Vec {
        elem: Box<Type>,
        prefix: PrefixType,
        max: usize,
    },
    Tail {
        element: TailElement,
    },
}

impl DynKind {
    /// Project to `DynFieldKind` if this is a dynamic field, `None` if `Fixed`.
    ///
    /// Use with `filter_map` to obtain a collection where every match is
    /// exhaustive over dynamic variants — no `unreachable!()` needed.
    pub(crate) fn as_dynamic(&self) -> Option<DynFieldKind<'_>> {
        match self {
            Self::Str { prefix, max } => Some(DynFieldKind::Str {
                prefix: *prefix,
                max: *max,
            }),
            Self::Vec { elem, prefix, max } => Some(DynFieldKind::Vec {
                elem,
                prefix: *prefix,
                max: *max,
            }),
            Self::Tail { element } => Some(DynFieldKind::Tail { element: *element }),
            Self::Fixed => None,
        }
    }
}

/// Dynamic-only field kind — the type-safe projection of [`DynKind`] after
/// filtering out `Fixed` fields. Matches are exhaustive without
/// `unreachable!()`.
pub(crate) enum DynFieldKind<'a> {
    Str {
        prefix: PrefixType,
        max: usize,
    },
    Vec {
        elem: &'a Type,
        prefix: PrefixType,
        max: usize,
    },
    Tail {
        element: TailElement,
    },
}

impl DynFieldKind<'_> {
    /// Length-prefix type, if this field has one (Str and Vec do, Tail does
    /// not).
    pub(crate) fn prefix(&self) -> Option<PrefixType> {
        match self {
            Self::Str { prefix, .. } | Self::Vec { prefix, .. } => Some(*prefix),
            Self::Tail { .. } => None,
        }
    }

    /// Byte size of the length prefix (0 for Tail fields).
    pub(crate) fn prefix_bytes(&self) -> usize {
        self.prefix().map_or(0, |p| p.bytes())
    }
}

// --- Discriminator argument parsing (shared by instruction, account, event,
// program) ---

/// Parsed `#[account(...)]` attribute arguments.
///
/// Either `discriminator = <bytes>` (standard) or `unsafe_no_disc` (no
/// discriminator — size-only validation, like SPL Token accounts).
pub(crate) struct AccountAttr {
    pub disc_bytes: Vec<LitInt>,
    pub unsafe_no_disc: bool,
    pub set_inner: bool,
}

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut disc_bytes = Vec::new();
        let mut unsafe_no_disc = false;
        let mut set_inner = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "unsafe_no_disc" {
                unsafe_no_disc = true;
            } else if ident == "set_inner" {
                set_inner = true;
            } else if ident == "discriminator" {
                let _: Token![=] = input.parse()?;
                if input.peek(syn::token::Bracket) {
                    let content;
                    syn::bracketed!(content in input);
                    let lits = content.parse_terminated(LitInt::parse, Token![,])?;
                    disc_bytes = lits.into_iter().collect();
                    if disc_bytes.is_empty() {
                        return Err(syn::Error::new(
                            input.span(),
                            "discriminator must have at least one byte",
                        ));
                    }
                } else {
                    let lit: LitInt = input.parse()?;
                    disc_bytes = vec![lit];
                }
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator`, `unsafe_no_disc`, or `set_inner`",
                ));
            }
            // consume optional trailing comma
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
        })
    }
}

/// Parsed `#[instruction(...)]` attribute arguments.
///
/// Supports `discriminator = [...]` and/or `heap` in any order:
/// - `#[instruction(discriminator = [0])]`
/// - `#[instruction(discriminator = [0], heap)]`
/// - `#[instruction(heap, discriminator = [0])]`
/// - `#[instruction(heap)]` (discriminator optional for some contexts)
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
                    discriminator = Some(disc_bytes);
                } else {
                    let lit: LitInt = input.parse()?;
                    discriminator = Some(vec![lit]);
                }
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `discriminator` or `heap`",
                ));
            }
            // consume optional trailing comma
            let _ = input.parse::<Option<Token![,]>>();
        }

        Ok(Self {
            discriminator,
            heap,
        })
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
            "discriminator must contain at least one non-zero byte; all-zero discriminators are \
             indistinguishable from uninitialized account data",
        ));
    }
    Ok(values)
}

// --- Type helpers ---

/// Expand a seed expression into a byte slice for use inside parse (fields are
/// local variables).
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

/// Resolve a typed seed argument to a `&[u8]` expression.
///
/// Typed seeds use `seeds = Type::seeds(arg1, arg2)` syntax. Each argument is
/// resolved by its expression kind:
/// - Bare identifier matching a prior account field -> address bytes
/// - Bare identifier matching an instruction arg -> type-appropriate conversion
/// - Dotted field access (`config.namespace`) -> raw byte cast via
///   `from_raw_parts`
/// - Anything else -> emit as-is, let rustc decide
pub(crate) fn typed_seed_slice_expr(
    expr: &Expr,
    field_names: &[String],
    instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
) -> proc_macro2::TokenStream {
    match expr {
        // Bare identifier
        Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            let ident = &ep.path.segments[0].ident;
            let name = ident.to_string();

            // Account field -> address
            if field_names.contains(&name) {
                return quote! { #ident.to_account_view().address().as_ref() };
            }

            // Instruction arg -> type-appropriate conversion
            if let Some(args) = instruction_args {
                if let Some(arg) = args.iter().find(|a| a.name == *ident) {
                    return ix_arg_to_seed_bytes(&arg.name, &arg.ty);
                }
            }

            // Unknown — emit as-is, rustc will error
            quote! { &#ident as &[u8] }
        }

        // Dotted field access: config.namespace
        // Deserialized account field — on sBPF (little-endian), the in-memory
        // representation of both Pod types and native integers is already LE bytes.
        // This is a zero-cost reference to those bytes.
        Expr::Field(field_expr) => {
            quote! {{
                const _: () = assert!(cfg!(target_endian = "little"), "typed seeds require little-endian");
                unsafe {
                    core::slice::from_raw_parts(
                        &#field_expr as *const _ as *const u8,
                        core::mem::size_of_val(&#field_expr),
                    )
                }
            }}
        }

        // Byte literal or other expression
        _ => quote! { #expr as &[u8] },
    }
}

/// Like `typed_seed_slice_expr`, but for use in seed methods on the Accounts
/// struct. Field access expressions (e.g. `config.namespace`) are prefixed
/// with `self.` so they resolve to the Accounts struct fields.
pub(crate) fn typed_seed_method_expr(
    expr: &Expr,
    field_names: &[String],
    instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
) -> proc_macro2::TokenStream {
    match expr {
        // Bare identifier — should not reach here (handled by caller for
        // account keys and ix args), but if it does, emit with self prefix
        // if it's a field name.
        Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            let ident = &ep.path.segments[0].ident;
            let name = ident.to_string();

            if field_names.contains(&name) {
                return quote! { self.#ident.to_account_view().address().as_ref() };
            }

            if let Some(args) = instruction_args {
                if let Some(arg) = args.iter().find(|a| a.name == *ident) {
                    return ix_arg_to_seed_bytes(&arg.name, &arg.ty);
                }
            }

            quote! { &#ident as &[u8] }
        }

        // Dotted field access: config.namespace -> self.config.namespace
        Expr::Field(field_expr) => {
            quote! {{
                const _: () = assert!(cfg!(target_endian = "little"), "typed seeds require little-endian");
                unsafe {
                    core::slice::from_raw_parts(
                        &self.#field_expr as *const _ as *const u8,
                        core::mem::size_of_val(&self.#field_expr),
                    )
                }
            }}
        }

        _ => quote! { #expr as &[u8] },
    }
}

fn ix_arg_to_seed_bytes(name: &syn::Ident, ty: &Type) -> proc_macro2::TokenStream {
    let type_str = quote!(#ty).to_string().replace(' ', "");
    match type_str.as_str() {
        "u8" => quote! { &[#name] },
        "bool" => quote! { &[#name as u8] },
        "Address" | "Pubkey" => quote! { #name.as_ref() },
        _ => quote! { &#name.to_le_bytes() },
    }
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
/// E.g. `extract_generic_inner_type(ty, "Option")` returns `Some(&T)` for
/// `Option<T>`.
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

/// Check if a type is a composite (non-reference, non-Option type with a
/// lifetime parameter).
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

/// Returns `true` if `ty` is the unit type `()`.
pub(crate) fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}

/// Strips generic arguments from a type path, returning the bare path.
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

/// Converts `PascalCase` to `snake_case` (e.g., `MakeEscrow` → `make_escrow`).
pub(crate) fn pascal_to_snake(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            // Only insert underscore before an uppercase letter when the
            // previous char is lowercase, OR the next char is lowercase
            // (handles acronym runs like "HTTP" → "http" not "h_t_t_p").
            let prev_lower = chars[i - 1].is_lowercase();
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev_lower || next_lower {
                result.push('_');
            }
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

/// Converts `snake_case` to `PascalCase` (e.g., `make_escrow` → `MakeEscrow`).
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
            // Only match unqualified `String` (the Quasar import), not
            // `std::string::String` or `my_crate::String`.
            if seg.ident == "String" && type_path.path.segments.len() == 1 {
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
            if seg.ident == "Vec" && type_path.path.segments.len() == 1 {
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

pub(crate) fn validate_prefix_capacity(
    ty: &Type,
    prefix: PrefixType,
    max: usize,
    field_kind: &str,
) -> syn::Result<()> {
    let capacity = prefix.max_value();
    if max > capacity {
        return Err(syn::Error::new_spanned(
            ty,
            format!(
                "{field_kind} max {max} exceeds {} prefix capacity {capacity}",
                prefix.display_name()
            ),
        ));
    }
    Ok(())
}

/// Classifies bare `&str` / `&'a str` and `&[u8]` / `&'a [u8]` as tail fields.
///
/// Tail fields have no length prefix — remaining account/instruction data IS
/// the field. Must be the last dynamic field in the struct.
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

/// Maps a native integer type to its Pod companion (e.g., `u64` → `PodU64`).
/// Non-integer types pass through unchanged.
pub(crate) fn map_to_pod_type(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let ident_str = seg.ident.to_string();
            return match ident_str.as_str() {
                "u128" => quote! { quasar_lang::pod::PodU128 },
                "u64" => quote! { quasar_lang::pod::PodU64 },
                "u32" => quote! { quasar_lang::pod::PodU32 },
                "u16" => quote! { quasar_lang::pod::PodU16 },
                "i128" => quote! { quasar_lang::pod::PodI128 },
                "i64" => quote! { quasar_lang::pod::PodI64 },
                "i32" => quote! { quasar_lang::pod::PodI32 },
                "i16" => quote! { quasar_lang::pod::PodI16 },
                "bool" => quote! { quasar_lang::pod::PodBool },
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
            let pod_type = match seg.ident.to_string().as_str() {
                "u8" | "i8" => return quote! { __zc.#field_name = #value; },
                "bool" => quote! { quasar_lang::pod::PodBool },
                "u16" => quote! { quasar_lang::pod::PodU16 },
                "u32" => quote! { quasar_lang::pod::PodU32 },
                "u64" => quote! { quasar_lang::pod::PodU64 },
                "u128" => quote! { quasar_lang::pod::PodU128 },
                "i16" => quote! { quasar_lang::pod::PodI16 },
                "i32" => quote! { quasar_lang::pod::PodI32 },
                "i64" => quote! { quasar_lang::pod::PodI64 },
                "i128" => quote! { quasar_lang::pod::PodI128 },
                _ => return quote! { __zc.#field_name = #value; },
            };
            return quote! { __zc.#field_name = #pod_type::from(#value); };
        }
    }
    quote! { __zc.#field_name = #value; }
}

/// Generates a ZC assignment statement: `__zc.field = PodXX::from(field)`.
pub(crate) fn zc_assign_from_value(field_name: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    zc_assign_expr(field_name, ty, quote! { #field_name })
}

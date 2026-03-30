//! Field type classification for `#[derive(Accounts)]`.
//!
//! Classifies each field's wrapper type ONCE, replacing ~25 independent
//! `extract_generic_inner_type` + string-matching call sites with a single
//! enum that enables exhaustive `match` dispatch.

use {crate::helpers::extract_generic_inner_type, syn::Type};

/// The wrapper type of an account field, with inner type where applicable.
///
/// Classified once per field, then used everywhere: validation codegen,
/// field construction, init dispatch, header constants, detected-field
/// scanning, and attribute validation.
pub(super) enum FieldKind<'a> {
    /// `Account<T>` or `&[mut] Account<T>`
    Account { inner_ty: &'a Type },
    /// `InterfaceAccount<T>` or `&[mut] InterfaceAccount<T>`
    InterfaceAccount { inner_ty: &'a Type },
    /// `Program<T>`
    Program { inner_ty: &'a Type },
    /// `Interface<T>`
    Interface { inner_ty: &'a Type },
    /// `Sysvar<T>`
    Sysvar { inner_ty: &'a Type },
    /// `SystemAccount`
    SystemAccount,
    /// `Signer`
    Signer,
    /// Any type not matching above (UncheckedAccount, custom, etc.)
    Other,
}

/// Precomputed header flags for a field. Replaces the triple-computation in
/// `determine_nodup_constant`, `compute_header_expected`, and the dup-aware
/// path in `mod.rs`.
pub(super) struct FieldFlags {
    pub is_signer: bool,
    pub is_writable: bool,
    pub is_executable: bool,
}

/// Strip one layer of `&` / `&mut` from a type.
pub(super) fn strip_ref(ty: &Type) -> &Type {
    match ty {
        Type::Reference(r) => &r.elem,
        other => other,
    }
}

/// Extract the base name (last path segment) of a type.
pub(super) fn type_base_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(r) => type_base_name(&r.elem),
        _ => None,
    }
}

impl<'a> FieldKind<'a> {
    /// Classify a field type. Expects the type AFTER stripping `Option<>` and
    /// references (i.e., pass the "underlying" type).
    pub fn classify(underlying_ty: &'a Type) -> Self {
        // Order matters: check generic wrappers first, then bare types.
        if let Some(inner) = extract_generic_inner_type(underlying_ty, "Account") {
            return FieldKind::Account { inner_ty: inner };
        }
        if let Some(inner) = extract_generic_inner_type(underlying_ty, "InterfaceAccount") {
            return FieldKind::InterfaceAccount { inner_ty: inner };
        }
        if let Some(inner) = extract_generic_inner_type(underlying_ty, "Program") {
            return FieldKind::Program { inner_ty: inner };
        }
        if let Some(inner) = extract_generic_inner_type(underlying_ty, "Interface") {
            return FieldKind::Interface { inner_ty: inner };
        }
        if let Some(inner) = extract_generic_inner_type(underlying_ty, "Sysvar") {
            return FieldKind::Sysvar { inner_ty: inner };
        }
        match type_base_name(underlying_ty).as_deref() {
            Some("SystemAccount") => FieldKind::SystemAccount,
            Some("Signer") => FieldKind::Signer,
            _ => FieldKind::Other,
        }
    }

    pub fn is_executable(&self) -> bool {
        matches!(
            self,
            FieldKind::Program { .. } | FieldKind::Interface { .. }
        )
    }

    /// Check if the inner type (for Account/InterfaceAccount) matches any of
    /// the given names.
    pub fn inner_name_matches(&self, names: &[&str]) -> bool {
        let inner = match self {
            FieldKind::Account { inner_ty } | FieldKind::InterfaceAccount { inner_ty } => inner_ty,
            _ => return false,
        };
        type_base_name(inner)
            .as_deref()
            .is_some_and(|n| names.contains(&n))
    }

    /// Check if this is a token or mint type (Token, Token2022, Mint,
    /// Mint2022).
    pub fn is_token_or_mint(&self) -> bool {
        self.inner_name_matches(&["Token", "Token2022", "Mint", "Mint2022"])
    }

    /// Check if this is a token account (not mint).
    pub fn is_token_account(&self) -> bool {
        self.inner_name_matches(&["Token", "Token2022"])
    }

    /// Check if inner type has a lifetime parameter (dynamic account).
    pub fn is_dynamic(&self) -> bool {
        let inner = match self {
            FieldKind::Account { inner_ty } => inner_ty,
            _ => return false,
        };
        if let Type::Path(tp) = inner {
            if let Some(last) = tp.path.segments.last() {
                if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                    return args
                        .args
                        .iter()
                        .any(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)));
                }
            }
        }
        false
    }
}

impl FieldFlags {
    /// Compute header flags from the classified field kind and parsed attrs.
    pub fn compute(
        kind: &FieldKind,
        attrs: &super::attrs::AccountFieldAttrs,
        is_ref_mut: bool,
    ) -> Self {
        let is_signer = matches!(kind, FieldKind::Signer)
            || (attrs.is_init && attrs.seeds.is_none() && attrs.associated_token_mint.is_none());

        let is_writable = is_ref_mut || attrs.is_mut;

        let is_executable = kind.is_executable();

        FieldFlags {
            is_signer,
            is_writable,
            is_executable,
        }
    }

    /// The NODUP constant name for the no-dup fast path.
    pub fn nodup_constant(&self) -> &'static str {
        if self.is_executable {
            return "NODUP_EXECUTABLE";
        }
        match (self.is_signer, self.is_writable) {
            (true, true) => "NODUP_MUT_SIGNER",
            (true, false) => "NODUP_SIGNER",
            (false, true) => "NODUP_MUT",
            (false, false) => "NODUP",
        }
    }

    /// The expected u32 header value (little-endian: [borrow, signer, writable,
    /// exec]).
    pub fn header_constant(&self) -> u32 {
        let mut h: u32 = 0xFF; // byte 0: NOT_BORROWED
        if self.is_signer {
            h |= 0x01 << 8;
        }
        if self.is_writable {
            h |= 0x01 << 16;
        }
        if self.is_executable {
            h |= 0x01 << 24;
        }
        h
    }
}

/// Mask for the dup-aware path: covers all flag bytes (skips borrow_state).
/// Used for single-comparison flag validation in mod.rs Task 3.
pub(super) const FLAG_MASK: u32 = 0xFFFFFF00;

/// DRY codegen helper: emit a boolean-condition guard with debug logging.
///
/// Generates `if unlikely(condition) { [debug: log msg]; return Err(error); }`.
/// Use for checks where the condition and error are explicit (address
/// mismatches, interface checks, etc.) rather than wrapping a Result-returning
/// expression.
pub(super) fn debug_guard(
    condition: proc_macro2::TokenStream,
    debug_msg: proc_macro2::TokenStream,
    error: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote::quote! {
        if quasar_lang::utils::hint::unlikely(#condition) {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log(&#debug_msg);
            return Err(#error);
        }
    }
}

/// DRY codegen helper: emit a check with debug logging on failure.
///
/// In `#[cfg(feature = "debug")]`: logs `msg` with field name, returns Err.
/// In release: just `check_expr?;`
///
/// This replaces the 8-line debug/non-debug pattern repeated ~20 times.
pub(super) fn debug_checked(
    field_name_str: &str,
    check_expr: proc_macro2::TokenStream,
    msg: &str,
) -> proc_macro2::TokenStream {
    quote::quote! {
        #check_expr.map_err(|__e| {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log(&::alloc::format!(#msg, #field_name_str));
            __e
        })?;
    }
}

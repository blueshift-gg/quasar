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

/// Precomputed header flags for a field.
///
/// Used to generate both the expected header constant (for the exact u32
/// comparison on the hot path) and the required-mask (for the cold-path
/// minimum-requirements check in `decode_header_error`).
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
            || (attrs.is_init
                && attrs.seeds.is_none()
                && attrs.typed_seeds.is_none()
                && attrs.associated_token_mint.is_none());

        let is_writable = is_ref_mut
            || attrs.is_mut
            || attrs.is_init
            || attrs.init_if_needed
            || attrs.close.is_some()
            || attrs.realloc.is_some()
            || attrs.sweep.is_some();

        let is_executable = kind.is_executable();

        FieldFlags {
            is_signer,
            is_writable,
            is_executable,
        }
    }

    // RuntimeAccount header layout (u32 LE):
    //   byte 0: borrow_state  (0xFF = NOT_BORROWED)
    //   byte 1: is_signer     (0 or 1)
    //   byte 2: is_writable   (0 or 1)
    //   byte 3: executable    (0 or 1)
    const BORROW_STATE_BYTE: u32 = 0xFF;
    const SIGNER_BIT: u32 = 0x01 << 8;
    const WRITABLE_BIT: u32 = 0x01 << 16;
    const EXECUTABLE_BIT: u32 = 0x01 << 24;
    const SIGNER_MASK: u32 = 0xFF << 8;
    const WRITABLE_MASK: u32 = 0xFF << 16;
    const EXECUTABLE_MASK: u32 = 0xFF << 24;
    const FLAG_ONLY_MASK: u32 = 0xFFFFFF00;

    /// The expected u32 header value (little-endian: [borrow, signer, writable,
    /// exec]).
    pub fn header_constant(&self) -> u32 {
        let mut h: u32 = Self::BORROW_STATE_BYTE;
        if self.is_signer {
            h |= Self::SIGNER_BIT;
        }
        if self.is_writable {
            h |= Self::WRITABLE_BIT;
        }
        if self.is_executable {
            h |= Self::EXECUTABLE_BIT;
        }
        h
    }

    /// Mask for the cold-path "minimum requirements" check.
    ///
    /// Includes borrow_state (byte 0) plus only the flag bytes the field
    /// actually requires. Extra permissions are masked out so they don't
    /// cause a rejection.
    pub fn required_mask(&self) -> u32 {
        let mut mask: u32 = Self::BORROW_STATE_BYTE;
        if self.is_signer {
            mask |= Self::SIGNER_MASK;
        }
        if self.is_writable {
            mask |= Self::WRITABLE_MASK;
        }
        if self.is_executable {
            mask |= Self::EXECUTABLE_MASK;
        }
        mask
    }

    /// Flag-only mask (excludes borrow_state byte).
    /// Used in the dup-aware path where borrow_state is already validated.
    pub fn required_flag_mask(&self) -> u32 {
        self.required_mask() & Self::FLAG_ONLY_MASK
    }
}

/// DRY codegen helper: emit a boolean-condition guard with debug logging.
///
/// Generates `if unlikely(condition) { [debug: log msg]; return Err(error); }`.
/// Use for checks where the condition and error are explicit (address
/// mismatches, interface checks, etc.) rather than wrapping a Result-returning
/// expression.
#[allow(dead_code)]
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

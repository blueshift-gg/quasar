//! Shared classification of an instruction handler's context parameter.
//!
//! Both `#[program]` (when scanning handlers) and `#[instruction]` (when
//! deciding whether the direct-parse entry applies) must agree on what a valid
//! `ctx` parameter is. [`CtxKind::classify`] is the single source of truth: it
//! recognizes `Ctx<T>` and `CtxWithRemaining<T>`, exposes the inner accounts
//! type, and reports whether the handler carries remaining accounts — rejecting
//! any other first parameter identically for both callers.

use syn::Type;

/// The classified context wrapper of an instruction handler's first parameter:
/// the inner accounts type plus whether it is `CtxWithRemaining`.
#[derive(Clone, Copy)]
pub(crate) struct CtxKind<'a>(&'a Type, bool);

impl<'a> CtxKind<'a> {
    /// Classify the first parameter of an instruction function.
    pub(crate) fn classify(sig: &'a syn::Signature) -> syn::Result<Self> {
        let first_arg = match sig.inputs.first() {
            Some(syn::FnArg::Typed(pt)) => pt,
            _ => {
                return Err(syn::Error::new_spanned(
                    &sig.ident,
                    "instruction function must have ctx: Ctx<T> as first parameter",
                ));
            }
        };

        if let Some(inner) = crate::helpers::extract_generic_inner_type(&first_arg.ty, "Ctx") {
            return Ok(CtxKind(inner, false));
        }
        if let Some(inner) =
            crate::helpers::extract_generic_inner_type(&first_arg.ty, "CtxWithRemaining")
        {
            return Ok(CtxKind(inner, true));
        }

        Err(syn::Error::new_spanned(
            &first_arg.ty,
            "first parameter must be Ctx<T> or CtxWithRemaining<T>",
        ))
    }

    /// The inner accounts type `T` of `Ctx<T>` / `CtxWithRemaining<T>`.
    pub(crate) fn inner_ty(&self) -> &'a Type {
        self.0
    }

    /// Whether the handler takes remaining accounts (`CtxWithRemaining<T>`).
    pub(crate) fn has_remaining(&self) -> bool {
        self.1
    }
}

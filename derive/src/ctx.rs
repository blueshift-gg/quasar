//! Shared classification of an instruction handler's context parameter.
//!
//! `#[program]` and `#[instruction]` both use this parser, including the typed
//! `CtxWithRemaining<Accounts, Item, N>` form that becomes an enforced IDL
//! remaining-account policy.

use syn::{AngleBracketedGenericArguments, Expr, GenericArgument, PathArguments, Type};

#[derive(Clone, Copy)]
enum RemainingKind<'a> {
    None,
    Dynamic,
    Bounded { item: &'a Type, max: &'a Expr },
}

/// The classified context wrapper of an instruction handler.
#[derive(Clone, Copy)]
pub(crate) struct CtxKind<'a> {
    inner: &'a Type,
    remaining: RemainingKind<'a>,
}

impl<'a> CtxKind<'a> {
    /// Classify the first parameter of an instruction function.
    pub(crate) fn classify(sig: &'a syn::Signature) -> syn::Result<Self> {
        let first_arg = match sig.inputs.first() {
            Some(syn::FnArg::Typed(pt)) => pt,
            _ => {
                return Err(syn::Error::new_spanned(
                    &sig.ident,
                    "instruction function must have a context as its first parameter",
                ));
            }
        };

        if let Some(args) = context_args(&first_arg.ty, "Ctx") {
            let inner = only_type_arg(args).ok_or_else(|| {
                syn::Error::new_spanned(&first_arg.ty, "Ctx requires exactly one accounts type")
            })?;
            return Ok(Self {
                inner,
                remaining: RemainingKind::None,
            });
        }

        if let Some(args) = context_args(&first_arg.ty, "CtxWithRemaining") {
            let args = args.args.iter().collect::<Vec<_>>();
            return match args.as_slice() {
                [GenericArgument::Type(inner)] => Ok(Self {
                    inner,
                    remaining: RemainingKind::Dynamic,
                }),
                [GenericArgument::Type(inner), GenericArgument::Type(item), GenericArgument::Const(max)] => {
                    Ok(Self {
                        inner,
                        remaining: RemainingKind::Bounded { item, max },
                    })
                }
                _ => Err(syn::Error::new_spanned(
                    &first_arg.ty,
                    "CtxWithRemaining requires either <Accounts> or <Accounts, Item, MAX>",
                )),
            };
        }

        Err(syn::Error::new_spanned(
            &first_arg.ty,
            "first parameter must be Ctx<T>, CtxWithRemaining<T>, or CtxWithRemaining<T, Item, \
             MAX>",
        ))
    }

    /// The declared accounts type.
    pub(crate) fn inner_ty(&self) -> &'a Type {
        self.inner
    }

    /// Whether the handler takes any remaining accounts.
    pub(crate) fn has_remaining(&self) -> bool {
        !matches!(self.remaining, RemainingKind::None)
    }

    /// The item type and maximum for an enforced bounded tail.
    pub(crate) fn bounded_remaining(&self) -> Option<(&'a Type, &'a Expr)> {
        match self.remaining {
            RemainingKind::Bounded { item, max } => Some((item, max)),
            RemainingKind::None | RemainingKind::Dynamic => None,
        }
    }
}

fn context_args<'a>(ty: &'a Type, expected: &str) -> Option<&'a AngleBracketedGenericArguments> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != expected {
        return None;
    }
    match &segment.arguments {
        PathArguments::AngleBracketed(args) => Some(args),
        PathArguments::None | PathArguments::Parenthesized(_) => None,
    }
}

fn only_type_arg(args: &AngleBracketedGenericArguments) -> Option<&Type> {
    let mut iter = args.args.iter();
    let GenericArgument::Type(ty) = iter.next()? else {
        return None;
    };
    iter.next().is_none().then_some(ty)
}

#[cfg(test)]
mod tests {
    use {super::CtxKind, quote::ToTokens};

    #[test]
    fn classifies_bounded_typed_remaining_context() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn create(ctx: CtxWithRemaining<Create, Signer, 10>) {}
        };
        let kind = CtxKind::classify(&function.sig).unwrap();
        let (item, max) = kind.bounded_remaining().unwrap();

        assert_eq!(kind.inner_ty().to_token_stream().to_string(), "Create");
        assert_eq!(item.to_token_stream().to_string(), "Signer");
        assert_eq!(max.to_token_stream().to_string(), "10");
    }

    #[test]
    fn rejects_incomplete_typed_remaining_context() {
        let function: syn::ItemFn = syn::parse_quote! {
            fn create(ctx: CtxWithRemaining<Create, Signer>) {}
        };

        assert!(CtxKind::classify(&function.sig).is_err());
    }
}

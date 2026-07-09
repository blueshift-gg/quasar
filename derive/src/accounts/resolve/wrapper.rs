//! One wrapper-type classifier (absorbed plan 040).
//!
//! Account field types are matched **syntactically on their last path
//! segment**. Proc macros cannot resolve type aliases, so
//! `type Vault = Account<'a, VaultState>; vault: Vault` classifies as `Other`,
//! never `Account`. This is a fundamental proc-macro limitation; it is
//! documented once here and on the `#[derive(Accounts)]` rustdoc.
//!
//! Before this module the wrapper-name lists had drifted across call sites
//! (`lower.rs` knew `Interface`, the rent planner and IDL emitters did not).
//! `classify_wrapper` is the single reconciled list; every consumer matches on
//! the `WrapperKind` stored on `FieldCore` (or calls `classify_wrapper` on a
//! bare type in the emitters) instead of re-deriving the kind from raw `syn`.

use syn::Type;

/// Which library wrapper an account field's effective type is, by last-segment
/// match. `Other` covers user composites and anything unrecognized.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WrapperKind {
    Account,
    InterfaceAccount,
    Migration,
    Uninit,
    Program,
    Interface,
    Sysvar,
    Signer,
    UncheckedAccount,
    EventAuthority,
    AccountsArray,
    Other,
}

/// Classify a field's effective type by its last path segment.
pub(crate) fn classify_wrapper(ty: &Type) -> WrapperKind {
    let Type::Path(tp) = ty else {
        return WrapperKind::Other;
    };
    let Some(last) = tp.path.segments.last() else {
        return WrapperKind::Other;
    };
    match last.ident.to_string().as_str() {
        "Account" => WrapperKind::Account,
        "InterfaceAccount" => WrapperKind::InterfaceAccount,
        "Migration" => WrapperKind::Migration,
        "Uninit" => WrapperKind::Uninit,
        "Program" => WrapperKind::Program,
        "Interface" => WrapperKind::Interface,
        "Sysvar" => WrapperKind::Sysvar,
        "Signer" => WrapperKind::Signer,
        "UncheckedAccount" => WrapperKind::UncheckedAccount,
        "EventAuthority" => WrapperKind::EventAuthority,
        "AccountsArray" => WrapperKind::AccountsArray,
        _ => WrapperKind::Other,
    }
}

/// The first inner type argument of a `Sysvar<T>` wrapper, if this is one.
/// Used by the rent planner to detect `Sysvar<Rent>`.
pub(crate) fn sysvar_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(tp) = ty else { return None };
    let last = tp.path.segments.last()?;
    if last.ident != "Sysvar" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ty(s: &str) -> Type {
        syn::parse_str(s).expect("type parses")
    }

    #[test]
    fn last_segment_classification() {
        assert_eq!(
            classify_wrapper(&ty("Account<'a, T>")),
            WrapperKind::Account
        );
        assert_eq!(
            classify_wrapper(&ty("quasar_lang::Interface<'a, T>")),
            WrapperKind::Interface
        );
        assert_eq!(classify_wrapper(&ty("Signer")), WrapperKind::Signer);
        assert_eq!(
            classify_wrapper(&ty("AccountsArray<T, 3>")),
            WrapperKind::AccountsArray
        );
        // Type aliases are opaque to a proc macro: not classified.
        assert_eq!(classify_wrapper(&ty("MyVaultAlias")), WrapperKind::Other);
    }

    #[test]
    fn sysvar_inner_extracts_rent() {
        let sysvar = ty("Sysvar<'a, Rent>");
        let inner = sysvar_inner(&sysvar).expect("sysvar inner");
        // The rent planner keys off the inner type's last path segment.
        let is_rent = matches!(inner, Type::Path(tp)
            if tp.path.segments.last().is_some_and(|s| s.ident == "Rent"));
        assert!(is_rent);
        assert!(sysvar_inner(&ty("Account<'a, T>")).is_none());
    }
}

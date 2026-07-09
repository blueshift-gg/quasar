//! Structural validation: invariants only, no protocol knowledge.
//!
//! Protocol-specific validation (required args, arg types, exit ordering)
//! is owned by behavior modules via builder errors and trait bounds.

use {
    super::{
        wrapper::{classify_wrapper, WrapperKind},
        BehaviorArgValue, FieldKind, FieldSemantics,
    },
    std::collections::HashSet,
    syn::Ident,
};

/// A wrapper type that cannot be the target of a structural lifecycle op
/// (`init` / `realloc` / `close`) or the `From` side of a `Migration`. Returns
/// the display name of the wrapper when it is in the deny-set, `None` when the
/// wrapper legitimately supports these ops (`Account<T>` / `InterfaceAccount<T>`,
/// plus user composites, which carry their own rules).
///
/// The deny-set is verified against the runtime trait impls: only `Account<T>`
/// and `InterfaceAccount<T>` implement `SupportsRealloc`/`AccountInit`/`Space`
/// (`lang/src/accounts/{account,interface_account}.rs`), and only they carry a
/// discriminator to zero on close, so every wrapper below genuinely lacks the
/// op. Front-ending the rejection here turns an `E0277` trait-bound dump that
/// quotes `ops/realloc.rs` internals into one spanned error at the field.
fn denied_op_target(wrapper: WrapperKind) -> Option<&'static str> {
    match wrapper {
        WrapperKind::Signer => Some("`Signer`"),
        WrapperKind::UncheckedAccount => Some("`UncheckedAccount`"),
        WrapperKind::Program => Some("`Program`"),
        WrapperKind::Interface => Some("`Interface`"),
        WrapperKind::Sysvar => Some("`Sysvar`"),
        WrapperKind::EventAuthority => Some("`EventAuthority`"),
        _ => None,
    }
}

/// Reject structural lifecycle ops on wrapper types that cannot support them,
/// as a single spanned error at the field, before codegen reaches the op's
/// trait bounds. `Uninit`/`Migration`/composite wrappers are handled by their
/// own dedicated rules and are never in the deny-set.
fn validate_op_target(sem: &FieldSemantics) -> syn::Result<()> {
    let span = &sem.core.field;
    let wrapper = sem.core.wrapper;

    if sem.realloc.is_some() {
        if let Some(name) = denied_op_target(wrapper) {
            return Err(syn::Error::new_spanned(
                span,
                format!(
                    "`realloc` requires a program account (`Account<T>` or `InterfaceAccount<T>`); \
                     {name} cannot be reallocated"
                ),
            ));
        }
    }

    if sem.has_init() {
        if let Some(name) = denied_op_target(wrapper) {
            return Err(syn::Error::new_spanned(
                span,
                format!(
                    "`init` requires a program account (`Account<T>` or `InterfaceAccount<T>`); \
                     {name} cannot be initialized"
                ),
            ));
        }
    }

    if sem.close_dest.is_some() {
        if let Some(name) = denied_op_target(wrapper) {
            return Err(syn::Error::new_spanned(
                span,
                format!(
                    "`close` requires a program account (`Account<T>` or `InterfaceAccount<T>`); \
                     {name} cannot be closed"
                ),
            ));
        }
    }

    // `Migration<From, To>`: the `From` (source) type is the first generic
    // argument, recorded as `core.inner_ty`. It must be a program-owned data
    // account (a `#[account]` type, classified `Other`), never a bare wrapper
    // like `Signer` that carries no discriminator to verify the old version.
    if sem.is_migration {
        if let Some(from) = &sem.core.inner_ty {
            if let Some(name) = denied_op_target(classify_wrapper(from)) {
                return Err(syn::Error::new_spanned(
                    span,
                    format!(
                        "`Migration<From, To>` requires a program account as its `From` type; \
                         {name} cannot be migrated"
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Fold `e` into the running accumulator via `syn::Error::combine`, so each
/// contained error is emitted as its own `compile_error!` — three bad fields
/// surface three diagnostics in one compile cycle instead of three.
fn push_err(acc: &mut Option<syn::Error>, e: syn::Error) {
    match acc {
        Some(prev) => prev.combine(e),
        None => *acc = Some(e),
    }
}

/// Validate every field, accumulating errors across fields (each field is
/// independent) so a single compile reports all structural violations.
///
/// `field_names` is supplied by the caller (built from the field cores, which
/// always exist), so a field that failed to *lower* is still a known name for
/// behavior-ref resolution — dropping it never turns a valid `mint = vault`
/// into a spurious "no field `vault`".
pub(super) fn validate_semantics(
    semantics: &[FieldSemantics],
    field_names: &HashSet<String>,
) -> Option<syn::Error> {
    let mut acc: Option<syn::Error> = None;
    for sem in semantics {
        if let Err(e) = validate_field(sem) {
            push_err(&mut acc, e);
        }
        if let Err(e) = validate_behavior_field_refs(sem, field_names) {
            push_err(&mut acc, e);
        }
    }
    acc
}

fn validate_field(sem: &FieldSemantics) -> syn::Result<()> {
    let span = &sem.core.field;

    // --- Composite fields accept only `group` ---
    // A composite field (an `AccountsArray<..>` or a nested `#[account(group)]`
    // struct) delegates parsing/validation to its own `Accounts` impl. Applying
    // a lifecycle/constraint directive to the composite itself is meaningless, so
    // reject everything except the `group` marker.
    if sem.core.kind == FieldKind::Composite {
        let offending = if sem.core.declared_mut {
            Some("mut")
        } else if sem.has_init() {
            Some("init")
        } else if sem.payer.is_some() {
            Some("payer")
        } else if sem.address.is_some() {
            Some("address")
        } else if sem.realloc.is_some() {
            Some("realloc")
        } else if sem.close_dest.is_some() {
            Some("close")
        } else if sem.core.dup {
            Some("dup")
        } else if !sem.groups.is_empty() {
            Some("behavior group")
        } else if !sem.user_checks.is_empty() {
            Some("has_one/constraints")
        } else {
            None
        };
        if let Some(directive) = offending {
            return Err(syn::Error::new_spanned(
                span,
                format!(
                    "`{directive}` is not supported on a composite account field; composite \
                     fields (`AccountsArray<..>` or nested `#[account(group)]` structs) accept \
                     only `group`"
                ),
            ));
        }
    }

    // --- Migration exclusivity rules ---
    if sem.is_migration {
        if sem.core.optional {
            return Err(syn::Error::new_spanned(
                span,
                "`Option<Migration<...>>` is not supported: migration fields cannot be optional",
            ));
        }
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "`init` cannot be used with `Migration<From, To>`",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc` cannot be used with `Migration<From, To>`",
            ));
        }
        if !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "behavior groups cannot be used with `Migration<From, To>`",
            ));
        }
    }

    // --- Deferred init exclusivity rules ---
    if sem.is_uninit {
        if sem.core.optional {
            return Err(syn::Error::new_spanned(
                span,
                "`Option<Uninit<...>>` is not supported: deferred init fields cannot be optional",
            ));
        }
        if sem.has_init() {
            return Err(syn::Error::new_spanned(
                span,
                "`init` cannot be used with `Uninit<T>`; call `.init(...)` in the handler",
            ));
        }
        if sem.realloc.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`realloc` cannot be used with `Uninit<T>`",
            ));
        }
        if sem.close_dest.is_some() {
            return Err(syn::Error::new_spanned(
                span,
                "`close` cannot be used with `Uninit<T>`",
            ));
        }
        if !sem.groups.is_empty() {
            return Err(syn::Error::new_spanned(
                span,
                "behavior groups cannot be used with `Uninit<T>`; pass init params to `.init(...)`",
            ));
        }
    }

    // `init` implies `mut` (init-implies-mut): no separate requirement.

    // The remaining rules are independent single-field constraints: accumulate
    // them so a field that trips several (e.g. `realloc` on a `Signer` that is
    // also `Option`) reports every violation at once.
    let mut acc: Option<syn::Error> = None;

    // Structural ops require a program data-account wrapper (type gate).
    if let Err(e) = validate_op_target(sem) {
        push_err(&mut acc, e);
    }

    // init + realloc mutual exclusion
    if sem.has_init() && sem.realloc.is_some() {
        push_err(
            &mut acc,
            syn::Error::new_spanned(span, "`realloc = ...` cannot be used with `init`"),
        );
    }

    // dup + mutation ops blocked (init, realloc, close, mut behavior groups)
    if sem.core.dup {
        if sem.has_init() {
            push_err(
                &mut acc,
                syn::Error::new_spanned(
                    span,
                    "`dup` cannot be used with `init`: mutation on aliased accounts is unsound",
                ),
            );
        }
        if sem.realloc.is_some() {
            push_err(
                &mut acc,
                syn::Error::new_spanned(
                    span,
                    "`dup` cannot be used with `realloc`: mutation on aliased accounts is unsound",
                ),
            );
        }
        if sem.close_dest.is_some() {
            push_err(
                &mut acc,
                syn::Error::new_spanned(
                    span,
                    "`dup` cannot be used with `close`: mutation on aliased accounts is unsound",
                ),
            );
        }
        if sem.core.declared_mut && !sem.groups.is_empty() {
            push_err(
                &mut acc,
                syn::Error::new_spanned(
                    span,
                    "`dup` with `mut` cannot have behavior groups: mutation on aliased accounts \
                     is unsound",
                ),
            );
        }
    }

    // dup requires /// CHECK: doc comment
    if sem.core.dup {
        let has_doc = sem
            .core
            .field
            .attrs
            .iter()
            .any(|a| a.path().is_ident("doc"));
        if !has_doc {
            push_err(
                &mut acc,
                syn::Error::new_spanned(
                    span,
                    "#[account(dup)] requires a /// CHECK: <reason> doc comment",
                ),
            );
        }
    }

    if sem.core.optional && sem.has_init() {
        push_err(
            &mut acc,
            syn::Error::new_spanned(span, "init(...) cannot be used on Option<T> fields"),
        );
    }

    // Optional realloc not supported
    if sem.core.optional && sem.realloc.is_some() {
        push_err(
            &mut acc,
            syn::Error::new_spanned(span, "`realloc = ...` cannot be used on Option<T> fields"),
        );
    }

    // `realloc` implies `mut` (realloc-implies-mut): no separate requirement.

    // init(idempotent) requires a behavior group or address constraint
    if let Some(init) = &sem.init {
        if init.idempotent {
            let has_behavior = !sem.groups.is_empty();
            let has_address = sem.address.is_some();
            if !has_behavior && !has_address {
                push_err(
                    &mut acc,
                    syn::Error::new_spanned(
                        span,
                        "`init(idempotent)` requires a behavior group (e.g., token(...)) or \
                         address constraint",
                    ),
                );
            }
        }
    }

    match acc {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Validate behavior arg field references: every `FieldRef` (bare lowercase
/// ident), at any nesting depth inside `Some(...)`, must name a real field.
/// The recursion closes the old one-level `Some(Some(typo))` hole.
fn validate_behavior_field_refs(
    sem: &FieldSemantics,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    for group in &sem.groups {
        for arg in &group.args {
            check_arg_value(&arg.value, &arg.key, field_names)?;
        }
    }
    Ok(())
}

/// Recursively reject `FieldRef` idents that aren't field names.
fn check_arg_value(
    value: &BehaviorArgValue,
    key: &Ident,
    field_names: &HashSet<String>,
) -> syn::Result<()> {
    match value {
        BehaviorArgValue::FieldRef(ident) => {
            let name = ident.to_string();
            if !field_names.contains(&name) {
                return Err(syn::Error::new_spanned(
                    ident,
                    format!("`{key} = {name}`: no field `{name}` in this accounts struct"),
                ));
            }
            Ok(())
        }
        BehaviorArgValue::Some(inner) => check_arg_value(inner, key, field_names),
        BehaviorArgValue::None | BehaviorArgValue::Expr(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::super::lower_semantics,
        quote::quote,
        syn::{punctuated::Punctuated, token::Comma, Field, Fields, ItemStruct},
    };

    fn fields(ts: proc_macro2::TokenStream) -> Punctuated<Field, Comma> {
        let item: ItemStruct = syn::parse2(ts).expect("struct parses");
        match item.fields {
            Fields::Named(named) => named.named,
            _ => Punctuated::new(),
        }
    }

    fn lower_err(ts: proc_macro2::TokenStream) -> String {
        lower_semantics(&fields(ts), &[])
            .err()
            .expect("expected a validation error")
            .to_string()
    }

    #[test]
    fn composite_rejects_init() {
        let err = lower_err(quote! {
            struct S { #[account(group, init)] bundle: SomeBundle }
        });
        assert!(
            err.contains("`init` is not supported on a composite"),
            "{err}"
        );
    }

    #[test]
    fn composite_rejects_address() {
        let err = lower_err(quote! {
            struct S { #[account(group, address = FOO)] bundle: SomeBundle }
        });
        assert!(
            err.contains("`address` is not supported on a composite"),
            "{err}"
        );
    }

    #[test]
    fn composite_rejects_behavior_group() {
        let err = lower_err(quote! {
            struct S { #[account(group, min_value(min = 1u64))] bundle: SomeBundle }
        });
        assert!(
            err.contains("`behavior group` is not supported on a composite"),
            "{err}"
        );
    }

    #[test]
    fn composite_rejects_dup() {
        let err = lower_err(quote! {
            struct S {
                /// CHECK: test
                #[account(group, dup)] bundle: SomeBundle
            }
        });
        assert!(
            err.contains("`dup` is not supported on a composite"),
            "{err}"
        );
    }

    #[test]
    fn composite_group_marker_alone_is_accepted() {
        let sems = lower_semantics(
            &fields(quote! { struct S { #[account(group)] bundle: SomeBundle } }),
            &[],
        )
        .expect("bare group lowers");
        assert_eq!(sems.len(), 1);
    }
}

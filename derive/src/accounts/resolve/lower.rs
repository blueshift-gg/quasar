//! Lowering: parsed directives to field semantics.
//!
//! This pass records syntax-level facts and simple wrapper shape; validation
//! and lifecycle scheduling happen in rules + planner.

use {
    super::{
        super::{
            syntax::{
                attrs::{CoreDirective, Directive},
                parse_field_attrs,
            },
            InstructionArg,
        },
        rules::validate_semantics,
        wrapper::{classify_wrapper, WrapperKind},
        AddressConstraint, AddressKind, FieldCore, FieldKind, FieldSemantics, InitDirective,
        SeedRef,
    },
    crate::helpers::{extract_generic_inner_type, is_composite_type},
    std::collections::HashSet,
    syn::{Expr, ExprCall, Member, Type},
};

pub(super) fn lower_semantics(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    instruction_args: &[InstructionArg],
) -> syn::Result<Vec<FieldSemantics>> {
    let parsed: Vec<(syn::Field, Vec<Directive>)> = fields
        .iter()
        .map(|field| Ok((field.clone(), parse_field_attrs(field)?)))
        .collect::<syn::Result<_>>()?;

    let cores: Vec<FieldCore> = parsed
        .iter()
        .map(|(field, directives)| lower_core(field, directives))
        .collect();

    let scope = SeedScope::new(&cores, instruction_args);

    let semantics: Vec<FieldSemantics> = parsed
        .into_iter()
        .zip(cores)
        .map(|((_, directives), core)| {
            let is_migration = core.wrapper == WrapperKind::Migration;
            let is_uninit = core.wrapper == WrapperKind::Uninit;
            let mut sem = FieldSemantics {
                core,
                init: None,
                payer: None,
                address: None,
                realloc: None,
                close_dest: None,
                groups: Vec::new(),
                user_checks: Vec::new(),
                is_migration,
                is_uninit,
            };
            lower_directives(&mut sem, directives, &scope)?;
            Ok(sem)
        })
        .collect::<syn::Result<_>>()?;

    validate_semantics(&semantics)?;

    Ok(semantics)
}

/// Name sets used to resolve typed-seed arguments once during lowering.
struct SeedScope {
    /// Every account field ident in the struct.
    field_names: HashSet<String>,
    /// Account field idents whose wrapper carries an inner type (so a
    /// `field.member` seed can name the account type for the IDL). Fields
    /// without an inner type fall back to `Const`, matching the old resolver.
    field_with_inner: HashSet<String>,
    /// Struct-level `#[instruction(..)]` argument names.
    ix_args: HashSet<String>,
}

impl SeedScope {
    fn new(cores: &[FieldCore], instruction_args: &[InstructionArg]) -> Self {
        Self {
            field_names: cores.iter().map(|c| c.ident.to_string()).collect(),
            field_with_inner: cores
                .iter()
                .filter(|c| c.inner_ty.is_some())
                .map(|c| c.ident.to_string())
                .collect(),
            ix_args: instruction_args
                .iter()
                .map(|a| a.name.to_string())
                .collect(),
        }
    }
}

fn lower_core(field: &syn::Field, directives: &[Directive]) -> FieldCore {
    let ty = &field.ty;
    let option_inner = extract_generic_inner_type(ty, "Option").cloned();
    let optional = option_inner.is_some();
    let after_option = option_inner.unwrap_or_else(|| ty.clone());

    let effective_ty = match &after_option {
        Type::Reference(r) => (*r.elem).clone(),
        other => other.clone(),
    };

    let kind = classify_kind(
        ty,
        directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Group))),
    );

    let inner_ty = extract_inner_ty(&effective_ty);
    let dynamic = detect_dynamic(&effective_ty, inner_ty.as_ref());
    let wrapper = classify_wrapper(&effective_ty);

    FieldCore {
        ident: field
            .ident
            .clone()
            .expect("account field must have an identifier"),
        field: field.clone(),
        effective_ty,
        kind,
        wrapper,
        inner_ty,
        optional,
        dynamic,
        declared_mut: directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Mut))),
        dup: directives
            .iter()
            .any(|d| matches!(d, Directive::Core(CoreDirective::Dup))),
    }
}

fn classify_kind(raw_ty: &Type, explicit_group: bool) -> FieldKind {
    if explicit_group || is_composite_type(raw_ty) {
        FieldKind::Composite
    } else {
        FieldKind::Single
    }
}

fn lower_directives(
    sem: &mut FieldSemantics,
    directives: Vec<Directive>,
    scope: &SeedScope,
) -> syn::Result<()> {
    let mut groups = Vec::new();

    for directive in directives {
        match directive {
            Directive::Core(core) => match core {
                CoreDirective::Mut | CoreDirective::Dup | CoreDirective::Group => {
                    /* handled in lower_core */
                }
                CoreDirective::Init { idempotent } => {
                    if sem.init.is_some() {
                        return Err(syn::Error::new_spanned(
                            &sem.core.field,
                            "duplicate `init` directive",
                        ));
                    }
                    sem.init = Some(InitDirective { idempotent });
                }
                CoreDirective::Payer(ident) => {
                    if sem.payer.is_some() {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            "duplicate `payer = ...` directive",
                        ));
                    }
                    sem.payer = Some(ident);
                }
                CoreDirective::Address(expr, error) => {
                    if sem.address.is_some() {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "duplicate `address = ...` directive",
                        ));
                    }
                    // The `@ error` form stays on the address constraint (not
                    // user_checks) so the field keeps its Bumps entry, stored-
                    // bump fast path, signer helper, and IDL PDA resolver.
                    let kind = classify_address(&expr, scope);
                    sem.address = Some(AddressConstraint { expr, error, kind });
                }
                CoreDirective::Realloc(expr) => {
                    if sem.realloc.is_some() {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "duplicate `realloc = ...` directive",
                        ));
                    }
                    sem.realloc = Some(expr);
                }
                CoreDirective::Close(dest) => {
                    if sem.close_dest.is_some() {
                        return Err(syn::Error::new_spanned(
                            &dest,
                            "duplicate `close(...)` directive",
                        ));
                    }
                    sem.close_dest = Some(dest);
                }
            },
            Directive::Behavior(group) => {
                groups.push(group);
            }
            Directive::Check(check) => {
                sem.user_checks.push(check);
            }
        }
    }

    sem.groups = groups;

    Ok(())
}

// Type classification helpers.

fn extract_inner_ty(effective_ty: &Type) -> Option<Type> {
    for wrapper in &[
        "Account",
        "InterfaceAccount",
        "Migration",
        "Uninit",
        "Program",
        "Interface",
        "Sysvar",
    ] {
        if let Some(inner) = extract_generic_inner_type(effective_ty, wrapper) {
            return Some(inner.clone());
        }
    }
    None
}

fn detect_dynamic(effective_ty: &Type, inner_ty: Option<&Type>) -> bool {
    if extract_generic_inner_type(effective_ty, "Account").is_none() {
        return false;
    }
    let Some(inner) = inner_ty else { return false };
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

// Address classification: `AddressKind` computed once, consumed by the signer
// helper and IDL resolver emitters so they can never disagree.

/// Classify an `address = expr` constraint. A `Path::seeds(args...)` call
/// (tolerant of surrounding parentheses/groups) is a typed-seeds PDA; every
/// other form is `Opaque` and keeps `expr` verbatim on the constraint.
fn classify_address(expr: &Expr, scope: &SeedScope) -> AddressKind {
    let Some(call) = as_seeds_call(expr) else {
        return AddressKind::Opaque;
    };
    let Expr::Path(func) = call.func.as_ref() else {
        return AddressKind::Opaque;
    };
    // The account type is the path with the trailing `seeds` segment removed;
    // `as_seeds_call` guarantees at least two segments.
    let segments = func.path.segments.len() - 1;
    let account_ty = syn::Path {
        leading_colon: func.path.leading_colon,
        segments: func.path.segments.iter().take(segments).cloned().collect(),
    };
    let seeds = call
        .args
        .iter()
        .map(|arg| resolve_seed_ref(arg, scope))
        .collect();
    AddressKind::Seeds { account_ty, seeds }
}

/// Strip surrounding parens/groups and return the inner call if it is a
/// `Account::seeds(...)` (at least a type segment plus the `seeds` segment).
fn as_seeds_call(expr: &Expr) -> Option<&ExprCall> {
    match expr {
        Expr::Paren(p) => as_seeds_call(&p.expr),
        Expr::Group(g) => as_seeds_call(&g.expr),
        Expr::Call(call) => {
            let Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let last = path.path.segments.last()?;
            (last.ident == "seeds" && path.path.segments.len() >= 2).then_some(call)
        }
        _ => None,
    }
}

/// Classify one seed argument into a `SeedRef`.
fn resolve_seed_ref(expr: &Expr, scope: &SeedScope) -> SeedRef {
    let expr = strip_seed_into(expr);

    // `field.address()` on an account field.
    if let Expr::MethodCall(call) = expr {
        if call.method == "address" && call.args.is_empty() {
            if let Some(base) = single_ident(&call.receiver) {
                if scope.field_names.contains(&base.to_string()) {
                    return SeedRef::AccountAddr(base);
                }
            }
        }
    }

    // `field.member` (possibly nested) read off an account field that carries an
    // inner type; without an inner type the IDL cannot name the account, so it
    // degrades to `Const` exactly as the old resolver did.
    if let Some((base, path)) = account_field_path(expr) {
        if scope.field_with_inner.contains(&base.to_string()) {
            return SeedRef::AccountField { base, path };
        }
    }

    // Bare identifier naming an instruction argument.
    if let Some(id) = single_ident(expr) {
        if scope.ix_args.contains(&id.to_string()) {
            return SeedRef::IxArg(id);
        }
    }

    SeedRef::Const(expr.clone())
}

/// Strip a trailing `.into()` (with no args) from a seed expression.
fn strip_seed_into(expr: &Expr) -> &Expr {
    if let Expr::MethodCall(call) = expr {
        if call.method == "into" && call.args.is_empty() {
            return strip_seed_into(&call.receiver);
        }
    }
    expr
}

/// If `expr` is a single-segment path, return that identifier.
fn single_ident(expr: &Expr) -> Option<syn::Ident> {
    if let Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 {
            return Some(ep.path.segments[0].ident.clone());
        }
    }
    None
}

/// Walk a `base.a.b` member chain; return the base ident and dotted path.
fn account_field_path(expr: &Expr) -> Option<(syn::Ident, String)> {
    let mut fields = Vec::new();
    let mut cur = expr;
    loop {
        match cur {
            Expr::Field(field) => {
                let name = match &field.member {
                    Member::Named(ident) => ident.to_string(),
                    Member::Unnamed(_) => return None,
                };
                fields.push(name);
                cur = &field.base;
            }
            Expr::Path(_) => {
                let base = single_ident(cur)?;
                if fields.is_empty() {
                    return None;
                }
                fields.reverse();
                return Some((base, fields.join(".")));
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::accounts::syntax::parse_field_attrs,
        quote::{quote, ToTokens},
        syn::{parse::Parser, Fields},
    };

    fn field(tokens: proc_macro2::TokenStream) -> syn::Field {
        syn::Field::parse_named
            .parse2(tokens)
            .expect("test field parses")
    }

    fn core_of(tokens: proc_macro2::TokenStream) -> FieldCore {
        let f = field(tokens);
        let directives = parse_field_attrs(&f).expect("directives parse");
        lower_core(&f, &directives)
    }

    #[test]
    fn option_wrapper_is_unwrapped() {
        let core = core_of(quote! {
            #[account(has_one(authority))] config: Option<Account<Config>>
        });
        assert!(core.optional);
        assert_eq!(
            core.effective_ty.to_token_stream().to_string(),
            "Account < Config >"
        );
        assert_eq!(
            core.inner_ty
                .as_ref()
                .unwrap()
                .to_token_stream()
                .to_string(),
            "Config"
        );
    }

    #[test]
    fn reference_is_unwrapped() {
        let core = core_of(quote! { signer: &'a Signer });
        assert!(!core.optional);
        assert_eq!(core.effective_ty.to_token_stream().to_string(), "Signer");
    }

    #[test]
    fn mut_and_dup_flags_recorded() {
        let core = core_of(quote! { #[account(mut, dup)] account: UncheckedAccount });
        assert!(core.declared_mut);
        assert!(core.dup);
    }

    #[test]
    fn group_directive_marks_composite() {
        let core = core_of(quote! { #[account(group)] bundle: SomeBundle });
        assert!(matches!(core.kind, FieldKind::Composite));
    }

    #[test]
    fn address_error_routes_to_address_not_user_checks() {
        // A19: the `@ error` form must stay on the address constraint (keeping the
        // Bumps entry, stored-bump fast path, signer helper, IDL PDA resolver), not
        // be rerouted into `user_checks`.
        let f = field(quote! {
            #[account(address = SOME_ADDR @ MyError::Bad)] account: Account<Config>
        });
        let directives = parse_field_attrs(&f).expect("directives parse");
        let core = lower_core(&f, &directives);
        let scope = SeedScope::new(std::slice::from_ref(&core), &[]);
        let mut sem = FieldSemantics {
            core,
            init: None,
            payer: None,
            address: None,
            realloc: None,
            close_dest: None,
            groups: Vec::new(),
            user_checks: Vec::new(),
            is_migration: false,
            is_uninit: false,
        };
        lower_directives(&mut sem, directives, &scope).expect("lowering succeeds");
        let address = sem.address.expect("address constraint recorded");
        assert!(
            address.error.is_some(),
            "custom `@ error` must stay on the address constraint"
        );
        assert!(
            sem.user_checks.is_empty(),
            "`@ error` must not be rerouted into user_checks"
        );
    }

    #[test]
    fn full_lower_records_optional_and_has_one() {
        let item: syn::ItemStruct = syn::parse_quote! {
            struct OptionalAccounts {
                authority: Signer,
                #[account(has_one(authority))]
                config: Option<Account<Config>>,
            }
        };
        let fields = match item.fields {
            Fields::Named(named) => named.named,
            _ => Default::default(),
        };
        let sems = lower_semantics(&fields, &[]).expect("valid struct lowers");
        assert_eq!(sems.len(), 2);
        assert!(sems[1].core.optional);
        assert_eq!(sems[1].user_checks.len(), 1);
    }
}

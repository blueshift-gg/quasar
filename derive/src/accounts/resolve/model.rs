use {
    super::wrapper::WrapperKind,
    quote::ToTokens,
    syn::{parse_quote, Expr, Ident, Type},
};

/// Account field shape for parsing and account-count planning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Single,
    Composite,
}

pub(crate) struct FieldCore {
    pub ident: Ident,
    pub field: syn::Field,
    pub effective_ty: Type,
    pub kind: FieldKind,
    /// Which library wrapper `effective_ty` is (last-segment match). Computed
    /// once in lowering; every consumer reads this instead of re-classifying.
    pub wrapper: WrapperKind,
    /// Inner/source type for generic wrappers.
    pub inner_ty: Option<Type>,
    pub optional: bool,
    pub dynamic: bool,
    /// The user's literal `#[account(mut)]` directive. Never mutated by
    /// lowering: `init`/`close`/`realloc`/`Migration`/`Uninit` imply writability
    /// through `is_writable()`, not by forging a `mut` the user never wrote.
    pub declared_mut: bool,
    pub dup: bool,
}

/// A behavior group directive: `path(key = value, ...)`.
///
/// The derive treats every non-core group as an open behavior group. The path
/// resolves to a Rust module exporting `Args::builder()` and `Behavior`.
/// No protocol-specific knowledge lives here.
#[derive(Clone)]
pub(crate) struct BehaviorGroup {
    pub path: syn::Path,
    pub args: Vec<BehaviorArg>,
}

impl BehaviorGroup {
    /// The last segment of the path, used for variable naming.
    pub(crate) fn name(&self) -> String {
        self.path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("_")
    }
}

/// A single `key = value` arg in a behavior group directive. The value is
/// parsed once at parse time into the phase-polymorphic grammar.
#[derive(Clone)]
pub(crate) struct BehaviorArg {
    pub key: Ident,
    pub value: BehaviorArgValue,
}

/// A behavior-arg value in the phase-polymorphic grammar, parsed once at parse
/// time. Grammar validation, field-ref validation, and lowering are all total
/// matches on this enum — there is no re-parsing of a raw `Expr` and no
/// one-level `Some(Some(typo))` hole.
#[derive(Clone)]
pub(crate) enum BehaviorArgValue {
    /// A bare lowercase identifier: a candidate account-field reference,
    /// validated against the struct's field names by rules.
    FieldRef(Ident),
    /// `Some(inner)`.
    Some(Box<BehaviorArgValue>),
    /// The `None` literal.
    None,
    /// Any other value passed through verbatim (literal, `true`/`false`,
    /// uppercase/multi-segment const or type path).
    Expr(Expr),
}

impl BehaviorArgValue {
    /// Reconstruct the equivalent `syn::Expr`. Every variant tokenizes to a
    /// valid expression, so this construction cannot fail.
    pub(crate) fn as_expr(&self) -> Expr {
        match self {
            BehaviorArgValue::FieldRef(id) => parse_quote!(#id),
            BehaviorArgValue::None => parse_quote!(None),
            BehaviorArgValue::Expr(e) => e.clone(),
            BehaviorArgValue::Some(inner) => {
                let inner = inner.as_expr();
                parse_quote!(Some(#inner))
            }
        }
    }
}

impl ToTokens for BehaviorArgValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.as_expr().to_tokens(tokens);
    }
}

/// User-specified structural assertion.
pub(crate) enum UserCheck {
    HasOne {
        targets: Vec<Ident>,
        error: Option<Expr>,
    },
    Constraints {
        exprs: Vec<Expr>,
        error: Option<Expr>,
    },
}

/// An `address = expr` constraint with its optional custom `@ error`.
///
/// The `@ error` form used to be rerouted into `user_checks`, which dropped the
/// field from the generated `Bumps` struct, the stored-bump fast path, the
/// `{field}_signer` helper, and the IDL PDA resolver. Keeping the error on the
/// address constraint itself preserves all of those while still surfacing the
/// custom error from the verify call.
pub(crate) struct AddressConstraint {
    pub expr: Expr,
    pub error: Option<Expr>,
    /// Whether this is a typed-seeds PDA and, if so, its resolved seeds.
    /// Classified once in lowering so the signer-helper emitter and the IDL
    /// resolver emitter can never disagree on paren/group-wrapped forms.
    pub kind: AddressKind,
}

/// The shape of an `address = expr` constraint, classified once in lowering.
pub(crate) enum AddressKind {
    /// `Path::seeds(args...)` (paren/group tolerant): a typed-seeds PDA. The
    /// account type owns `HasSeeds::SEED_PREFIX`; each arg is a resolved seed.
    Seeds {
        account_ty: syn::Path,
        seeds: Vec<SeedRef>,
    },
    /// Any other address expression (a constant or opaque derivation). The
    /// expression itself lives on `AddressConstraint::expr`.
    Opaque,
}

/// One resolved seed argument of a typed-seeds PDA. Resolution (which account
/// field / instruction arg an ident refers to) happens once in lowering; the
/// IDL emitter maps these directly to `IdlPdaSeed` variants.
pub(crate) enum SeedRef {
    /// `vault.address()` — the address of another account field.
    AccountAddr(Ident),
    /// `vault.owner` (or nested `vault.a.b`) — a field read off another account.
    /// `base` is the account field; `path` is the dotted member path.
    AccountField { base: Ident, path: String },
    /// A bare identifier naming a struct-level `#[instruction(..)]` argument.
    IxArg(Ident),
    /// Any other expression: hashed to bytes at PDA-derivation time.
    Const(Expr),
}

pub(crate) struct FieldSemantics {
    pub core: FieldCore,
    /// `init` / `init(idempotent)`: structural, Phase 1.
    pub init: Option<InitDirective>,
    /// Top-level `payer = field`.
    pub payer: Option<Ident>,
    /// `address = expr [@ error]`: address constraint (plain or typed-seeds).
    pub address: Option<AddressConstraint>,
    /// `realloc = expr`: realloc size expression.
    pub realloc: Option<Expr>,
    /// `close(dest = field)`: core structural close.
    pub close_dest: Option<Ident>,
    /// All behavior groups (open directives: the derive is protocol-neutral).
    pub groups: Vec<BehaviorGroup>,
    /// Structural assertions: has_one, address, constraints.
    pub user_checks: Vec<UserCheck>,
    /// True when the field type is `Migration<From, To>` (syntactic detection
    /// on the last path segment). Proc macros cannot resolve type aliases :
    /// only direct `Migration<From, To>` paths are supported.
    pub is_migration: bool,
    /// True when the field type is `Uninit<T>` (syntactic detection on the
    /// last path segment).
    pub is_uninit: bool,
}

impl FieldSemantics {
    pub fn has_init(&self) -> bool {
        self.init.is_some()
    }

    /// Derived writability. `init`/`realloc`/`close`/`Migration`/`Uninit` all
    /// mutate the account, so they imply writability without an explicit `mut`
    /// (init-implies-mut / realloc-implies-mut).
    pub fn is_writable(&self) -> bool {
        self.core.declared_mut
            || self.has_init()
            || self.is_migration
            || self.is_uninit
            || self.close_dest.is_some()
            || self.realloc.is_some()
    }
}

/// Resolved `writable`/`signer` account-meta flags for a field.
///
/// This is the single source consumed by BOTH the generated client
/// (`client_macro::describe_accounts`) and the IDL accounts-meta fragment
/// (`emit_idl_accounts_meta`), so the two can never disagree.
pub(crate) struct AccountMetaFlags {
    pub writable: bool,
    pub signer: bool,
}

pub(crate) fn account_meta_flags(sem: &FieldSemantics) -> AccountMetaFlags {
    AccountMetaFlags {
        writable: sem.is_writable(),
        // A `Signer<'_>` field is always a signer; a keypair-init account
        // (`init` without an `address`, i.e. a non-PDA created from a freshly
        // generated keypair) must also be signed by the client.
        signer: sem.core.wrapper == WrapperKind::Signer
            || (sem.has_init() && sem.address.is_none()),
    }
}

/// Parsed `init` / `init(idempotent)` directive.
pub(crate) struct InitDirective {
    pub idempotent: bool,
}

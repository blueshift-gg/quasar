use {
    super::wrapper::WrapperKind,
    syn::{Expr, Ident, Type},
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
    pub is_mut: bool,
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

/// A single `key = value` arg in a behavior group directive.
#[derive(Clone)]
pub(crate) struct BehaviorArg {
    pub key: Ident,
    pub value: Expr,
}

/// Classification of a behavior arg value for lowering.
/// Computed by the planner from the field name table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueKind {
    /// Bare identifier matching an account field -> `field.to_account_view()`.
    BareFieldRef,
    /// Bare identifier matching an optional account field ->
    /// `field.as_ref().map(|v| v.to_account_view())`.
    OptionalFieldRef,
    /// Any expression (literal, path, const) -> pass through directly.
    Expr,
    /// `None` literal -> `None`.
    NoneLiteral,
    /// `Some(field)` where field is an account field ->
    /// `Some(field.to_account_view())`.
    SomeFieldRef,
    /// `Some(expr)` where expr is not a field -> `Some(expr)`.
    SomeExpr,
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

    pub fn is_writable(&self) -> bool {
        self.core.is_mut || self.has_init() || self.is_migration || self.is_uninit
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

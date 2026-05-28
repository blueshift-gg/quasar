//! Lowered representations downstream of parser output.

use quasar_syntax::accounts::{BehaviorGroup, InitDirective, UserCheck};
use syn::{Expr, Ident, Type};

/// Account field shape for parsing and account-count planning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Single,
    Composite,
}

pub struct FieldCore {
    pub ident: Ident,
    pub field: syn::Field,
    pub effective_ty: Type,
    pub kind: FieldKind,
    /// Inner/source type for generic wrappers.
    pub inner_ty: Option<Type>,
    pub optional: bool,
    pub dynamic: bool,
    pub is_mut: bool,
    pub dup: bool,
}

/// Classification of a behavior arg value for lowering.
/// Computed by the planner from the field name table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
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

pub struct FieldSemantics {
    pub core: FieldCore,
    /// `init` / `init(idempotent)`: structural, Phase 1.
    pub init: Option<InitDirective>,
    /// Top-level `payer = field`.
    pub payer: Option<Ident>,
    /// `address = expr`: opaque address constraint.
    pub address: Option<Expr>,
    /// `realloc = expr`: realloc size expression.
    pub realloc: Option<Expr>,
    /// `close(dest = field)`: core structural close.
    pub close_dest: Option<Ident>,
    /// All behavior groups (open directives: the derive is protocol-neutral).
    pub groups: Vec<BehaviorGroup>,
    /// Structural assertions: has_one, address, constraints.
    pub user_checks: Vec<UserCheck>,
    /// True when the field type is `Migration<From, To>` (syntactic detection
    /// on the last path segment). Proc macros cannot resolve type aliases:
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

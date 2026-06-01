//! Parser output types for `#[derive(Accounts)]` field attributes.

use syn::{Expr, Ident};

/// One parsed directive from `#[account(...)]` on an `Accounts`-struct field.
///
/// Core directives are structural (owned by the derive); behavior directives
/// are protocol-owned (lowered to trait calls); checks are user-specified
/// structural assertions.
#[derive(Debug)]
pub enum Directive {
    Core(CoreDirective),
    Behavior(BehaviorGroup),
    Check(UserCheck),
}

/// Core structural directives: owned by the derive, not by protocol crates.
// Variants wrap `syn::Expr` (large by nature); these directives are short-lived
// during parsing, so boxing every expr to equalize variants isn't worth the
// churn across the parser, HIR, and derive.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum CoreDirective {
    Mut,
    Dup,
    Group,
    Init { idempotent: bool },
    Payer(Ident),
    Address(Expr, Option<Expr>),
    Realloc(Expr),
    Close(Ident),
}

/// A behavior group directive: `path(key = value, ...)`.
///
/// The derive treats every non-core group as an open behavior group. The path
/// resolves to a Rust module exporting `Args::builder()` and `Behavior`. No
/// protocol-specific knowledge lives in `quasar-syntax` or `quasar-hir`.
#[derive(Clone, Debug)]
pub struct BehaviorGroup {
    pub path: syn::Path,
    pub args: Vec<BehaviorArg>,
}

impl BehaviorGroup {
    /// The path joined by `_`, used for variable naming in generated code.
    pub fn name(&self) -> String {
        self.path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("_")
    }
}

/// A single `key = value` arg in a behavior group directive.
#[derive(Clone, Debug)]
pub struct BehaviorArg {
    pub key: Ident,
    pub value: Expr,
}

/// User-specified structural assertion.
#[allow(clippy::large_enum_variant)] // variants wrap `syn::Expr`; see `CoreDirective`
#[derive(Debug)]
pub enum UserCheck {
    HasOne {
        targets: Vec<Ident>,
        error: Option<Expr>,
    },
    Address {
        expr: Expr,
        error: Option<Expr>,
    },
    Constraints {
        exprs: Vec<Expr>,
        error: Option<Expr>,
    },
}

/// Parsed `init` / `init(idempotent)` directive payload.
#[derive(Debug)]
pub struct InitDirective {
    pub idempotent: bool,
}

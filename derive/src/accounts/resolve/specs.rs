//! Typed execution plan: protocol-neutral phase model.
//!
//! After planning, every field has a `FieldPlan` with phase-ordered steps.
//! All protocol behavior is lowered to generic `BehaviorCall` steps that
//! emit `AccountBehavior` trait calls. No SPL domain knowledge.

use syn::{Expr, Ident, Type};

/// A resolved behavior call for one behavior group on one field.
///
/// The emitter uses this to generate:
/// ```text
/// let __args = path::Args::builder()
///     .key(lowered_value)
///     .build_check()?;
/// <path::Behavior as AccountBehavior<FieldTy>>::check(&field, &__args)?;
/// ```
#[derive(Clone)]
pub(crate) struct BehaviorCall {
    /// Module path for the behavior (e.g., `token`,
    /// `quasar_spl::accounts::token`).
    pub path: syn::Path,
    /// Resolved arguments with lowered values.
    pub args: Vec<LoweredArg>,
    /// Which lifecycle phase this call participates in.
    pub phase: BehaviorPhase,
}

/// A resolved key = value pair with the value already lowered.
#[derive(Clone)]
pub(crate) struct LoweredArg {
    pub key: Ident,
    pub lowered: LoweredValue,
}

/// How a behavior arg value is lowered for codegen.
#[derive(Clone)]
pub(crate) enum LoweredValue {
    /// `field.to_account_view()`: bare field reference.
    FieldView(Ident),
    /// `field.as_ref().map(|v| v.to_account_view())`: optional field
    /// reference.
    OptionalFieldView(Ident),
    /// Pass expression directly.
    Expr(Expr),
    /// `None`.
    NoneLiteral,
    /// `Some(field.to_account_view())`.
    SomeFieldView(Ident),
    /// `Some(expr)`.
    SomeExpr(Expr),
}

/// Behavior lifecycle phase. Each phase maps to one associated const guard,
/// one builder build method, and one trait method call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BehaviorPhase {
    /// `SETS_INIT_PARAMS` -> `build_init()` -> `set_init_param()`
    SetInitParam,
    /// `RUN_AFTER_INIT` -> `build_init()` -> `after_init()`
    AfterInit,
    /// `RUN_CHECK` -> `build_check()` -> `check()`
    Check,
    /// `RUN_UPDATE` -> `build_check()` -> `update()`
    Update,
    /// `RUN_EXIT` -> `build_exit()` -> `exit()`
    Exit,
}

/// A reference that is guaranteed to be a field (never an expression).
/// Used for payer refs where the planner enforces field-only.
#[derive(Clone)]
pub(crate) struct FieldRef {
    pub ident: Ident,
}

/// Plain program account init (no behavior: system program create +
/// discriminator).
#[derive(Clone)]
pub(crate) struct ProgramInitSpec {
    pub payer: FieldRef,
    pub space_ty: Type,
    pub idempotent: bool,
}

/// Delegated init via behavior modules. Pre-load stage only: calls
/// `set_init_param` for each behavior, then `AccountInit::init`. The account
/// is loaded in the normal load phase. `after_init` + `check` run as
/// post-load steps.
#[derive(Clone)]
pub(crate) struct BehaviorInitSpec {
    pub payer: FieldRef,
    pub idempotent: bool,
    /// Behavior calls that contribute init params via `set_init_param`.
    pub init_param_calls: Vec<BehaviorCall>,
}

/// Discriminated init plan.
#[derive(Clone)]
pub(crate) enum InitPlan {
    /// Plain program-owned init (system program create + discriminator).
    Program(ProgramInitSpec),
    /// Behavior-delegated init (set_init_param -> AccountInit::init).
    /// Load, after_init, and check happen in later phases.
    Behavior(BehaviorInitSpec),
}

/// Realloc spec.
#[derive(Clone)]
pub(crate) struct ReallocSpec {
    pub new_space: Expr,
    pub payer: FieldRef,
}

/// Address verification plan for a field.
#[derive(Clone)]
pub(crate) struct AddressSpec {
    pub expr: Expr,
}

/// Program-level close (drain lamports). Core lifecycle: not protocol-owned.
#[derive(Clone)]
pub(crate) struct ProgramCloseSpec {
    pub destination_field: Ident,
}

/// Instruction-wide rent resolution.
#[derive(Clone)]
pub(crate) enum RentPlan {
    /// No step needs rent.
    NotNeeded,
    /// A Sysvar<Rent> field exists: read from it.
    FromSysvarField { field: Ident },
    /// No sysvar field: syscall once.
    FetchOnce,
}

/// A step that runs before account load (address verify + init CPI).
#[derive(Clone)]
pub(crate) enum PreLoadStep {
    VerifyAddress(AddressSpec),
    Init(InitPlan),
}

/// A step that runs after account load.
#[derive(Clone)]
pub(crate) enum PostLoadStep {
    /// Behavior phase call (after_init, check, or update). Guarded by the
    /// phase's associated const at compile time.
    Behavior(BehaviorCall),
    /// Core address verification for non-init fields.
    VerifyExistingAddress(AddressSpec),
    /// Realloc.
    Realloc(ReallocSpec),
}

/// A step that runs in the epilogue.
#[derive(Clone)]
pub(crate) enum EpilogueStep {
    /// Behavior exit phase call. Guarded by `RUN_EXIT` at compile time.
    Behavior(BehaviorCall),
    /// Core program close (lamport drain).
    ProgramClose(ProgramCloseSpec),
}

/// Per-field execution plan. Only phase vectors: structural info lives in
/// FieldSemantics.
#[derive(Clone)]
pub(crate) struct FieldPlan {
    /// Steps before load (init fields only).
    pub pre_load: Vec<PreLoadStep>,
    /// Steps after load (behavior checks/updates, realloc, address verify).
    pub post_load: Vec<PostLoadStep>,
    /// Steps in epilogue (behavior exit, program close).
    pub epilogue: Vec<EpilogueStep>,
}

/// Instruction-wide execution plan.
pub(crate) struct AccountsPlanTyped {
    pub fields: Vec<FieldPlan>,
    pub rent: RentPlan,
}

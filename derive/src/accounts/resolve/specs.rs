//! Typed execution plan: protocol-neutral phase model.
//!
//! After planning, every field has a `FieldPlan` with phase-ordered steps.
//! All protocol behavior is lowered to generic `BehaviorCall` steps that
//! emit `AccountBehavior` trait calls. No SPL domain knowledge.

use {
    super::{
        model::{FieldKind, UserCheck},
        wrapper::WrapperKind,
    },
    syn::{Expr, Ident, Path, Type},
};

/// A resolved behavior call for one behavior group on one field.
///
/// The emitter uses this to generate:
/// ```text
/// let __args = path::Args::builder()
///     .key(lowered_value)
///     .build_check()?;
/// <path::Behavior as AccountBehavior<FieldTy>>::check(&field, &__args)?;
/// ```
///
/// The lifecycle phase is NOT stored here — it is carried by the step that owns
/// the call (`init_param_calls` = SetInitParam, `PostLoadStep::Behavior` =
/// `PostLoadPhase`, `EpilogueStep::Behavior` = Exit), so an out-of-phase call
/// is unrepresentable rather than an ICE.
pub(crate) struct BehaviorCall {
    /// Module path for the behavior (e.g., `token`,
    /// `quasar_spl::accounts::token`).
    pub path: syn::Path,
    /// Resolved arguments with lowered values.
    pub args: Vec<LoweredArg>,
}

/// A resolved key = value pair with the value already lowered.
pub(crate) struct LoweredArg {
    pub key: Ident,
    pub lowered: LoweredValue,
}

/// How a behavior arg value is lowered for codegen.
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
/// one builder build method, and one trait method call. Used by the emitter to
/// select the builder/method; the plan uses phase-scoped step types instead of
/// storing this on a call.
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

/// The subset of behavior phases that can run in the post-load stage. Making
/// this a distinct type means a SetInitParam/Exit call cannot be scheduled
/// post-load — the old `unreachable!` ICE (A10) is now unrepresentable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PostLoadPhase {
    AfterInit,
    Check,
    Update,
}

impl PostLoadPhase {
    pub(crate) fn as_behavior_phase(self) -> BehaviorPhase {
        match self {
            PostLoadPhase::AfterInit => BehaviorPhase::AfterInit,
            PostLoadPhase::Check => BehaviorPhase::Check,
            PostLoadPhase::Update => BehaviorPhase::Update,
        }
    }
}

/// A reference that is guaranteed to be a field (never an expression).
/// Used for payer refs where the planner enforces field-only.
#[derive(Clone)]
pub(crate) struct FieldRef {
    pub ident: Ident,
}

/// One behavior group's identity, carried on `FieldPlan` so the compile-time
/// behavior assertions (REQUIRES_MUT / SETS_INIT_PARAMS / RUN_AFTER_INIT /
/// VALIDATES_ACCOUNT_DATA / DEFAULT_INIT_PARAMS_VALID) are emitted from the
/// plan. `name` is `BehaviorGroup::name()` captured once so the assertion
/// messages match lowering exactly.
pub(crate) struct BehaviorGroupRef {
    pub path: Path,
    pub name: String,
    /// Behavior argument names mapped to concrete account fields for host-only
    /// IDL address resolution.
    pub idl_account_args: Vec<BehaviorIdlAccountArg>,
}

pub(crate) struct BehaviorIdlAccountArg {
    pub key: String,
    /// camelCase field name for IDL resolution.
    pub field: String,
    /// The referenced field's original identifier, for client codegen.
    pub field_ident: Ident,
}

/// Plain program account init (no behavior: system program create +
/// discriminator).
pub(crate) struct ProgramInitSpec {
    pub payer: FieldRef,
    pub space_ty: Type,
    pub idempotent: bool,
    /// When this init field also has an `address` constraint, the preceding
    /// `VerifyAddress` step stored `__addr_{f}`/`__bumps_{f}`; init then signs
    /// with those seeds. `Some` records that cross-step dependency (the address
    /// itself is verified by the `VerifyAddress` step).
    pub verified_address: Option<AddressSpec>,
}

/// Delegated init via behavior modules. Pre-load stage only: calls
/// `set_init_param` for each behavior, then `AccountInit::init`. The account
/// is loaded in the normal load phase. `after_init` + `check` run as
/// post-load steps.
pub(crate) struct BehaviorInitSpec {
    pub payer: FieldRef,
    pub idempotent: bool,
    /// Behavior calls that contribute init params via `set_init_param`.
    pub init_param_calls: Vec<BehaviorCall>,
    /// See `ProgramInitSpec::verified_address`.
    pub verified_address: Option<AddressSpec>,
}

/// Discriminated init plan.
pub(crate) enum InitPlan {
    /// Plain program-owned init (system program create + discriminator).
    Program(ProgramInitSpec),
    /// Behavior-delegated init (set_init_param -> AccountInit::init).
    /// Load, after_init, and check happen in later phases.
    Behavior(BehaviorInitSpec),
}

/// Realloc spec.
pub(crate) struct ReallocSpec {
    pub new_space: Expr,
    pub payer: FieldRef,
}

/// Address verification plan for a field.
pub(crate) struct AddressSpec {
    pub expr: Expr,
    /// Optional custom `@ error` mapped onto the verify call's failure.
    pub error: Option<Expr>,
}

/// A field carries a generated stored-bump slot (`__bumps_{f}: u8` and a `u8`
/// field in the `Bumps` struct) exactly when it has an `address` constraint.
/// Marker type: the slot's name derives from `FieldPlan::ident`/`optional`.
pub(crate) struct BumpSlot;

/// Program-level close (drain lamports). Core lifecycle: not protocol-owned.
pub(crate) struct ProgramCloseSpec {
    pub destination_field: Ident,
}

/// Resolver for one IDL account-meta node. Built once in the planner so the IDL
/// emitter is a pure formatter and never reclassifies wrapper/address syntax.
pub(crate) enum IdlResolverPlan {
    /// A wrapper whose inner type provides one canonical address.
    FixedAddress {
        inner_ty: Type,
        source: FixedAddressSource,
    },
    /// A typed-seeds PDA with every seed resolved against accounts/args.
    Pda {
        /// The PDA account type (owns `HasSeeds::SEED_PREFIX`).
        account_ty: Path,
        seeds: Vec<IdlSeedPlan>,
    },
}

/// Trait that owns the canonical ID for a fixed-address wrapper.
#[derive(Clone, Copy)]
pub(crate) enum FixedAddressSource {
    /// `Program<T>` reads `<T as Id>::ID`.
    Program,
    /// `Sysvar<T>` reads `<T as Sysvar>::ID`.
    Sysvar,
}

/// One resolved IDL PDA seed.
pub(crate) enum IdlSeedPlan {
    /// `base.address()`: the address of another account field.
    AccountAddr { base: Ident },
    /// `base.field`: a field read off another account, with the account type
    /// name resolved from the base field's inner type.
    AccountField {
        base: Ident,
        account: String,
        field: String,
    },
    /// A struct-level `#[instruction(..)]` argument, with its type resolved for
    /// the IDL layout.
    IxArg { name: Ident, ty: Type },
    /// Any other expression hashed to bytes at derivation time.
    Const { expr: Expr },
}

/// The `{field}_signer` helper for a Single field with a typed-seeds address.
/// Carries the address expression; the method/field names derive from
/// `FieldPlan::ident`.
pub(crate) struct SignerHelperPlan {
    pub addr_expr: Expr,
    /// The seeds-owning account type; the helper returns its concrete
    /// `HasSeeds::WithBump` set so callers reach `signer_seeds`.
    pub set_ty: Path,
}

/// Instruction-wide rent resolution.
pub(crate) enum RentPlan {
    /// No step needs rent.
    NotNeeded,
    /// A Sysvar<Rent> field exists: read from it.
    FromSysvarField { field: Ident },
    /// No sysvar field: syscall once.
    FetchOnce,
}

/// How a field is loaded in the load phase. Encodes the load-mode selection
/// (dynamic wrapper vs `AccountLoad`) and the `VALIDATES_ACCOUNT_DATA` guard,
/// previously derived in `emit/parse.rs` from `FieldSemantics`.
pub(crate) enum LoadStep {
    /// Dynamic-layout wrapper: `<base_ty>::from_account_view(ident)?`.
    /// `base_ty` is the wrapper's inner type (generics are stripped at emit
    /// time).
    Dynamic { base_ty: Type },
    /// Fixed-layout account loaded via `AccountLoad::load*`. `validates_paths`
    /// are the field's behavior-group paths; when non-empty the load is guarded
    /// by their `VALIDATES_ACCOUNT_DATA` to pick the intrinsic path. The
    /// checked/mut variant is selected from `FieldPlan::dup`/`writable`.
    Fixed { validates_paths: Vec<Path> },
}

/// A step that runs before account load (address verify + init CPI).
pub(crate) enum PreLoadStep {
    VerifyAddress(Box<AddressSpec>),
    Init(Box<InitPlan>),
}

/// A step that runs after account load.
pub(crate) enum PostLoadStep {
    /// Behavior phase call (after_init, check, or update). Guarded by the
    /// phase's associated const at compile time.
    Behavior {
        phase: PostLoadPhase,
        call: BehaviorCall,
    },
    /// Structural user assertion (`has_one` / `constraints`), run after load.
    UserCheck(UserCheck),
    /// Core address verification for non-init fields.
    VerifyExistingAddress(AddressSpec),
    /// Realloc.
    Realloc(ReallocSpec),
}

/// A step that runs in the epilogue.
pub(crate) enum EpilogueStep {
    /// Behavior exit phase call. Guarded by `RUN_EXIT` at compile time.
    Behavior(BehaviorCall),
    /// Core program close (lamport drain).
    ProgramClose(ProgramCloseSpec),
}

/// Per-field execution plan. Carries every structural fact and phase-ordered
/// step the emitter needs, so emit consumes ONLY the plan (never
/// `FieldSemantics`). Field `i` corresponds to lowering's semantics field `i`.
pub(crate) struct FieldPlan {
    /// The field's identifier.
    pub ident: Ident,
    /// The field's effective (wrapper) type, e.g. `Account<'a, Vault>`.
    pub effective_ty: Type,
    /// Which library wrapper `effective_ty` is (last-segment match).
    pub wrapper: WrapperKind,
    /// Single account vs a composite (`#[derive(Accounts)]` group).
    pub kind: FieldKind,
    /// `Option<..>`-wrapped account field.
    pub optional: bool,
    /// `#[account(dup)]`: duplicate-alias tolerant.
    pub dup: bool,
    /// Derived writability (`FieldSemantics::is_writable`), computed once here.
    pub writable: bool,
    /// Account-meta signer flag (`account_meta_flags().signer`): the single
    /// core requirement shared by the client macro and IDL metadata.
    pub signer: bool,
    /// Whether the signer requirement for a delegated, non-PDA init must be
    /// resolved from the attached behaviors' `INIT_REQUIRES_SIGNER` contract.
    pub behavior_init_signer: bool,
    /// How this field is loaded (single fields only; composites parse via
    /// their own `ParseAccountsUnchecked` impl).
    pub load: LoadStep,
    /// A stored-bump slot when the field has an `address` constraint. Drives
    /// the `__bumps_{f}` local and the field's entry in the `Bumps` struct.
    pub bump: Option<BumpSlot>,
    /// The field's behavior groups (declaration order), for the compile-time
    /// behavior assertions.
    pub behaviors: Vec<BehaviorGroupRef>,
    /// Doc-comment lines for the IDL account node (`/// ...` on the field).
    pub docs: Vec<String>,
    /// Fixed-address or typed-seeds resolver for the IDL account-meta fragment.
    pub idl_resolver: Option<IdlResolverPlan>,
    /// The `{field}_signer` helper, emitted for Single fields with a
    /// typed-seeds address.
    pub signer_helper: Option<SignerHelperPlan>,
    /// Steps before load (init fields only).
    pub pre_load: Vec<PreLoadStep>,
    /// Steps after load (behavior checks/updates, realloc, address verify).
    pub post_load: Vec<PostLoadStep>,
    /// Steps in epilogue (behavior exit, program close).
    pub epilogue: Vec<EpilogueStep>,
}

impl FieldPlan {
    /// Whether this field is initialized (an `Init` step is scheduled
    /// pre-load).
    pub(crate) fn has_init(&self) -> bool {
        self.pre_load
            .iter()
            .any(|step| matches!(step, PreLoadStep::Init(_)))
    }
}

/// One field's contribution to the instruction-wide `NEEDS_EVENT_CPI`
/// OR-chain.
pub(crate) enum EventCpiTerm {
    /// A single field that never enables event CPI: contributes `false`.
    Never,
    /// The event-authority field: contributes `true`.
    EventAuthority,
    /// A composite: delegates to its inner `AccountCount::NEEDS_EVENT_CPI`.
    /// Carries the field's effective type (the inner type is derived at emit).
    Composite(Box<Type>),
}

/// Instruction-wide execution plan.
pub(crate) struct AccountsPlanTyped {
    pub fields: Vec<FieldPlan>,
    pub rent: RentPlan,
    /// Whether the instruction declares struct-level `#[instruction(..)]` args
    /// (the ix-arg-extraction need).
    pub has_instruction_args: bool,
    /// Per-field `NEEDS_EVENT_CPI` contributions, in field order.
    pub event_cpi: Vec<EventCpiTerm>,
}

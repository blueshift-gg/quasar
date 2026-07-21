use solana_program_error::ProgramError;

/// Protocol-owned account behavior attached via `#[account(my_behavior(...))]`.
///
/// This is the stable extension seam for protocol-owned plugins.
///
/// # Writing a behavior module
///
/// A behavior group `#[account(foo(a = x, b = y))]` requires a module `foo`
/// exporting:
///
/// - `foo::Args`: the args struct
/// - `foo::Args::builder()`: returns an `ArgsBuilder` with `.a()`, `.b()`
///   setters, implementing [`BehaviorArgsBuilder`] (`build_init` /
///   `build_check` / `build_exit`)
/// - `foo::Behavior`: a unit struct implementing `AccountBehavior<T>` for each
///   supported account wrapper type
///
/// Everything is built from the stable API
/// (`quasar_lang::account_behavior::{AccountBehavior, BehaviorArgsBuilder}`):
///
/// ```text
/// use quasar_lang::account_behavior::{AccountBehavior, BehaviorArgsBuilder};
/// use quasar_lang::prelude::*;
///
/// pub struct Args<'a> { pub authority: &'a AccountView }
///
/// pub struct ArgsBuilder<'a> { authority: Option<&'a AccountView> }
/// impl<'a> Args<'a> {
///     pub fn builder() -> ArgsBuilder<'a> { ArgsBuilder { authority: None } }
/// }
/// impl<'a> ArgsBuilder<'a> {
///     pub fn authority(mut self, v: &'a AccountView) -> Self { self.authority = Some(v); self }
/// }
/// impl<'a> BehaviorArgsBuilder for ArgsBuilder<'a> {
///     type Init = Args<'a>;
///     type Check = Args<'a>;
///     type Exit = Args<'a>;
///     fn build_init(self) -> Result<Args<'a>, ProgramError> { self.build_check() }
///     fn build_check(self) -> Result<Args<'a>, ProgramError> {
///         Ok(Args { authority: self.authority.ok_or(ProgramError::InvalidArgument)? })
///     }
///     fn build_exit(self) -> Result<Args<'a>, ProgramError> { self.build_check() }
/// }
///
/// pub struct Behavior;
/// impl<T> AccountBehavior<T> for Behavior {
///     type Args<'a> = Args<'a>;
///     fn check<'a>(_account: &T, _args: &Args<'a>) -> Result<(), ProgramError> { Ok(()) }
/// }
/// ```
///
/// # Lifecycle phases
///
/// Each phase is guarded by an associated const. The derive only emits code
/// for phases where the const is `true`.
///
/// ```text
/// Phase           Const             Builder      Trait method       When
/// --------------- ----------------- ------------ ------------------ ----------
/// set_init_param  SETS_INIT_PARAMS  build_init   set_init_param()  init fields
/// after_init      RUN_AFTER_INIT    build_init   after_init()      init fields
/// check           RUN_CHECK         build_check  check()           all fields
/// update          RUN_UPDATE        build_check  update()          mut fields
/// exit            RUN_EXIT          build_exit   exit()            mut fields (epilogue)
/// ```
///
/// Default methods are no-ops. Override only the methods your behavior needs.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a behavior for `{A}`",
    label = "no `AccountBehavior<{A}>` impl",
    note = "behavior groups like `token(...)` require a module exposing a `Behavior` type that \
            implements `AccountBehavior` for this account"
)]
pub trait AccountBehavior<A> {
    /// Behavior arguments for one lifecycle phase.
    type Args<'a>;

    /// Whether `set_init_param` contributes init parameters for `A`.
    /// The derive asserts at most one attached behavior group per field has
    /// this set to `true`.
    const SETS_INIT_PARAMS: bool = false;

    /// Whether a non-PDA account initialized through this behavior must sign
    /// the outer transaction.
    ///
    /// Most account creation paths create the passed account directly and
    /// therefore require its keypair signature. Derived-address protocols,
    /// such as associated-token accounts, override this to `false` because
    /// their program creates the account without a client-held keypair.
    const INIT_REQUIRES_SIGNER: bool = true;

    /// Optional address recipe exported to generated clients through the IDL.
    ///
    /// Behaviors that validate a derived address should describe it here so
    /// callers do not have to supply an address the program can derive. The
    /// recipe names behavior arguments rather than account fields, keeping the
    /// behavior reusable when programs choose different field names.
    const IDL_RESOLVER: Option<BehaviorIdlResolver> = None;

    /// Whether `after_init` runs after account creation.
    const RUN_AFTER_INIT: bool = false;

    /// Whether `check` runs after account load.
    const RUN_CHECK: bool = true;

    /// Whether a successful fresh init through this behavior establishes the
    /// same invariants as `check`.
    const INIT_SATISFIES_CHECK: bool = false;

    /// Whether this behavior validates the target account's data.
    ///
    /// When true, generated parsing may use the target account type's cheaper
    /// intrinsic pre-load path and rely on this behavior's semantic validation
    /// to complete account-data checks before the parsed accounts are returned.
    const VALIDATES_ACCOUNT_DATA: bool = false;

    /// Whether this behavior consumes the given behavior arg in the given
    /// lifecycle phase.
    ///
    /// Derive uses this to avoid building phase-local args that a concrete
    /// behavior impl does not read. The default keeps all existing behavior
    /// modules source-compatible.
    #[inline(always)]
    fn uses_arg<const PHASE: u8, const KEY: u64>() -> bool {
        true
    }

    /// Whether an exit-phase account argument identifies a PDA authority that
    /// this behavior can sign for.
    ///
    /// The accounts derive uses this opt-in together with the referenced
    /// field's typed `#[seeds(...)]` address to construct signer seeds from the
    /// already-verified bump. Behaviors that opt in must perform their CPI in
    /// [`AccountBehavior::exit_signed`].
    #[inline(always)]
    fn uses_exit_signer_arg<const KEY: u64>() -> bool {
        false
    }

    /// Whether `update` runs after validation (requires `#[account(mut)]`).
    const RUN_UPDATE: bool = false;

    /// Whether `exit` runs in the epilogue (requires `#[account(mut)]`).
    const RUN_EXIT: bool = false;

    /// Whether the target field must be mutable for this behavior.
    /// Defaults to `RUN_UPDATE || RUN_EXIT`.
    const REQUIRES_MUT: bool = Self::RUN_UPDATE || Self::RUN_EXIT;

    /// Applies this behavior's values to the account's initialization
    /// parameters.
    fn set_init_param<'a>(
        _params: &mut <A as crate::account_init::AccountInit>::InitParams<'a>,
        _args: &Self::Args<'a>,
    ) -> Result<(), ProgramError>
    where
        A: crate::account_init::AccountInit,
    {
        Ok(())
    }

    /// Offer a conventionally named account field to init arguments.
    ///
    /// The default ignores it. Protocol behaviors may fill omitted
    /// boilerplate accounts while preserving any value the user supplied
    /// explicitly through the behavior builder.
    #[inline(always)]
    fn infer_init_account<'a, const KEY: u64>(
        _args: &mut Self::Args<'a>,
        _account: &'a crate::__internal::AccountView,
    ) {
    }

    /// Runs immediately after a new account has been initialized.
    fn after_init<'a>(_account: &mut A, _args: &Self::Args<'a>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Validates the loaded account against this behavior's arguments.
    fn check<'a>(_account: &A, _args: &Self::Args<'a>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Applies a behavior-specific mutation after validation.
    fn update<'a>(_account: &mut A, _args: &Self::Args<'a>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Runs this behavior's epilogue before instruction completion.
    fn exit<'a>(_account: &mut A, _args: &Self::Args<'a>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Runs the epilogue with signer seeds for a typed PDA authority.
    ///
    /// The default delegates to [`AccountBehavior::exit`], preserving source
    /// compatibility for existing behaviors. A behavior opts into signed CPI
    /// by returning `true` from [`AccountBehavior::uses_exit_signer_arg`] and
    /// overriding this method.
    fn exit_signed<'a, S>(
        account: &mut A,
        args: &Self::Args<'a>,
        _signer: &S,
    ) -> Result<(), ProgramError>
    where
        S: crate::cpi::CpiSignerSeeds + ?Sized,
    {
        Self::exit(account, args)
    }
}

/// Client-address metadata supplied by a protocol-owned account behavior.
///
/// This is metadata only. The protocol crate still owns all validation and
/// lifecycle logic; Quasar maps the named behavior arguments into the public
/// IDL during the host-only IDL build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorIdlResolver {
    /// An associated token account derived from mint, owner, and optionally a
    /// caller-selected token program.
    AssociatedToken {
        /// Behavior argument containing the mint account.
        mint: &'static str,
        /// Behavior argument containing the wallet or PDA owner.
        owner: &'static str,
        /// Behavior argument containing the token program. `None` selects the
        /// canonical SPL Token program in generated clients.
        token_program: Option<&'static str>,
    },
}

/// The builder contract for a behavior module's phase arguments.
///
/// A behavior module's `Args::builder()` returns a builder implementing this
/// trait; the derive calls `build_init` / `build_check` / `build_exit` to
/// assemble the per-phase argument struct. Making this a trait (rather than
/// duck-typed inherent methods) means a plugin whose builder is missing a phase
/// fails to compile with a clear diagnostic pointing at this trait.
///
/// The three associated types are usually the same `Args` struct, but the
/// contract allows a builder to produce a different shape per phase.
pub trait BehaviorArgsBuilder {
    /// Arguments for the init phases (`set_init_param`, `after_init`).
    type Init;
    /// Arguments for the read phases (`check`, `update`).
    type Check;
    /// Arguments for the epilogue `exit` phase.
    type Exit;

    /// Assemble arguments for the init phases.
    fn build_init(self) -> Result<Self::Init, ProgramError>;
    /// Assemble arguments for the read phases.
    fn build_check(self) -> Result<Self::Check, ProgramError>;
    /// Assemble arguments for the epilogue `exit` phase.
    fn build_exit(self) -> Result<Self::Exit, ProgramError>;
}

/// Phase id passed to `uses_arg` for `set_init_param`.
pub const ARG_PHASE_SET_INIT_PARAM: u8 = 0;
/// Phase id passed to `uses_arg` for `after_init`.
pub const ARG_PHASE_AFTER_INIT: u8 = 1;
/// Phase id passed to `uses_arg` for `check`.
pub const ARG_PHASE_CHECK: u8 = 2;
/// Phase id passed to `uses_arg` for `update`.
pub const ARG_PHASE_UPDATE: u8 = 3;
/// Phase id passed to `uses_arg` for `exit`.
pub const ARG_PHASE_EXIT: u8 = 4;

/// Stable FNV-1a hash for behavior argument keys used in `uses_arg` const
/// generics.
pub const fn behavior_arg_key_hash(key: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let bytes = key.as_bytes();
    let mut hash = FNV_OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

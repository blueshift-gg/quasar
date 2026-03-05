use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::ProgramError;

/// Construct a typed account wrapper from a raw [`AccountView`].
///
/// Implemented by the `define_account!` macro and by `Account<T>` / `Initialize<T>`.
pub trait FromAccountView<'info>: Sized {
    fn from_account_view(view: &'info AccountView) -> Result<Self, ProgramError>;
}

/// Declares the expected on-chain owner for an account type.
///
/// Implemented by: `#[account]` derive macro.
/// Used by: `Account<T>` to validate owner during parsing.
pub trait Owner {
    const OWNER: Address;
}

/// Declares the on-chain address (ID) for a program type.
///
/// This trait simply provides the program's address constant. The `Program<T>`
/// wrapper type requires `T: Id` to validate that accounts match the expected address.
///
/// Implemented by: Program marker types (e.g., `System`, `Token`).
/// Used by: `Program<T>` wrapper for address validation.
pub trait Id {
    const ID: Address;
}

/// Declares that a type represents a program interface accepting multiple program IDs.
///
/// Unlike [`Program`] which represents a single program, this trait allows
/// checking against multiple valid program addresses (e.g., Token vs Token-2022).
///
/// Implemented by: Manual `impl` for interface types.
/// Used by: `Interface<T>` to validate the address during parsing.
pub trait ProgramInterface {
    /// Check if the given address matches any valid program ID for this interface.
    fn matches(address: &Address) -> bool;
}

/// Declares the byte-level discriminator prefix for an account or instruction.
///
/// Discriminators are developer-specified (not sha256-derived). All-zero
/// discriminators are rejected at compile time to prevent uninitialized
/// account data from passing validation.
///
/// Implemented by: `#[account]` and `#[instruction]` derive macros.
pub trait Discriminator {
    const DISCRIMINATOR: &'static [u8];
}

/// Declares the total byte size of an account's data (including discriminator prefix).
///
/// Implemented by: `#[account]` derive macro.
/// Used by: `create_account` CPI to allocate the correct account size.
pub trait Space {
    const SPACE: usize;
}

/// Runtime validation hook called during account parsing.
///
/// The default implementation is a no-op. Override to add custom checks
/// (e.g. verifying a mint address or checking account state).
///
/// Implemented by: `define_account!` macro (for check trait composition),
/// manual `impl` for custom validation.
pub trait AccountCheck {
    #[inline(always)]
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

/// Declares the number of accounts consumed by a struct during parsing.
///
/// Implemented by: `#[derive(Accounts)]` macro.
/// Used by: `dispatch!` macro to size the `MaybeUninit` account buffer.
pub trait AccountCount {
    const COUNT: usize;
}

/// Parse and validate a set of accounts from a raw `AccountView` slice.
///
/// Implemented by: `#[derive(Accounts)]` macro.
/// Called by: `Ctx::new()` during instruction dispatch.
///
/// Returns the parsed struct and a `Bumps` companion containing any PDA bump
/// seeds discovered during validation.
pub trait ParseAccounts<'info>: Sized {
    type Bumps: Copy;
    fn parse(
        accounts: &'info [AccountView],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError>;

    /// Parse accounts with access to instruction data.
    ///
    /// When `#[instruction(args)]` is present on the Accounts struct, the
    /// derived impl deserializes declared args from `data` and makes them
    /// available during account initialization (e.g. for `metadata::name`).
    ///
    /// The default implementation ignores `data` and delegates to [`parse`].
    #[inline(always)]
    fn parse_with_instruction_data(
        accounts: &'info [AccountView],
        _data: &'info [u8],
        program_id: &Address,
    ) -> Result<(Self, Self::Bumps), ProgramError> {
        Self::parse(accounts, program_id)
    }

    #[inline(always)]
    fn epilogue(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

/// Convert a typed account wrapper to its underlying [`AccountView`].
///
/// All account types (`Account<T>`, `Signer`, `UncheckedAccount`, etc.)
/// implement this trait to provide uniform access to the raw account data.
pub trait AsAccountView {
    fn to_account_view(&self) -> &AccountView;

    #[inline(always)]
    fn address(&self) -> &Address {
        self.to_account_view().address()
    }
}

impl AsAccountView for AccountView {
    #[inline(always)]
    fn to_account_view(&self) -> &AccountView {
        self
    }
}

/// Borsh-style serialization for non-zero-copy account types.
///
/// Implemented by: `#[account]` macro when the type is not `#[repr(C)]`.
/// Used by: `Account<T>::get()` and `Account<T>::set()`.
pub trait QuasarAccount: Sized + Discriminator + Space {
    fn deserialize(data: &[u8]) -> Result<Self, ProgramError>;
    fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError>;
}

/// Validate that an account is owned by the expected program(s).
///
/// Single-owner types get a blanket implementation via [`Owner`].
/// Multi-owner (interface) types implement this directly with explicit
/// comparison chains, avoiding the ~20-40 CU cost of slice iteration.
pub trait CheckOwner {
    fn check_owner(view: &AccountView) -> Result<(), ProgramError>;
}

impl<T: Owner> CheckOwner for T {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        // SAFETY: owner() returns &Address from the SVM buffer. Called during
        // account parsing before any handler runs.
        if !crate::keys_eq(unsafe { view.owner() }, &T::OWNER) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

/// Polymorphic zero-copy dispatch for interface account types.
///
/// When an account can be owned by multiple programs with different layouts
/// or behaviors, `InterfaceResolve` provides a way to dispatch to the
/// correct resolved type based on the runtime owner.
pub trait InterfaceResolve {
    type Resolved<'a>;
    fn resolve<'a>(view: &'a AccountView) -> Result<Self::Resolved<'a>, ProgramError>;
}

/// Zero-copy deref target for `#[repr(C)]` account types.
///
/// When an account type implements `ZeroCopyDeref`, `Account<T>` provides
/// `Deref`/`DerefMut` to `T::Target` via `deref_from` / `deref_from_mut`.
///
/// For fixed-size accounts, `Target` is the ZC companion struct and the
/// methods perform a pointer cast past the discriminator.
///
/// For dynamic accounts, `Target` is a generated View type (`{Name}View`)
/// that is `#[repr(transparent)]` over `AccountView`. The View type
/// provides accessors for dynamic fields and derefs further to the ZC
/// struct for fixed field access.
///
/// Implemented by: `#[account]` macro.
pub trait ZeroCopyDeref {
    type Target;
    fn deref_from(view: &AccountView) -> &Self::Target;
    #[allow(clippy::mut_from_ref)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target;
}

/// On-chain event with a discriminator, fixed-size data, and emission logic.
///
/// Events support dual emission:
/// - `emit!()` via `sol_log_data` (~100 CU) — fast but spoofable
/// - `emit_cpi!()` via self-CPI (~1,000 CU) — spoofing-resistant
///
/// Implemented by: `#[event]` derive macro.
pub trait Event {
    const DISCRIMINATOR: &'static [u8];
    const DATA_SIZE: usize;
    fn write_data(&self, buf: &mut [u8]);
    fn emit(&self, f: impl FnOnce(&[u8]) -> Result<(), ProgramError>) -> Result<(), ProgramError>;
}

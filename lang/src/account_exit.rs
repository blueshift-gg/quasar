use {
    crate::{
        accounts::account::{resize, set_lamports},
        cpi::system::SYSTEM_PROGRAM_ID,
    },
    solana_account_view::AccountView,
    solana_program_error::{ProgramError, ProgramResult},
};

/// Context for closing an account.
///
/// `authority` and `token_program` are `Option` — used by token/SPL close
/// paths, ignored by program-owned close.
pub struct CloseCtx<'a> {
    pub destination: &'a AccountView,
    /// Token close authority. `None` for program-owned accounts.
    pub authority: Option<&'a AccountView>,
    /// Token program for CPI close. `None` for program-owned accounts.
    pub token_program: Option<&'a AccountView>,
}

/// Context for sweeping tokens from an account before closing.
pub struct SweepCtx<'a> {
    pub receiver: &'a AccountView,
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

/// Account exit lifecycle: close and sweep.
///
/// Implemented on the behavior target (Token, Mint, `#[account]` types).
///
/// # Safety Contract
///
/// Implementations of `close()` MUST ensure the account's discriminator is
/// invalidated before or atomically with the lamport drain. Failure creates
/// a revival attack window where a concurrent transaction can re-fund a
/// closed account that still has a valid discriminator.
///
/// - **Program-owned**: use [`close_program_account()`] which zeros the
///   discriminator FIRST, then drains lamports.
/// - **CPI-based** (SPL Token): the target program handles the drain
///   atomically.
pub trait AccountExit {
    fn close(view: &mut AccountView, ctx: CloseCtx<'_>) -> ProgramResult;

    /// Sweep transfers all tokens out before closing. Only implemented by
    /// token account types. Takes shared `&AccountView` — sweep only reads
    /// account data and issues a CPI.
    fn sweep(_view: &AccountView, _ctx: SweepCtx<'_>) -> ProgramResult {
        Err(ProgramError::InvalidAccountData)
    }
}

/// Close a program-owned account: zero discriminator, drain lamports, reassign
/// to system program, resize to zero.
///
/// Ordering: discriminator zeroed first to prevent revival attacks.
#[inline(always)]
pub fn close_program_account(
    account: &mut AccountView,
    destination: &AccountView,
    disc_len: usize,
) -> ProgramResult {
    if crate::utils::hint::unlikely(!destination.is_writable()) {
        return Err(ProgramError::Immutable);
    }

    // SAFETY: parse verified data_len >= disc_len.
    unsafe { core::ptr::write_bytes(account.data_mut_ptr(), 0, disc_len) };

    let new_lamports = destination.lamports().wrapping_add(account.lamports());
    set_lamports(destination, new_lamports);
    account.set_lamports(0);

    unsafe { account.assign(&SYSTEM_PROGRAM_ID) };

    resize(account, 0)?;
    Ok(())
}

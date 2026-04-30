//! Close-program op: Phase 4 epilogue account close for program-owned accounts.
//!
//! `close_program::Op` calls `AccountClose::close` on the field's behavior
//! target to zero the discriminator, drain lamports, and reassign to system.

use {
    super::{AccountOp, OpCtx},
    crate::account_load::AccountLoad,
    solana_account_view::AccountView,
    solana_program_error::{ProgramError, ProgramResult},
};

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
    unsafe { core::ptr::write_bytes(account.data_mut_ptr(), 0, disc_len) };
    let new_lamports = destination.lamports().wrapping_add(account.lamports());
    crate::accounts::set_lamports(destination, new_lamports);
    account.set_lamports(0);
    unsafe { account.assign(&crate::cpi::system::SYSTEM_PROGRAM_ID) };
    crate::accounts::resize(account, 0)?;
    Ok(())
}

/// Trait for program-owned accounts that can be closed.
///
/// Closes an account by zeroing its discriminator, draining lamports to `dest`,
/// reassigning ownership to the system program, and resizing data to zero.
///
/// Implemented by `#[account]` macro for non-custom account types.
pub trait AccountClose {
    fn close(view: &mut AccountView, dest: &AccountView) -> ProgramResult;
}

/// Close operation for program-owned accounts. Constructed by the derive
/// macro from `exit(close(...))` syntax when the field type implements
/// `AccountClose` (not `TokenClose`).
pub struct Op<'a> {
    pub dest: &'a AccountView,
}

impl<'a, F: AccountLoad> AccountOp<F> for Op<'a>
where
    <F as AccountLoad>::BehaviorTarget: AccountClose,
{
    const REQUIRES_MUT: bool = true;
    const HAS_EXIT: bool = true;

    #[inline(always)]
    fn exit(&self, field: &mut F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        type Target<F2> = <F2 as AccountLoad>::BehaviorTarget;
        let view = unsafe { <F as AccountLoad>::to_account_view_mut(field) };
        <Target<F> as AccountClose>::close(view, self.dest)
    }
}

//! Close epilogue for program-owned accounts.
//!
//! The derive emits a `close::Op { disc_len }.apply(view, dest)` call in the
//! epilogue, mirroring the config-vs-slot split of
//! [`init::Op`](crate::ops::init::Op)
//! and [`realloc::Op`](crate::ops::realloc::Op).

use {
    solana_account_view::AccountView,
    solana_program_error::{ProgramError, ProgramResult},
};

/// Runtime form of a `close(destination)` field directive.
///
/// The config (`disc_len`) is carried on the struct; `apply` operates on the
/// account slot, mirroring [`init::Op`](crate::ops::init::Op) and
/// [`realloc::Op`](crate::ops::realloc::Op). Close needs no
/// [`OpCtx`](crate::ops::OpCtx): it reads neither rent nor the program id.
pub struct Op {
    /// Discriminator length to zero before draining and reassigning.
    pub disc_len: usize,
}

impl Op {
    /// Close `account`, draining its lamports into `destination`.
    #[inline(always)]
    pub fn apply(&self, account: &mut AccountView, destination: &AccountView) -> ProgramResult {
        close_account(account, destination, self.disc_len)
    }
}

/// Zero the discriminator, drain lamports, assign to the system program, then
/// resize the account to zero bytes.
///
/// The discriminator is cleared first so any later failure leaves the account
/// unusable as typed program state.
///
/// Returns [`ProgramError::InvalidArgument`] if `destination` is the same
/// account as `account`.
#[inline(always)]
pub fn close_account(
    account: &mut AccountView,
    destination: &AccountView,
    disc_len: usize,
) -> ProgramResult {
    if crate::utils::hint::unlikely(!destination.is_writable()) {
        return Err(ProgramError::Immutable);
    }
    // A self-close sums the drained lamports back into the same account and
    // then zeroes them, destroying the balance (`UnbalancedInstruction`). A
    // duplicate meta shares one backing account, so reject by pointer identity
    // before any mutation leaves the account in a partially-closed state.
    if crate::utils::hint::unlikely(core::ptr::eq(
        account.account_ptr(),
        destination.account_ptr(),
    )) {
        return Err(ProgramError::InvalidArgument);
    }
    // SAFETY: Callers only close accounts that have already passed the normal
    // account load path, so the discriminator prefix is present and writable.
    unsafe { core::ptr::write_bytes(account.data_mut_ptr(), 0, disc_len) };
    let new_lamports = destination.lamports().wrapping_add(account.lamports());
    crate::accounts::set_lamports(destination, new_lamports);
    account.set_lamports(0);
    // SAFETY: The account no longer carries lamports or valid typed state; the
    // final close steps hand ownership back to the system program before the
    // data region is truncated.
    unsafe { account.assign(&crate::cpi::system::SYSTEM_PROGRAM_ID) };
    crate::accounts::resize(account, 0)?;
    Ok(())
}

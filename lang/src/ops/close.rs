//! Close epilogue for program-owned accounts.
//!
//! The derive emits direct `close_account(view, dest, disc_len)` calls in the
//! epilogue.

use {
    solana_account_view::AccountView,
    solana_program_error::{ProgramError, ProgramResult},
};

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

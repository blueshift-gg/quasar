//! Account close: epilogue close for program-owned accounts.
//!
//! The derive emits direct `close_account(view, dest, disc_len)` calls in the
//! epilogue.

use {
    solana_account_view::AccountView,
    solana_program_error::{ProgramError, ProgramResult},
};

/// Close a program-owned account: zero discriminator, drain lamports, reassign
/// to system program, resize to zero.
///
/// Ordering: discriminator zeroed first to prevent revival attacks.
#[inline(always)]
pub fn close_account(
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

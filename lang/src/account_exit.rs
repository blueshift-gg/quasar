use {
    crate::{
        accounts::account::{resize, set_lamports},
        cpi::system::SYSTEM_PROGRAM_ID,
    },
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

    // SAFETY: parse verified data_len >= disc_len.
    unsafe { core::ptr::write_bytes(account.data_mut_ptr(), 0, disc_len) };

    let new_lamports = destination.lamports().wrapping_add(account.lamports());
    set_lamports(destination, new_lamports);
    account.set_lamports(0);

    unsafe { account.assign(&SYSTEM_PROGRAM_ID) };

    resize(account, 0)?;
    Ok(())
}

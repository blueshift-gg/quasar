//! Epilogue CPI helpers for closing and sweeping token accounts.
//!
//! Backs the `token_close` and `token_sweep` behaviors: close an account to
//! reclaim its rent, or transfer its full balance out before closing.

use {
    crate::instructions,
    quasar_lang::{cpi::CpiCall, prelude::*},
};

/// Close a token account via CPI to the token program.
#[inline(always)]
pub(crate) fn close_token_account(
    token_program: &AccountView,
    account: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
) -> Result<(), ProgramError> {
    instructions::close_account(token_program, account, destination, authority).invoke()
}

/// Close a token account via a CPI signed by its PDA authority.
#[inline(always)]
pub(crate) fn close_token_account_signed<S>(
    token_program: &AccountView,
    account: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
    signer: &S,
) -> Result<(), ProgramError>
where
    S: quasar_lang::cpi::CpiSignerSeeds + ?Sized,
{
    instructions::close_account(token_program, account, destination, authority)
        .invoke_signed(signer)
}

/// Transfer all tokens out, then no-op if balance is zero.
#[inline(always)]
pub(crate) fn sweep_token_account(
    token_program: &AccountView,
    source: &AccountView,
    mint: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
) -> Result<(), ProgramError> {
    let Some(call) = sweep_token_account_call(token_program, source, mint, destination, authority)?
    else {
        return Ok(());
    };
    call.invoke()
}

#[inline(always)]
fn sweep_token_account_call<'a>(
    token_program: &'a AccountView,
    source: &'a AccountView,
    mint: &'a AccountView,
    destination: &'a AccountView,
    authority: &'a AccountView,
) -> Result<Option<CpiCall<'a, 4, 10>>, ProgramError> {
    if quasar_lang::utils::hint::unlikely(
        source.data_len() < core::mem::size_of::<crate::token::TokenDataZc>(),
    ) {
        return Err(ProgramError::InvalidAccountData);
    }
    let amount = {
        // SAFETY: Length is checked above and TokenDataZc has alignment 1.
        let state = unsafe { &*(source.data_ptr() as *const crate::token::TokenDataZc) };
        state.amount()
    };

    if amount == 0 {
        return Ok(None);
    }

    if quasar_lang::utils::hint::unlikely(
        mint.data_len() < core::mem::size_of::<crate::token::MintDataZc>(),
    ) {
        return Err(ProgramError::InvalidAccountData);
    }
    let decimals = {
        // SAFETY: Length is checked above and MintDataZc has alignment 1.
        let mint_state = unsafe { &*(mint.data_ptr() as *const crate::token::MintDataZc) };
        mint_state.decimals()
    };

    Ok(Some(instructions::transfer_checked(
        token_program,
        source,
        mint,
        destination,
        authority,
        amount,
        decimals,
    )))
}

/// Sweep all tokens out via a CPI signed by the source's PDA authority.
#[inline(always)]
pub(crate) fn sweep_token_account_signed<S>(
    token_program: &AccountView,
    source: &AccountView,
    mint: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
    signer: &S,
) -> Result<(), ProgramError>
where
    S: quasar_lang::cpi::CpiSignerSeeds + ?Sized,
{
    let Some(call) = sweep_token_account_call(token_program, source, mint, destination, authority)?
    else {
        return Ok(());
    };
    call.invoke_signed(signer)
}

use {crate::instructions, quasar_lang::prelude::*};

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

/// Transfer all tokens out, then no-op if balance is zero.
#[inline(always)]
pub(crate) fn sweep_token_account(
    token_program: &AccountView,
    source: &AccountView,
    mint: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
) -> Result<(), ProgramError> {
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
        return Ok(());
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

    instructions::transfer_checked(
        token_program,
        source,
        mint,
        destination,
        authority,
        amount,
        decimals,
    )
    .invoke()
}

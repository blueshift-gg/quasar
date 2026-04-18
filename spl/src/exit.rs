use {crate::instructions, quasar_lang::prelude::*};

/// Close a token account via CPI to the token program.
#[inline(always)]
pub fn close_token_account(
    token_program: &AccountView,
    account: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
) -> Result<(), ProgramError> {
    instructions::close_account(token_program, account, destination, authority).invoke()
}

/// Transfer all tokens out, then no-op if balance is zero.
#[inline(always)]
pub fn sweep_token_account(
    token_program: &AccountView,
    source: &AccountView,
    mint: &AccountView,
    destination: &AccountView,
    authority: &AccountView,
) -> Result<(), ProgramError> {
    // SAFETY: source validated as token account during parse.
    let amount = {
        let state = unsafe { &*(source.data_ptr() as *const crate::state::TokenAccountState) };
        state.amount()
    };

    if amount == 0 {
        return Ok(());
    }

    let decimals = {
        let mint_state = unsafe { &*(mint.data_ptr() as *const crate::state::MintAccountState) };
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

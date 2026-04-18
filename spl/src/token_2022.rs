use {
    crate::{
        constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
        token::{validate_mint_inner, validate_token_inner, MintParams, TokenParams},
    },
    quasar_lang::{prelude::*, traits::Id},
};

/// Token account view — validates owner is Token-2022 program.
///
/// Also implements `Id`, so `Program<Token2022>` serves as the program account
/// type.
#[repr(transparent)]
pub struct Token2022 {
    __view: AccountView,
}
impl_program_account!(Token2022, TOKEN_2022_ID, TokenAccountState);

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Mint account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Mint2022 {
    __view: AccountView,
}
impl_program_account!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Program<Token2022> {}

// ---------------------------------------------------------------------------
// AccountCheck validation params — Token2022 / Mint2022
// ---------------------------------------------------------------------------

impl AccountCheck for Token2022 {
    type Params = TokenParams;

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if quasar_lang::utils::hint::unlikely(view.data_len() < TokenAccountState::LEN) {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }

    #[inline(always)]
    fn validate(view: &AccountView, params: &Self::Params) -> Result<(), ProgramError> {
        validate_token_inner(view, params, &TOKEN_2022_ID)
    }
}

impl AccountCheck for Mint2022 {
    type Params = MintParams;

    #[inline(always)]
    fn check(view: &AccountView) -> Result<(), ProgramError> {
        if quasar_lang::utils::hint::unlikely(view.data_len() < MintAccountState::LEN) {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }

    #[inline(always)]
    fn validate(view: &AccountView, params: &Self::Params) -> Result<(), ProgramError> {
        validate_mint_inner(view, params, &TOKEN_2022_ID)
    }
}

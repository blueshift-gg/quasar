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

// ---------------------------------------------------------------------------
// AccountInit / AccountExit — Token2022 reuses Token's param types
// ---------------------------------------------------------------------------

use crate::token::{MintInitParams, TokenInitParams};

impl quasar_lang::account_init::AccountInit for Token2022 {
    type InitParams<'a> = TokenInitParams<'a>;

    #[inline(always)]
    fn init<'a>(
        ctx: quasar_lang::account_init::InitCtx<'a>,
        params: &Self::InitParams<'a>,
    ) -> Result<(), ProgramError> {
        let mint = params.mint.ok_or(ProgramError::InvalidAccountData)?;
        let authority = params.authority.ok_or(ProgramError::InvalidAccountData)?;
        let token_program = params
            .token_program
            .ok_or(ProgramError::InvalidAccountData)?;
        crate::init::init_token_account(
            ctx.payer,
            ctx.target,
            token_program,
            mint,
            authority,
            ctx.signers,
            ctx.rent,
        )
    }
}

impl quasar_lang::account_init::AccountInit for Mint2022 {
    type InitParams<'a> = MintInitParams<'a>;

    #[inline(always)]
    fn init<'a>(
        ctx: quasar_lang::account_init::InitCtx<'a>,
        params: &Self::InitParams<'a>,
    ) -> Result<(), ProgramError> {
        let decimals = params.decimals.ok_or(ProgramError::InvalidAccountData)?;
        let authority = params.authority.ok_or(ProgramError::InvalidAccountData)?;
        let token_program = params
            .token_program
            .ok_or(ProgramError::InvalidAccountData)?;
        crate::init::init_mint_account(
            ctx.payer,
            ctx.target,
            token_program,
            decimals,
            authority,
            params.freeze_authority,
            ctx.signers,
            ctx.rent,
        )
    }
}

impl quasar_lang::account_exit::AccountExit for Token2022 {
    #[inline(always)]
    fn close(
        view: &mut AccountView,
        ctx: quasar_lang::account_exit::CloseCtx<'_>,
    ) -> Result<(), ProgramError> {
        let authority = ctx.authority.ok_or(ProgramError::InvalidAccountData)?;
        let token_program = ctx.token_program.ok_or(ProgramError::InvalidAccountData)?;
        crate::exit::close_token_account(
            token_program,
            unsafe { &*(view as *const AccountView) },
            ctx.destination,
            authority,
        )
    }

    #[inline(always)]
    fn sweep(
        view: &AccountView,
        ctx: quasar_lang::account_exit::SweepCtx<'_>,
    ) -> Result<(), ProgramError> {
        crate::exit::sweep_token_account(
            ctx.token_program,
            view,
            ctx.mint,
            ctx.receiver,
            ctx.authority,
        )
    }
}

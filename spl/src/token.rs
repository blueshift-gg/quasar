use {
    crate::{
        constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
    },
    quasar_lang::{prelude::*, traits::Id},
};

/// Token account view — validates owner is SPL Token program.
///
/// Use as `Account<Token>` for single-program token accounts,
/// or `InterfaceAccount<Token>` to accept both SPL Token and Token-2022.
///
/// Also implements `Id`, so `Program<Token>` serves as the program account
/// type.
#[repr(transparent)]
pub struct Token {
    __view: AccountView,
}
impl_program_account!(Token, SPL_TOKEN_ID, TokenAccountState);

impl Id for Token {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}

/// Mint account view — validates owner is SPL Token program.
///
/// Use as `Account<Mint>` for single-program mints,
/// or `InterfaceAccount<Mint>` to accept both SPL Token and Token-2022.
#[repr(transparent)]
pub struct Mint {
    __view: AccountView,
}
impl_program_account!(Mint, SPL_TOKEN_ID, MintAccountState);

/// Valid owner programs for token interface accounts (SPL Token + Token-2022).
static SPL_TOKEN_OWNERS: [Address; 2] = [SPL_TOKEN_ID, TOKEN_2022_ID];

impl quasar_lang::traits::Owners for Token {
    #[inline(always)]
    fn owners() -> &'static [Address] {
        &SPL_TOKEN_OWNERS
    }
}

impl quasar_lang::traits::Owners for Mint {
    #[inline(always)]
    fn owners() -> &'static [Address] {
        &SPL_TOKEN_OWNERS
    }
}

impl TokenCpi for Program<Token> {}

// ---------------------------------------------------------------------------
// Validation params for namespaced constraints
// ---------------------------------------------------------------------------

/// Validation params for token account constraints.
///
/// Filled by the derive macro from namespaced attributes (`token::mint`,
/// `token::authority`). The `token_program` field is resolved from the
/// account's owner at validation time.
#[derive(Default)]
pub struct TokenParams {
    pub mint: Option<solana_address::Address>,
    pub authority: Option<solana_address::Address>,
    pub token_program: Option<solana_address::Address>,
}

/// Validation params for mint account constraints.
///
/// Filled by the derive macro from namespaced attributes (`mint::authority`,
/// `mint::decimals`).
#[derive(Default)]
pub struct MintParams {
    pub authority: Option<solana_address::Address>,
    pub decimals: Option<u8>,
    pub freeze_authority: Option<solana_address::Address>,
    pub token_program: Option<solana_address::Address>,
}

// ---------------------------------------------------------------------------
// Shared validation helpers (used by both Token/Mint and Token2022/Mint2022)
// ---------------------------------------------------------------------------

/// Validate a token account against `TokenParams`, using `default_program` when
/// `params.token_program` is `None`.
#[inline(always)]
pub(crate) fn validate_token_inner(
    view: &AccountView,
    params: &TokenParams,
    default_program: &Address,
) -> Result<(), ProgramError> {
    let (mint, authority) = match (&params.mint, &params.authority) {
        (Some(m), Some(a)) => (m, a),
        _ => return Ok(()),
    };
    let token_program = params.token_program.as_ref().unwrap_or(default_program);
    crate::validate::validate_token_account(view, mint, authority, token_program)
}

/// Validate a mint account against `MintParams`, using `default_program` when
/// `params.token_program` is `None`.
#[inline(always)]
pub(crate) fn validate_mint_inner(
    view: &AccountView,
    params: &MintParams,
    default_program: &Address,
) -> Result<(), ProgramError> {
    let (authority, decimals) = match (&params.authority, params.decimals) {
        (Some(a), Some(d)) => (a, d),
        _ => return Ok(()),
    };
    let token_program = params.token_program.as_ref().unwrap_or(default_program);
    let freeze_authority = params.freeze_authority.as_ref();
    crate::validate::validate_mint(view, authority, decimals, freeze_authority, token_program)
}

// ---------------------------------------------------------------------------
// AccountCheck validation params — Token / Mint
// ---------------------------------------------------------------------------

impl AccountCheck for Token {
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
        validate_token_inner(view, params, &SPL_TOKEN_ID)
    }
}

impl AccountCheck for Mint {
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
        validate_mint_inner(view, params, &SPL_TOKEN_ID)
    }
}

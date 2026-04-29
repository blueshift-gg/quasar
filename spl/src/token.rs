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

// ---------------------------------------------------------------------------
// AccountInit — lifecycle trait impls
// ---------------------------------------------------------------------------

/// Init params for token account creation via CPI.
#[derive(Default)]
pub struct TokenInitParams<'a> {
    pub mint: Option<&'a AccountView>,
    pub authority: Option<&'a Address>,
    pub token_program: Option<&'a AccountView>,
}

/// Init params for mint account creation via CPI.
#[derive(Default)]
pub struct MintInitParams<'a> {
    pub decimals: Option<u8>,
    pub authority: Option<&'a Address>,
    pub freeze_authority: Option<&'a Address>,
    pub token_program: Option<&'a AccountView>,
}

impl quasar_lang::account_init::AccountInit for Token {
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

impl quasar_lang::account_init::AccountInit for Mint {
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

// ---------------------------------------------------------------------------
// AccountExit — lifecycle trait impls
// ---------------------------------------------------------------------------

impl quasar_lang::account_exit::AccountExit for Token {
    #[inline(always)]
    fn close(
        view: &mut AccountView,
        ctx: quasar_lang::account_exit::CloseCtx<'_>,
    ) -> Result<(), ProgramError> {
        // SAFETY: Token close via CPI is atomically safe — the token
        // program handles drain + zeroing in a single instruction.
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

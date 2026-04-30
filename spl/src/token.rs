use {
    crate::{
        constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
    },
    quasar_lang::{prelude::*, traits::Id},
};

/// Token account data marker — validates owner is SPL Token program.
///
/// Use as `Account<Token>` for single-program token accounts,
/// or `InterfaceAccount<Token>` to accept both SPL Token and Token-2022.
#[repr(transparent)]
pub struct Token {
    __view: AccountView,
}
impl_program_account!(Token, SPL_TOKEN_ID, TokenAccountState);

// SPL Token program marker. Use as `Program<TokenProgram>`.
quasar_lang::define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);

impl Id for TokenProgram {
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

impl TokenCpi for Program<TokenProgram> {}

// ---------------------------------------------------------------------------
// Shared trait impls via macros (AccountCheck, TokenClose, TokenSweep,
// AccountInit)
// ---------------------------------------------------------------------------

impl_token_account_traits!(Token);
impl_mint_account_check!(Mint);
impl_token_account_init!(Token);
impl_mint_account_init!(Mint);

// ---------------------------------------------------------------------------
// Init param types (shared by Token2022/Mint2022)
// ---------------------------------------------------------------------------

/// Init kind: direct token account or ATA.
pub enum TokenInitKind<'a> {
    /// Direct token account init via system program + initialize_account3.
    Token {
        mint: &'a AccountView,
        authority: &'a Address,
        token_program: &'a AccountView,
    },
    /// ATA init via the associated token program.
    AssociatedToken {
        mint: &'a AccountView,
        authority: &'a AccountView,
        token_program: &'a AccountView,
        system_program: &'a AccountView,
        ata_program: &'a AccountView,
        idempotent: bool,
    },
}

/// Init params for token account creation via CPI.
#[derive(Default)]
pub struct TokenInitParams<'a> {
    pub kind: Option<TokenInitKind<'a>>,
}

/// Init params for mint account creation via CPI.
#[derive(Default)]
pub struct MintInitParams<'a> {
    pub decimals: Option<u8>,
    pub authority: Option<&'a Address>,
    pub freeze_authority: Option<&'a Address>,
    pub token_program: Option<&'a AccountView>,
}

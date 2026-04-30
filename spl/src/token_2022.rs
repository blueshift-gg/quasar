use {
    crate::{
        constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
    },
    quasar_lang::{prelude::*, traits::Id},
};

/// Token-2022 account data marker.
#[repr(transparent)]
pub struct Token2022 {
    __view: AccountView,
}
impl_program_account!(Token2022, TOKEN_2022_ID, TokenAccountState);

// Token-2022 program marker. Use as `Program<Token2022Program>`.
quasar_lang::define_account!(pub struct Token2022Program => [checks::Executable, checks::Address]);

impl Id for Token2022Program {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Mint account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Mint2022 {
    __view: AccountView,
}
impl_program_account!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Program<Token2022Program> {}

// ---------------------------------------------------------------------------
// Shared trait impls via macros (AccountCheck, TokenClose, TokenSweep,
// AccountInit)
// ---------------------------------------------------------------------------

impl_token_account_traits!(Token2022);
impl_mint_account_check!(Mint2022);
impl_token_account_init!(Token2022);
impl_mint_account_init!(Mint2022);

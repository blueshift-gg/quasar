use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

quasar_core::define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);

impl Id for TokenProgram {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}

/// Token account view — validates owner is SPL Token program.
///
/// Use as `Account<Token>` for single-program token accounts,
/// or `InterfaceAccount<Token>` to accept both SPL Token and Token-2022.
#[repr(transparent)]
pub struct Token {
    __view: AccountView,
}
impl_single_owner!(Token, SPL_TOKEN_ID, TokenAccountState);

/// Mint account view — validates owner is SPL Token program.
///
/// Use as `Account<Mint>` for single-program mints,
/// or `InterfaceAccount<Mint>` to accept both SPL Token and Token-2022.
#[repr(transparent)]
pub struct Mint {
    __view: AccountView,
}
impl_single_owner!(Mint, SPL_TOKEN_ID, MintAccountState);

impl TokenCpi for TokenProgram {}
impl TokenCpi for Program<TokenProgram> {}

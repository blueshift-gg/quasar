use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

quasar_core::define_account!(pub struct Token => [checks::Executable, checks::Address]);

impl Id for Token {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}

/// Token account owned by the SPL Token program.
pub struct TokenAccount;
impl_single_owner!(TokenAccount, SPL_TOKEN_ID, TokenAccountState);

/// Mint account owned by the SPL Token program.
pub struct Mint;
impl_single_owner!(Mint, SPL_TOKEN_ID, MintAccountState);

impl TokenCpi for Token {}
impl TokenCpi for Program<Token> {}

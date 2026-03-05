use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{TOKEN_2022_BYTES, TOKEN_2022_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

quasar_core::define_account!(pub struct Token2022 => [checks::Executable, checks::Address]);

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Token account owned by the Token-2022 program.
pub struct Token2022Account;
impl_single_owner!(Token2022Account, TOKEN_2022_ID, TokenAccountState);

/// Mint account owned by the Token-2022 program.
pub struct Mint2022Account;
impl_single_owner!(Mint2022Account, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Token2022 {}
impl TokenCpi for Program<Token2022> {}

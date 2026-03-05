use quasar_core::prelude::*;

use crate::constants::{TOKEN_2022_BYTES, TOKEN_2022_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

quasar_core::define_account!(pub struct Token2022Program => [checks::Executable, checks::Address]);

impl Program for Token2022Program {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Token account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Token2022 {
    __view: AccountView,
}
impl_single_owner!(Token2022, TOKEN_2022_ID, TokenAccountState);

/// Mint account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Mint2022 {
    __view: AccountView,
}
impl_single_owner!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Token2022Program {}

use {
    crate::{
        helpers::constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
    },
    quasar_core::{prelude::*, traits::Id},
};

/// Token account view — validates owner is Token-2022 program.
///
/// Also implements `Id`, so `Program<Token2022>` serves as the program account
/// type.
#[repr(transparent)]
pub struct Token2022 {
    __view: AccountView,
}
impl_single_owner!(Token2022, TOKEN_2022_ID, TokenAccountState);

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Mint account view — validates owner is Token-2022 program.
#[repr(transparent)]
pub struct Mint2022 {
    __view: AccountView,
}
impl_single_owner!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for Program<Token2022> {}

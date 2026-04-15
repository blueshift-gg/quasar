use {
    crate::{
        constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        state::{MintAccountState, TokenAccountState},
    },
    quasar_lang::{prelude::*, traits::Id},
};

define_spl_account!(
    /// Token account view — validates owner is Token-2022 program.
    ///
    /// Also implements `Id`, so `Program<Token2022>` serves as the program account
    /// type.
    Token2022, TOKEN_2022_ID, TokenAccountState
);

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

define_spl_account!(
    /// Mint account view — validates owner is Token-2022 program.
    Mint2022, TOKEN_2022_ID, MintAccountState
);

impl TokenCpi for Program<Token2022> {}

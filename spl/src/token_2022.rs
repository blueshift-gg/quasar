//! Token-2022 account, mint, and program wrapper types.
//!
//! Mirrors [`crate::token`] against the Token-2022 program: `Token2022`,
//! `Mint2022`, and `Token2022Program` reuse the SPL `TokenData`/`MintData`
//! schemas but trust only accounts owned by the Token-2022 program.

use {
    crate::{
        constants::{TOKEN_2022_BYTES, TOKEN_2022_ID},
        instructions::TokenCpi,
        token::{MintData, TokenData},
    },
    quasar_lang::{prelude::*, traits::Id},
};

quasar_lang::define_account!(
    /// Token-2022 account data; validates owner is Token-2022 program.
    pub struct Token2022 => [checks::ZeroPod]: TokenData
);

impl Owner for Token2022 {
    const OWNER: Address = TOKEN_2022_ID;
}

quasar_lang::define_account!(
    /// Token-2022 executable account marker.
    ///
    /// Use as `Program<Token2022Program>`.
    pub struct Token2022Program => [checks::Executable, checks::Address]
);

impl Id for Token2022Program {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

quasar_lang::define_account!(
    /// Mint-2022 account data; validates owner is Token-2022 program.
    pub struct Mint2022 => [checks::ZeroPod]: MintData
);

impl Owner for Mint2022 {
    const OWNER: Address = TOKEN_2022_ID;
}

impl TokenCpi for Program<Token2022Program> {}

impl_token_account_init!(Token2022);
impl_mint_account_init!(Mint2022);

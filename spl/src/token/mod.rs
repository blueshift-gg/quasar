use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

/// Marker type for the SPL Token program.
///
/// Use with the `Program<T>` wrapper:
/// ```ignore
/// pub token_program: &'info Program<Token>,
/// ```
pub struct Token;

impl Id for Token {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}

/// Token account marker — validates owner is SPL Token program.
///
/// Use as `Account<Token>` for single-program token accounts,
/// or `InterfaceAccount<Token>` to accept both SPL Token and Token-2022.
pub struct Token;
impl_single_owner!(Token, SPL_TOKEN_ID, TokenAccountState);

/// Mint account marker — validates owner is SPL Token program.
///
/// Use as `Account<Mint>` for single-program mints,
/// or `InterfaceAccount<Mint>` to accept both SPL Token and Token-2022.
pub struct Mint;
impl_single_owner!(Mint, SPL_TOKEN_ID, MintAccountState);

impl TokenCpi for quasar_core::accounts::Program<Token> {}

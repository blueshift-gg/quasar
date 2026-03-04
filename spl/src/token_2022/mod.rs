use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{TOKEN_2022_BYTES, TOKEN_2022_ID};
use crate::cpi::TokenCpi;
use crate::state::{MintAccountState, TokenAccountState};

/// Marker type for the Token-2022 program.
///
/// Use with the `Program<T>` wrapper:
/// ```ignore
/// pub token_program: &'info Program<Token2022>,
/// ```
pub struct Token2022;

impl Id for Token2022 {
    const ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
}

/// Token account marker — validates owner is Token-2022 program.
pub struct Token2022;
impl_single_owner!(Token2022, TOKEN_2022_ID, TokenAccountState);

/// Mint account marker — validates owner is Token-2022 program.
pub struct Mint2022;
impl_single_owner!(Mint2022, TOKEN_2022_ID, MintAccountState);

impl TokenCpi for quasar_core::accounts::Program<Token2022> {}

//! Token program interface marker.
//!
//! `TokenInterface` matches either the SPL Token or Token-2022 program, letting
//! `Interface<TokenInterface>` accept both and issue their shared CPI methods.

use {
    crate::{
        constants::{SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
    },
    quasar_lang::prelude::*,
};

/// Returns `true` if the address is SPL Token or Token-2022.
#[inline(always)]
fn is_token_program(address: &Address) -> bool {
    quasar_lang::keys_eq(address, &SPL_TOKEN_ID) || quasar_lang::keys_eq(address, &TOKEN_2022_ID)
}

/// Marker type for the token program interface (SPL Token or Token-2022).
///
/// Use with the `Interface<T>` wrapper:
/// ```text
/// pub token_program: &'info Interface<TokenInterface>,
/// ```
pub struct TokenInterface;

impl ProgramInterface for TokenInterface {
    #[inline(always)]
    fn matches(address: &Address) -> bool {
        is_token_program(address)
    }
}

impl TokenCpi for quasar_lang::accounts::Interface<TokenInterface> {}

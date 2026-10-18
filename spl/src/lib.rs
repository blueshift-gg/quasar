//! SPL Token accounts, mints and calls, for programs built with Quasar,
//! spelled as in Anchor: `Account<TokenAccount>`, `Account<Mint>`,
//! `Program<Token>`, and `token(mint = .., authority = ..)`.
#![no_std]

#[cfg(test)]
extern crate std;

pub mod token;

pub mod prelude {
    pub use crate::token::{self, Mint, Token, Token2022, TokenAccount, TokenInterface, mint};
}

use quasar_core::cpi::CpiCall;
use quasar_core::prelude::*;

use crate::associated_token::AssociatedToken;
use crate::cpi::TokenCpi;
use crate::interface::InterfaceAccount;
use crate::token::{Mint, TokenAccount};
use crate::token_2022::{Mint2022Account, Token2022Account};

/// Extension trait providing `.close()` on `Account<T>` for token/mint account types.
///
/// Returns a deferred `CpiCall` — caller controls `.invoke()` vs `.invoke_signed()`.
///
/// ```ignore
/// self.vault.close(&self.token_program, &self.maker, &self.escrow)
///     .invoke_signed(&seeds)?;
/// ```
pub trait TokenClose: AsAccountView + Sized {
    #[inline(always)]
    fn close<'a>(
        &'a self,
        token_program: &'a impl TokenCpi,
        destination: &'a impl AsAccountView,
        authority: &'a impl AsAccountView,
    ) -> CpiCall<'a, 3, 1> {
        token_program.close_account(self, destination, authority)
    }
}

macro_rules! impl_token_close {
    ($($ty:ty),*) => {
        $(
            impl TokenClose for Account<$ty> {}
            impl TokenClose for InterfaceAccount<$ty> {}
        )*
    };
}

impl_token_close!(
    TokenAccount,
    Token2022Account,
    AssociatedToken,
    Mint,
    Mint2022Account
);

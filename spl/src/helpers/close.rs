use {
    crate::{
        associated_token::AssociatedToken,
        instructions::TokenCpi,
        interface::InterfaceAccount,
        token::{Mint, Token},
        token_2022::{Mint2022, Token2022},
    },
    quasar_core::{cpi::CpiCall, prelude::*},
};

/// Extension trait providing `.close()` on `Account<T>` for token/mint account
/// types.
///
/// Returns a deferred `CpiCall` — caller controls `.invoke()` vs
/// `.invoke_signed()`.
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

impl_token_close!(Token, Token2022, AssociatedToken, Mint, Mint2022);

//! Token account validation op (Phase 3).
//!
//! Validates that a token account has the expected mint, authority, and
//! token program. Contributes init params via `apply_init_params`.

use quasar_lang::{
    ops::{AccountOp, OpCtx},
    prelude::*,
};

/// Marker trait for account types with token account layout.
///
/// Bounds `token::Op` to only accept types that actually have token data.
pub trait HasTokenLayout {}

impl HasTokenLayout for quasar_lang::accounts::account::Account<crate::token::Token> {}
impl HasTokenLayout for quasar_lang::accounts::account::Account<crate::token_2022::Token2022> {}
impl HasTokenLayout
    for quasar_lang::accounts::interface_account::InterfaceAccount<crate::token::Token>
{
}

/// Token validation op. Constructed by the derive from `token(...)` syntax.
pub struct Op<'a> {
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a, F: AsAccountView + HasTokenLayout> AccountOp<F> for Op<'a> {
    #[inline(always)]
    fn after_load(&self, field: &F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        crate::validate::validate_token_account(
            field.to_account_view(),
            self.mint.address(),
            self.authority.address(),
            self.token_program.address(),
        )
    }

    #[inline(always)]
    fn apply_init_params(&self, params: *mut u8) -> Result<(), ProgramError> {
        // SAFETY: For all F: HasTokenLayout, BehaviorTarget: AccountInit
        // with InitParams = TokenInitParams. The derive passes a properly-typed
        // &mut TokenInitParams cast to *mut u8.
        let params: &mut crate::token::TokenInitParams<'_> =
            unsafe { &mut *(params as *mut crate::token::TokenInitParams<'_>) };
        if params.kind.is_some() {
            return Err(ProgramError::InvalidArgument);
        }
        params.kind = Some(crate::token::TokenInitKind::Token {
            mint: self.mint,
            authority: self.authority.address(),
            token_program: self.token_program,
        });
        Ok(())
    }
}

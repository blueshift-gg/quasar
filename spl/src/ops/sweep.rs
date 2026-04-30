//! Token sweep op (Phase 4 epilogue).
//!
//! Sweeps all tokens from an account to a receiver before closing.

use {
    crate::ops::token::HasTokenLayout,
    quasar_lang::{
        account_load::AccountLoad,
        ops::{AccountOp, OpCtx},
        prelude::*,
    },
};

/// Trait for token account types that support sweep (transfer all tokens out).
pub trait TokenSweep {
    fn sweep(
        view: &AccountView,
        receiver: &AccountView,
        mint: &AccountView,
        authority: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError>;
}

/// Token sweep op. Constructed by the derive from `exit(sweep(...))` syntax.
pub struct Op<'a> {
    pub receiver: &'a AccountView,
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a, F: AsAccountView + HasTokenLayout + AccountLoad> AccountOp<F> for Op<'a>
where
    <F as AccountLoad>::BehaviorTarget: TokenSweep,
{
    const REQUIRES_MUT: bool = true;
    const HAS_EXIT: bool = true;

    #[inline(always)]
    fn exit(&self, field: &mut F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        type Target<F2> = <F2 as AccountLoad>::BehaviorTarget;
        <Target<F> as TokenSweep>::sweep(
            field.to_account_view(),
            self.receiver,
            self.mint,
            self.authority,
            self.token_program,
        )
    }
}

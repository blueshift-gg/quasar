//! Token close op (Phase 4 epilogue).
//!
//! Closes a token account via CPI to the token program.

use {
    crate::ops::token::HasTokenLayout,
    quasar_lang::{
        account_load::AccountLoad,
        ops::{AccountOp, OpCtx},
        prelude::*,
    },
};

/// Trait for token account types that can be closed via CPI.
///
/// Implemented on the behavior target (`Token`, `Token2022`). The close
/// is performed by CPI to the token program, which atomically drains
/// lamports and invalidates the account.
pub trait TokenClose {
    fn close(
        view: &mut AccountView,
        dest: &AccountView,
        authority: &AccountView,
        token_program: &AccountView,
    ) -> Result<(), ProgramError>;
}

/// Token close op. Constructed by the derive from `exit(close(...))` syntax
/// on token account fields.
pub struct Op<'a> {
    pub dest: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a, F: AccountLoad + HasTokenLayout> AccountOp<F> for Op<'a>
where
    <F as AccountLoad>::BehaviorTarget: TokenClose,
{
    const REQUIRES_MUT: bool = true;
    const HAS_EXIT: bool = true;

    #[inline(always)]
    fn exit(&self, field: &mut F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        type Target<F2> = <F2 as AccountLoad>::BehaviorTarget;
        let view = unsafe { <F as AccountLoad>::to_account_view_mut(field) };
        <Target<F> as TokenClose>::close(view, self.dest, self.authority, self.token_program)
    }
}

//! Realloc op (Phase 3b — after_load_mut).
//!
//! Resizes an account's data region after load has validated owner and
//! discriminator. Runs in Phase 3b because it needs `&mut Field`.
//! Rejects shrinking below the account type's minimum Space.

use quasar_lang::{
    account_load::AccountLoad,
    ops::{AccountOp, OpCtx, SupportsRealloc},
    prelude::*,
};

/// Realloc op. Constructed by the derive from `realloc(...)` syntax.
pub struct Op<'a> {
    pub space: usize,
    pub payer: &'a AccountView,
}

impl<'a, F: AccountLoad> AccountOp<F> for Op<'a>
where
    <F as AccountLoad>::BehaviorTarget: SupportsRealloc + quasar_lang::traits::Space,
{
    const REQUIRES_MUT: bool = true;

    #[inline(always)]
    fn after_load_mut(&self, field: &mut F, ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        let min_space = <<F as AccountLoad>::BehaviorTarget as quasar_lang::traits::Space>::SPACE;
        if self.space < min_space {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let view = unsafe { <F as AccountLoad>::to_account_view_mut(field) };
        quasar_lang::accounts::realloc_account(view, self.space, self.payer, Some(ctx.rent()?))
    }
}

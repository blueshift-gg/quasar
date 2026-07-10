//! Realloc op emitted from account directives.
//!
//! The account has already passed normal load validation; this op only enforces
//! the account type's minimum `Space` before resizing the backing data.

use {
    super::{OpCtx, RentAccess, SupportsRealloc},
    crate::account_load::AccountLoad,
    solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Runtime form of a `realloc(...)` field directive.
pub struct Op<'a> {
    /// Requested account-data length in bytes.
    pub space: usize,
    /// Account funding any additional rent-exempt balance.
    pub payer: &'a AccountView,
}

impl<'a> Op<'a> {
    /// Resize a loaded account field.
    #[inline(always)]
    pub fn apply<F, R>(&self, field: &mut F, ctx: &OpCtx<'_, R>) -> Result<(), ProgramError>
    where
        F: AccountLoad + SupportsRealloc + crate::traits::Space,
        R: RentAccess,
    {
        let min_space = <F as crate::traits::Space>::SPACE;
        if self.space < min_space {
            return Err(ProgramError::AccountDataTooSmall);
        }
        // SAFETY: `field` is the loaded account wrapper selected by the derive;
        // realloc must operate on that wrapper's backing `AccountView`.
        let view = unsafe { <F as AccountLoad>::to_account_view_mut(field) };
        crate::accounts::realloc_account(view, self.space, self.payer, Some(ctx.rent.get()?))
    }
}

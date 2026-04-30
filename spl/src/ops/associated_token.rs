//! Associated Token Account op — validate-only.
//!
//! Validates the ATA address matches the expected authority + mint + program.
//! For ATA initialization, use `ata_init::Op` instead.

use {
    crate::ops::token::HasTokenLayout,
    quasar_lang::{
        ops::{AccountOp, OpCtx},
        prelude::*,
    },
};

/// ATA validate-only op. Constructed by the derive from
/// `associated_token(...)`.
pub struct Op<'a> {
    pub authority: &'a AccountView,
    pub mint: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a, F: AsAccountView + HasTokenLayout> AccountOp<F> for Op<'a> {
    #[inline(always)]
    fn after_load(&self, field: &F, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        crate::validate::validate_ata(
            field.to_account_view(),
            self.authority.address(),
            self.mint.address(),
            self.token_program.address(),
        )
    }
}

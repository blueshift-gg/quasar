//! ATA init op — provides init params for associated token account creation.
//!
//! All required accounts are non-optional fields — compile error if omitted.
//! The derive treats this as an ordinary op group.

use {
    crate::ops::token::HasTokenLayout,
    quasar_lang::{
        ops::{AccountOp, OpCtx},
        prelude::*,
    },
};

/// ATA init op. Constructed by the derive from `ata_init(...)` syntax.
///
/// All fields are non-optional — omitting any is a compile error.
pub struct Op<'a> {
    pub authority: &'a AccountView,
    pub mint: &'a AccountView,
    pub payer: &'a AccountView,
    pub token_program: &'a AccountView,
    pub system_program: &'a AccountView,
    pub ata_program: &'a AccountView,
    pub idempotent: bool,
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

    #[inline(always)]
    fn apply_init_params(&self, params: *mut u8) -> Result<(), ProgramError> {
        // SAFETY: Same as token::Op — F: HasTokenLayout guarantees
        // InitParams = TokenInitParams.
        let params: &mut crate::token::TokenInitParams<'_> =
            unsafe { &mut *(params as *mut crate::token::TokenInitParams<'_>) };
        if params.kind.is_some() {
            return Err(ProgramError::InvalidArgument);
        }
        params.kind = Some(crate::token::TokenInitKind::AssociatedToken {
            mint: self.mint,
            authority: self.authority,
            token_program: self.token_program,
            system_program: self.system_program,
            ata_program: self.ata_program,
            idempotent: self.idempotent,
        });
        Ok(())
    }
}

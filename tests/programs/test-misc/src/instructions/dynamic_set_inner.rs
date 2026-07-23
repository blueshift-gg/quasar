use {
    crate::state::{DynamicAccount, DynamicAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

/// No `Sysvar<Rent>` field: proves `set_inner` resolves rent through the
/// syscall path when the rewrite resizes the account.
#[derive(Accounts)]
pub struct DynamicSetInner {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl DynamicSetInner {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str, new_tags: &[Address]) -> Result<(), ProgramError> {
        self.account.set_inner(
            DynamicAccountInner {
                name: new_name,
                tags: new_tags,
            },
            self.payer.to_account_view(),
        )
    }
}

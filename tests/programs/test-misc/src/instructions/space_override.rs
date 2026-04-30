use {
    crate::state::{SpaceTestAccount, SpaceTestAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SpaceOverride {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = SpaceTestAccount::seeds(payer.address()))]
    pub account: Account<SpaceTestAccount>,
    pub system_program: Program<SystemProgram>,
}

impl SpaceOverride {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &SpaceOverrideBumps) -> Result<(), ProgramError> {
        self.account.set_inner(SpaceTestAccountInner {
            authority: *self.payer.address(),
            value,
            bump: bumps.account,
        });
        Ok(())
    }
}

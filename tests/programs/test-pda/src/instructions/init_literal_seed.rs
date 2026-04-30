use {
    crate::state::{ConfigAccount, ConfigAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitLiteralSeed {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = ConfigAccount::seeds())]
    pub config: Account<ConfigAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitLiteralSeed {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitLiteralSeedBumps) -> Result<(), ProgramError> {
        self.config
            .set_inner(ConfigAccountInner { bump: bumps.config });
        Ok(())
    }
}

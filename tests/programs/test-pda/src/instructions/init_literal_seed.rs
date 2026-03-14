use {crate::state::ConfigAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct InitLiteralSeed<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"config"], bump)]
    pub config: &'info mut Account<ConfigAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitLiteralSeed<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitLiteralSeedBumps) -> Result<(), ProgramError> {
        self.config.set_inner(bumps.config);
        Ok(())
    }
}

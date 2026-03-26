use {crate::state::MaxSeedAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct InitMaxSeedLength<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"abcdefghijklmnopqrstuvwxyz012345"], bump)]
    pub max_seed: &'info mut Account<MaxSeedAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitMaxSeedLength<'info> {
    #[inline(always)]
    pub fn handler(&mut self, bumps: &InitMaxSeedLengthBumps) -> Result<(), ProgramError> {
        self.max_seed.set_inner(bumps.max_seed);
        Ok(())
    }
}

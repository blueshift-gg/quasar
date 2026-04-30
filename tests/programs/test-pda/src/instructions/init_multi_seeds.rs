use {
    crate::state::{ComplexAccount, ComplexAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitMultiSeeds {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(mut, init, payer = payer, address = ComplexAccount::seeds(payer.address(), authority.address()))]
    pub complex: Account<ComplexAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitMultiSeeds {
    #[inline(always)]
    pub fn handler(
        &mut self,
        amount: u64,
        bumps: &InitMultiSeedsBumps,
    ) -> Result<(), ProgramError> {
        self.complex.set_inner(ComplexAccountInner {
            authority: *self.authority.address(),
            amount,
            bump: bumps.complex,
        });
        Ok(())
    }
}

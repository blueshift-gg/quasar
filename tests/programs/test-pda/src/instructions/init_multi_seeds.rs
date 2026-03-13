use {crate::state::ComplexAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct InitMultiSeeds<'info> {
    pub payer: &'info mut Signer,
    pub authority: &'info Signer,
    #[account(init, payer = payer, seeds = [b"complex", payer, authority], bump)]
    pub complex: &'info mut Account<ComplexAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitMultiSeeds<'info> {
    #[inline(always)]
    pub fn handler(
        &mut self,
        amount: u64,
        bumps: &InitMultiSeedsBumps,
    ) -> Result<(), ProgramError> {
        self.complex
            .set_inner(*self.authority.address(), amount, bumps.complex);
        Ok(())
    }
}

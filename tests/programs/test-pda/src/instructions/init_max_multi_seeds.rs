use {crate::state::ComplexAccount, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct InitMaxMultiSeeds<'info> {
    pub payer: &'info mut Signer,
    pub authority: &'info Signer,
    #[account(
        init,
        payer = payer,
        // Max 15 seeds allowed + 1 bump seed , in total 16
        seeds = [
            b"complex",
            b"complex",
            b"complex", 
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
            b"complex",
        ],
        bump)]
    pub complex: &'info mut Account<ComplexAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitMaxMultiSeeds<'info> {
    #[inline(always)]
    pub fn handler(
        &mut self,
        amount: u64,
        bumps: &InitMaxMultiSeedsBumps,
    ) -> Result<(), ProgramError> {
        self.complex
            .set_inner(*self.authority.address(), amount, bumps.complex);
        Ok(())
    }
}

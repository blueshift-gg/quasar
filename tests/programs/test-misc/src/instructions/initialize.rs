use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct InitializeSimple<'info> {
    pub payer: &'info mut Signer,
    #[account(init, payer = payer, seeds = [b"simple", payer], bump)]
    pub account: &'info mut Account<SimpleAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> InitializeSimple<'info> {
    #[inline(always)]
    pub fn handler(
        &mut self,
        value: u64,
        bumps: &InitializeSimpleBumps,
    ) -> Result<(), ProgramError> {
        self.account
            .set_inner(*self.payer.address(), value, bumps.account);
        Ok(())
    }
}

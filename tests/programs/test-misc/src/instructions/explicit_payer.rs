use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct ExplicitPayer<'info> {
    pub funder: &'info mut Signer,
    #[account(init, payer = funder, seeds = [b"explicit", funder], bump)]
    pub account: &'info mut Account<SimpleAccount>,
    pub system_program: &'info Program<System>,
}

impl<'info> ExplicitPayer<'info> {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &ExplicitPayerBumps) -> Result<(), ProgramError> {
        self.account
            .set_inner(*self.funder.address(), value, bumps.account);
        Ok(())
    }
}

use {
    crate::state::{SimpleAccount, SimpleAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitializeSimple {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = SimpleAccount::seeds(payer.address()))]
    pub account: Account<SimpleAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitializeSimple {
    #[inline(always)]
    pub fn handler(
        &mut self,
        value: u64,
        bumps: &InitializeSimpleBumps,
    ) -> Result<(), ProgramError> {
        self.account.set_inner(SimpleAccountInner {
            authority: *self.payer.address(),
            value,
            bump: bumps.account,
        });
        Ok(())
    }
}

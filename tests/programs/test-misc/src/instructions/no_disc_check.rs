use {
    crate::state::{NoDiscAccount, NoDiscAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitNoDisc {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = NoDiscAccount::seeds(payer.address()))]
    pub account: Account<NoDiscAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitNoDisc {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, _bumps: &InitNoDiscBumps) -> Result<(), ProgramError> {
        self.account.set_inner(NoDiscAccountInner {
            authority: *self.payer.address(),
            value,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ReadNoDisc {
    #[account(mut)]
    pub account: Account<NoDiscAccount>,
}

impl ReadNoDisc {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        // Just access the fields to verify Deref works.
        let _authority = self.account.authority;
        let _value = self.account.value;
        Ok(())
    }
}

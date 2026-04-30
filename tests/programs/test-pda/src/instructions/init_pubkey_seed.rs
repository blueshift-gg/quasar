use {
    crate::state::{UserAccount, UserAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitPubkeySeed {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, payer = payer, address = UserAccount::seeds(payer.address()))]
    pub user: Account<UserAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitPubkeySeed {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &InitPubkeySeedBumps) -> Result<(), ProgramError> {
        self.user.set_inner(UserAccountInner {
            authority: *self.payer.address(),
            value,
            bump: bumps.user,
        });
        Ok(())
    }
}

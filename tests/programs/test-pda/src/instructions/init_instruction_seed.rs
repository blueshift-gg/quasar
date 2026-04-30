use {
    crate::state::{ItemAccount, ItemAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct InitInstructionSeed {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(mut, init, payer = payer, address = ItemAccount::seeds(authority.address()))]
    pub item: Account<ItemAccount>,
    pub system_program: Program<SystemProgram>,
}

impl InitInstructionSeed {
    #[inline(always)]
    pub fn handler(
        &mut self,
        id: u64,
        bumps: &InitInstructionSeedBumps,
    ) -> Result<(), ProgramError> {
        self.item.set_inner(ItemAccountInner {
            id,
            bump: bumps.item,
        });
        Ok(())
    }
}

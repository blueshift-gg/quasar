use quasar_core::prelude::*;

use crate::state::ItemAccount;

#[derive(Accounts)]
pub struct InitInstructionSeed<'info> {
    pub payer: &'info mut Signer,
    pub authority: &'info Signer,
    #[account(init, payer = payer, seeds = [b"item", authority], bump)]
    pub item: &'info mut Account<ItemAccount>,
    pub system_program: &'info SystemProgram,
}

impl<'info> InitInstructionSeed<'info> {
    #[inline(always)]
    pub fn handler(
        &mut self,
        id: u64,
        bumps: &InitInstructionSeedBumps,
    ) -> Result<(), ProgramError> {
        self.item.set_inner(id, bumps.item);
        Ok(())
    }
}

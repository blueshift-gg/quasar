use {crate::state::SimpleAccount, quasar_core::prelude::*};

#[derive(Accounts)]
#[instruction(new_space: u64)]
pub struct ReallocCheck<'info> {
    #[account(mut, realloc = new_space as usize)]
    pub account: &'info mut Account<SimpleAccount>,
    #[account(mut)]
    pub payer: &'info mut Signer,
    pub system_program: &'info Program<System>,
}

impl<'info> ReallocCheck<'info> {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

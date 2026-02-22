use quasar_core::prelude::*;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct SetLabel<'info> {
    pub creator: &'info mut Signer,
    #[account(
        mut,
        has_one = creator,
        seeds = [b"multisig", creator],
        bump = config.bump
    )]
    pub config: &'info mut Account<MultisigConfig<'info>>,
    pub system_program: &'info SystemProgram,
}

impl<'info> SetLabel<'info> {
    #[inline(always)]
    pub fn update_label(&self, label_len: u8, label_bytes: &[u8; 32]) -> Result<(), ProgramError> {
        let label = core::str::from_utf8(&label_bytes[..label_len as usize])
            .map_err(|_| ProgramError::InvalidArgument)?;

        self.config.set_label(self.creator, label)
    }
}

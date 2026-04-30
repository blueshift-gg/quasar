use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct ReallocCheck {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl ReallocCheck {
    #[inline(always)]
    pub fn handler(&mut self, new_space: u64) -> Result<(), ProgramError> {
        self.account
            .realloc(new_space as usize, self.payer.to_account_view(), None)
    }
}

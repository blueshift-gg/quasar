use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct CloseAccountAlias {
    #[account(mut, close(dest = destination))]
    pub account: Account<SimpleAccount>,
    /// Duplicate account used to exercise a self-close through the generated
    /// close epilogue.
    #[account(mut, dup)]
    pub destination: UncheckedAccount,
}

impl CloseAccountAlias {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

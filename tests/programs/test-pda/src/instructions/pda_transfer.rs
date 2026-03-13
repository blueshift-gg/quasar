use {crate::state::UserAccount, quasar_core::prelude::*};

#[derive(Accounts)]
pub struct PdaTransfer<'info> {
    pub authority: &'info Signer,
    #[account(mut, has_one = authority, seeds = [b"user", authority], bump = pda.bump)]
    pub pda: &'info mut Account<UserAccount>,
    pub recipient: &'info mut SystemAccount,
}

impl<'info> PdaTransfer<'info> {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        let pda_view = self.pda.to_account_view();
        let recipient_view = self.recipient.to_account_view();
        let pda_lamports = pda_view.lamports();
        if pda_lamports < amount {
            return Err(ProgramError::InsufficientFunds);
        }
        set_lamports(pda_view, pda_lamports - amount);
        set_lamports(recipient_view, recipient_view.lamports() + amount);
        Ok(())
    }
}

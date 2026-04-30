use {crate::state::UserAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct PdaTransfer {
    pub authority: Signer,
    #[account(mut, has_one(authority), address = UserAccount::seeds(authority.address()))]
    pub pda: Account<UserAccount>,
    #[account(mut)]
    pub recipient: SystemAccount,
}

impl PdaTransfer {
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

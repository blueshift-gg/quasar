use {crate::state::UserAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct UpdatePda {
    pub authority: Signer,
    #[account(mut, has_one(authority), address = UserAccount::seeds(authority.address()))]
    pub user: Account<UserAccount>,
}

impl UpdatePda {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        self.user.value = new_value.into();
        Ok(())
    }
}

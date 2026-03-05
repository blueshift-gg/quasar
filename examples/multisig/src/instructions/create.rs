use quasar_core::prelude::*;
use quasar_core::remaining::RemainingAccounts;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct Create<'info> {
    pub creator: &'info mut Signer,
    #[account(init, mut, payer = creator, seeds = [b"multisig", creator], bump)]
    pub config: Account<MultisigConfig<'info>>,
    pub rent: &'info Sysvar<Rent>,
    pub system_program: &'info SystemProgram,
}

impl<'info> Create<'info> {
    #[inline(always)]
    pub fn create_multisig(
        &mut self,
        threshold: u8,
        bumps: &CreateBumps,
        remaining: RemainingAccounts,
    ) -> Result<(), ProgramError> {
        let mut addrs = [Address::default(); 10];
        let mut count = 0usize;

        for account in remaining.iter() {
            let account = account?;
            if count >= 10 {
                return Err(ProgramError::InvalidArgument);
            }
            if !account.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
            addrs[count] = *account.address();
            count += 1;
        }

        if threshold == 0 || threshold as usize > count {
            return Err(ProgramError::InvalidArgument);
        }

        self.config.set_inner(
            *self.creator.address(),
            threshold,
            bumps.config,
            "",
            &addrs[..count],
            self.creator.to_account_view(),
            Some(&**self.rent),
        )
    }
}

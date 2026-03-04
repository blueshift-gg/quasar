use quasar_core::prelude::*;
use quasar_core::remaining::RemainingAccounts;

use crate::state::MultisigConfig;

#[derive(Accounts)]
pub struct ExecuteTransfer<'info> {
    #[account(
        has_one = creator,
        seeds = [b"multisig", creator],
        bump = config.bump
    )]
    pub config: &'info Account<MultisigConfig<'info>>,
    pub creator: &'info UncheckedAccount,
    #[account(mut, seeds = [b"vault", config], bump)]
    pub vault: &'info mut UncheckedAccount,
    pub recipient: &'info mut UncheckedAccount,
    pub system_program: &'info Program<System>,
}

impl<'info> ExecuteTransfer<'info> {
    #[inline(always)]
    pub fn verify_and_transfer(
        &self,
        amount: u64,
        bumps: &ExecuteTransferBumps,
        remaining: RemainingAccounts,
    ) -> Result<(), ProgramError> {
        let stored_signers = self.config.signers();
        let threshold = self.config.threshold;

        let mut approvals = 0u8;
        for account in remaining.iter() {
            let account = account?;
            if !account.is_signer() {
                continue;
            }
            let addr = account.address();
            for stored in stored_signers {
                if addr == stored {
                    approvals += 1;
                    break;
                }
            }
        }

        if approvals < threshold {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let seeds = bumps.vault_seeds();
        self.system_program
            .transfer(self.vault, self.recipient, amount)
            .invoke_signed(&seeds)
    }
}

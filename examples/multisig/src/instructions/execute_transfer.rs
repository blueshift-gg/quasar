use {
    super::deposit::MultisigVaultPda,
    crate::state::MultisigConfig,
    quasar_lang::{prelude::*, remaining::RemainingAccounts},
};

#[derive(Accounts)]
pub struct ExecuteTransfer {
    #[account(
        has_one(creator),
        address = MultisigConfig::seeds(creator.address())
    )]
    pub config: Account<MultisigConfig>,
    pub creator: UncheckedAccount,
    #[account(mut, address = MultisigVaultPda::seeds(config.address()))]
    pub vault: UncheckedAccount,
    #[account(mut)]
    pub recipient: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

impl ExecuteTransfer {
    #[inline(always)]
    pub fn verify_and_transfer(
        &self,
        amount: u64,
        bumps: &ExecuteTransferBumps,
        remaining: RemainingAccounts<'_>,
    ) -> Result<(), ProgramError> {
        let stored_signers = self.config.signers();
        let threshold = self.config.threshold as u32;

        let mut approvals = 0u32;
        for stored in stored_signers {
            for account in remaining.iter() {
                let account = account?;
                if account.is_signer() && quasar_lang::keys_eq(account.address(), stored) {
                    approvals = approvals.wrapping_add(1);
                    break;
                }
            }
            if approvals >= threshold {
                break;
            }
        }

        if approvals < threshold {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let bump = [bumps.vault];
        let seeds = [
            Seed::from(b"vault" as &[u8]),
            Seed::from(self.config.address().as_ref()),
            Seed::from(bump.as_ref()),
        ];
        self.system_program
            .transfer(&self.vault, &self.recipient, amount)
            .invoke_signed(&seeds)?;
        Ok(())
    }
}

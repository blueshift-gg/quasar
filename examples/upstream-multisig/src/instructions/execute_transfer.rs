use {super::deposit::MultisigVaultPda, crate::state::MultisigConfig, quasar_lang::prelude::*};

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
        signers: Remaining<Signer, 10>,
    ) -> Result<(), ProgramError> {
        let stored_signers = self.config.signers();
        let threshold = self.config.threshold as u32;

        let mut approvals = 0u32;
        for stored in stored_signers {
            for signer in signers.iter() {
                if quasar_lang::keys_eq(signer.address(), stored) {
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

        let vault_signer = self.vault_signer(bumps);
        self.system_program
            .transfer(&self.vault, &self.recipient, amount)
            .invoke_signed(&vault_signer)?;
        Ok(())
    }
}

use {crate::state::SimpleAccount, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct RemainingConsumeReceipt {
    #[account(mut)]
    pub authority: Signer,
    #[account(
        mut,
        has_one(authority),
        close(dest = authority),
        address = SimpleAccount::seeds(authority.address()),
    )]
    pub receipt: Account<SimpleAccount>,
}

#[derive(Accounts)]
pub struct RemainingEpilogueClaim {
    #[account(mut, address = SimpleAccount::seeds(&crate::EXPECTED_ADDRESS))]
    pub vault: Account<SimpleAccount>,
}

impl RemainingEpilogueClaim {
    #[inline(always)]
    pub fn handler(&self, remaining: RemainingAccounts<'_>) -> Result<(), ProgramError> {
        let claims = remaining.parse::<RemainingConsumeReceipt, 1>()?;
        let claim = claims
            .as_slice()
            .first()
            .ok_or(ProgramError::NotEnoughAccountKeys)?;

        let reward: u64 = self.vault.value.into();
        let vault = self.vault.to_account_view();
        let claimant = claim.authority.to_account_view();
        let vault_lamports = vault
            .lamports()
            .checked_sub(reward)
            .ok_or(ProgramError::InsufficientFunds)?;
        let claimant_lamports = claimant
            .lamports()
            .checked_add(reward)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        quasar_lang::accounts::set_lamports(vault, vault_lamports);
        quasar_lang::accounts::set_lamports(claimant, claimant_lamports);
        Ok(())
    }
}

pub const REMAINING_GROUP_HAS_EPILOGUE: bool =
    <RemainingConsumeReceipt as ParseAccounts>::HAS_EPILOGUE;
pub const DECLARED_GROUP_HAS_EPILOGUE: bool =
    <RemainingEpilogueClaim as ParseAccounts>::HAS_EPILOGUE;


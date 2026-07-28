use {super::deposit::VaultPda, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, address = VaultPda::seeds(user.address()))]
    pub vault: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

impl Withdraw {
    #[inline(always)]
    pub fn withdraw(&self, amount: u64, bumps: &WithdrawBumps) -> Result<(), ProgramError> {
        // The vault is a system-owned PDA, so the program cannot debit its
        // lamports directly (the runtime rejects spends from accounts the
        // program does not own). Moving SOL out requires a System transfer
        // signed with the vault's PDA seeds.
        if self.vault.to_account_view().lamports() < amount {
            return Err(ProgramError::InsufficientFunds);
        }
        let vault_signer = self.vault_signer(bumps);
        self.system_program
            .transfer(&self.vault, &self.user, amount)
            .invoke_signed(&vault_signer)
    }
}

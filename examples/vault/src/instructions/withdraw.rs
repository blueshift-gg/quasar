use {super::deposit::VaultPda, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds(user))]
    pub vault: Pda<UncheckedAccount, VaultPda>,
    pub system_program: Program<SystemProgram>,
}

impl Withdraw {
    #[inline(always)]
    pub fn withdraw(&mut self, amount: u64) -> Result<(), ProgramError> {
        let user = *self.user.address();
        let signer = VaultPda::seeds(&user).with_bump(self.vault.bump());
        self.system_program.transfer(&mut self.vault, &mut self.user, amount).invoke_signed(&signer)
    }
}

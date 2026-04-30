use quasar_lang::prelude::*;

#[derive(Seeds)]
#[seeds(b"vault", user: Address)]
pub struct VaultPda;

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, address = VaultPda::seeds(user.address()))]
    pub vault: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

impl Deposit {
    #[inline(always)]
    pub fn deposit(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(&self.user, &self.vault, amount)
            .invoke()
    }
}

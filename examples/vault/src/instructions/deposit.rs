use quasar_lang::prelude::*;

#[derive(Seeds)]
#[seeds(b"vault", user: Address)]
pub struct VaultPda;

/// How many lamports a deposit or a withdrawal moves.
#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(mut, seeds(user))]
    pub vault: Pda<UncheckedAccount, VaultPda>,
    pub system_program: Program<SystemProgram>,
}

impl Deposit {
    #[inline(always)]
    pub fn deposit(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.system_program.transfer(&mut self.user, &mut self.vault, amount).invoke()
    }
}

use {crate::state::MultisigConfig, quasar_lang::prelude::*};

#[derive(Seeds)]
#[seeds(b"vault", config: Address)]
pub struct MultisigVaultPda;

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub depositor: Signer,
    pub config: Account<MultisigConfig>,
    #[account(mut, address = MultisigVaultPda::seeds(config.address()))]
    pub vault: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

impl Deposit {
    #[inline(always)]
    pub fn deposit(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(&self.depositor, &self.vault, amount)
            .invoke()
    }
}

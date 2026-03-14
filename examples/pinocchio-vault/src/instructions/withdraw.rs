use crate::{
    errors::VaultError,
    utils::{pda::vault_pda, Context},
};
/// # Withdraw Instruction
///
/// Withdraws SOL from vault PDA to user via direct lamport manipulation.
/// No system program CPI needed — the program owns the vault PDA.
///
/// | # | Account | Signer | Writable | Description              |
/// |---|---------|--------|----------|--------------------------|
/// | 0 | user    | yes    | yes      | Withdrawer               |
/// | 1 | vault   | no     | yes      | PDA: `["vault", user]`   |
use pinocchio::{AccountView, ProgramResult};

pub struct Withdraw<'info> {
    user: &'info AccountView,
    vault: &'info AccountView,
    amount: u64,
}

impl<'info> TryFrom<Context<'info>> for Withdraw<'info> {
    type Error = VaultError;

    fn try_from(ctx: Context<'info>) -> Result<Self, VaultError> {
        let [user, vault, ..] = ctx.accounts else {
            return Err(VaultError::NotEnoughAccountKeys);
        };

        if !user.is_signer() {
            return Err(VaultError::MissingRequiredSignature);
        }

        let (expected, _bump) = vault_pda(user.address().as_ref());
        if expected.ne(vault.address()) {
            return Err(VaultError::InvalidPDA);
        }

        if ctx.data.len() < 8 {
            return Err(VaultError::InvalidInstructionData);
        }
        // SAFETY: Bounds-checked above, SBF is little-endian.
        let amount = unsafe { *(ctx.data.as_ptr() as *const u64) };

        Ok(Self {
            user,
            vault,
            amount,
        })
    }
}

impl<'info> Withdraw<'info> {
    pub fn process(ctx: Context<'info>) -> ProgramResult {
        Self::try_from(ctx)
            .map_err(Into::into)
            .and_then(|w| w.execute())
    }

    #[inline(always)]
    fn execute(&self) -> ProgramResult {
        let vault_lamports = self.vault.lamports();
        if self.amount > vault_lamports {
            return Err(pinocchio::error::ProgramError::InsufficientFunds);
        }
        self.vault.set_lamports(vault_lamports - self.amount);
        self.user.set_lamports(self.user.lamports() + self.amount);
        Ok(())
    }
}

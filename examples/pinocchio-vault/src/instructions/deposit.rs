/// # Deposit Instruction
///
/// Transfers SOL from user to vault PDA via system program CPI.
///
/// | # | Account        | Signer | Writable | Description              |
/// |---|----------------|--------|----------|--------------------------|
/// | 0 | user           | yes    | yes      | Depositor                |
/// | 1 | vault          | no     | yes      | PDA: `["vault", user]`   |
/// | 2 | system_program | no     | no       | System program           |
use pinocchio::{AccountView, ProgramResult};
use {
    crate::{
        errors::VaultError,
        utils::{pda::vault_pda, Context},
    },
    pinocchio_system::instructions::Transfer,
};

pub struct Deposit<'info> {
    user: &'info AccountView,
    vault: &'info AccountView,
    amount: u64,
}

impl<'info> TryFrom<Context<'info>> for Deposit<'info> {
    type Error = VaultError;

    fn try_from(ctx: Context<'info>) -> Result<Self, VaultError> {
        let [user, vault, system_program, ..] = ctx.accounts else {
            return Err(VaultError::NotEnoughAccountKeys);
        };

        if !user.is_signer() {
            return Err(VaultError::MissingRequiredSignature);
        }

        if system_program.address() != &pinocchio_system::ID {
            return Err(VaultError::IncorrectSystem);
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

impl<'info> Deposit<'info> {
    pub fn process(ctx: Context<'info>) -> ProgramResult {
        Self::try_from(ctx)
            .map_err(Into::into)
            .and_then(|d| d.execute())
    }

    #[inline(always)]
    fn execute(&self) -> ProgramResult {
        Transfer {
            from: self.user,
            to: self.vault,
            lamports: self.amount,
        }
        .invoke()
    }
}

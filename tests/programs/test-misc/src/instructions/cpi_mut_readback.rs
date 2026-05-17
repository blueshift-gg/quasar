use {
    crate::state::{SimpleAccount, SimpleAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

/// Exercises mutable account data writes before and after a CPI using the same
/// account view.
#[derive(Accounts)]
pub struct CpiMutReadback {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl CpiMutReadback {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        let authority = self.account.authority;
        let bump = self.account.bump;
        let initial_lamports = self.account.to_account_view().lamports();

        self.account.set_inner(SimpleAccountInner {
            authority,
            value: new_value,
            bump,
        });

        if self.account.value != new_value {
            return Err(ProgramError::Custom(1));
        }

        self.system_program
            .transfer(&self.payer, &self.account, 1_000u64)
            .invoke()?;

        if self.account.to_account_view().lamports() != initial_lamports + 1_000 {
            return Err(ProgramError::Custom(2));
        }
        if self.account.value != new_value {
            return Err(ProgramError::Custom(3));
        }

        let second_value = new_value.wrapping_add(1);
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: second_value,
            bump,
        });

        if self.account.value != second_value {
            return Err(ProgramError::Custom(4));
        }

        Ok(())
    }
}

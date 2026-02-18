pub mod system;

pub use solana_instruction_view::cpi::{invoke_signed, invoke_signed_unchecked, CpiAccount, Signer, Seed};
pub use solana_instruction_view::{InstructionView, InstructionAccount};

use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::ProgramResult;

pub struct CpiCall<'a, const ACCTS: usize, const DATA: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    views: [&'a AccountView; ACCTS],
    data: [u8; DATA],
}

impl<'a, const ACCTS: usize, const DATA: usize> CpiCall<'a, ACCTS, DATA> {
    #[inline(always)]
    pub fn new(
        program_id: &'a Address,
        accounts: [InstructionAccount<'a>; ACCTS],
        views: [&'a AccountView; ACCTS],
        data: [u8; DATA],
    ) -> Self {
        Self { program_id, accounts, views, data }
    }

    #[inline(always)]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_inner(&[])
    }

    #[inline(always)]
    pub fn invoke_signed(&self, seeds: &[Seed]) -> ProgramResult {
        self.invoke_inner(&[Signer::from(seeds)])
    }

    #[inline(always)]
    pub fn invoke_with_signers(&self, signers: &[Signer]) -> ProgramResult {
        self.invoke_inner(signers)
    }

    #[inline(always)]
    fn invoke_inner(&self, signers: &[Signer]) -> ProgramResult {
        let cpi_accounts: [CpiAccount; ACCTS] = core::array::from_fn(|i| {
            CpiAccount::from(self.views[i])
        });

        let instruction = InstructionView {
            program_id: self.program_id,
            accounts: &self.accounts,
            data: &self.data,
        };

        unsafe { invoke_signed_unchecked(&instruction, &cpi_accounts, signers) };
        Ok(())
    }
}

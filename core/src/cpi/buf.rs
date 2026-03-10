//! Variable-length CPI call with a stack-allocated maximum-capacity buffer.

use super::{init_cpi_accounts, invoke_raw, result_from_raw, InstructionAccount, RawCpiAccount};
use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::ProgramResult;

use super::{Seed, Signer};

/// Like [`super::CpiCall`] but with a runtime-tracked `data_len` within
/// a compile-time `MAX` capacity buffer. Used for Borsh-serialized
/// instructions with variable-length data.
pub struct BufCpiCall<'a, const ACCTS: usize, const MAX: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
    data: [u8; MAX],
    data_len: usize,
}

impl<'a, const ACCTS: usize, const MAX: usize> BufCpiCall<'a, ACCTS, MAX> {
    #[inline(always)]
    pub fn new(
        program_id: &'a Address,
        accounts: [InstructionAccount<'a>; ACCTS],
        views: [&'a AccountView; ACCTS],
        data: [u8; MAX],
        data_len: usize,
    ) -> Self {
        if data_len > MAX {
            #[cold]
            #[inline(never)]
            fn capacity_exceeded() -> ! {
                panic!("BufCpiCall: data_len exceeds buffer capacity")
            }
            capacity_exceeded();
        }
        Self {
            program_id,
            accounts,
            cpi_accounts: init_cpi_accounts(views),
            data,
            data_len,
        }
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
        let result = unsafe {
            invoke_raw(
                self.program_id,
                self.accounts.as_ptr(),
                ACCTS,
                self.data.as_ptr(),
                self.data_len,
                self.cpi_accounts.as_ptr(),
                ACCTS,
                signers,
            )
        };
        result_from_raw(result)
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data(&self) -> &[u8] {
        &self.data[..self.data_len]
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data_len(&self) -> usize {
        self.data_len
    }
}

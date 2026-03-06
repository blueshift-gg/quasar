use super::{invoke_raw, InstructionAccount, RawCpiAccount};
use solana_account_view::AccountView;
use solana_address::Address;
use solana_program_error::{ProgramError, ProgramResult};

use super::{Seed, Signer};

/// CPI call with a maximum-capacity stack buffer and runtime-tracked data length.
///
/// Like [`super::CpiCall`], all data lives on the stack — no heap allocation.
/// The difference: `CpiCall` uses a compile-time `DATA` size, while `BufCpiCall`
/// stores up to `MAX` bytes but passes only `data_len` to the syscall.
///
/// Used for instructions with variable-length serialized data (e.g. Borsh strings,
/// optional vectors) where the exact byte count depends on runtime arguments.
///
/// Data is constructed by the caller via manual `core::ptr::write` /
/// `core::ptr::copy_nonoverlapping` into the buffer — no Borsh crate, no allocator.
pub struct BufCpiCall<'a, const ACCTS: usize, const MAX: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
    data: [u8; MAX],
    data_len: usize,
}

impl<'a, const ACCTS: usize, const MAX: usize> BufCpiCall<'a, ACCTS, MAX> {
    /// Creates a buffered CPI call. Panics if `data_len > MAX`.
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
        let mut cpi_accounts = core::mem::MaybeUninit::<[RawCpiAccount<'a>; ACCTS]>::uninit();
        let ptr = cpi_accounts.as_mut_ptr() as *mut RawCpiAccount<'a>;
        let mut i = 0;
        while i < ACCTS {
            // SAFETY: i < ACCTS, and ACCTS is the array length.
            // views[i] is valid because views has exactly ACCTS elements.
            unsafe { ptr.add(i).write(RawCpiAccount::from_view(views[i])) };
            i += 1;
        }
        // SAFETY: All ACCTS elements written by the loop above.
        let cpi_accounts = unsafe { cpi_accounts.assume_init() };
        Self {
            program_id,
            accounts,
            cpi_accounts,
            data,
            data_len,
        }
    }

    /// Invokes the CPI without any PDA signers.
    #[inline(always)]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_inner(&[])
    }

    /// Invokes the CPI with a single PDA signer (one set of seeds).
    #[inline(always)]
    pub fn invoke_signed(&self, seeds: &[Seed]) -> ProgramResult {
        self.invoke_inner(&[Signer::from(seeds)])
    }

    /// Invokes the CPI with multiple PDA signers.
    #[inline(always)]
    pub fn invoke_with_signers(&self, signers: &[Signer]) -> ProgramResult {
        self.invoke_inner(signers)
    }

    #[inline(always)]
    fn invoke_inner(&self, signers: &[Signer]) -> ProgramResult {
        // SAFETY: data_len <= MAX is enforced by the assert in `new()`.
        // The buffer is fully initialized up to data_len by the CPI method
        // that constructed this call.
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
        if result == 0 {
            Ok(())
        } else {
            #[cold]
            fn cpi_error(result: u64) -> ProgramError {
                ProgramError::from(result)
            }
            Err(cpi_error(result))
        }
    }

    /// Returns the serialized instruction data (only the `data_len` active bytes).
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data(&self) -> &[u8] {
        &self.data[..self.data_len]
    }

    /// Returns the number of active bytes in the data buffer.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data_len(&self) -> usize {
        self.data_len
    }
}

//! Cross-program invocation (CPI) builder with const-generic stack allocation.
//!
//! `CpiCall` is the primary type — a const-generic struct where account count
//! and data size are known at compile time, keeping everything on the stack.
//! `BufCpiCall` is the variable-length variant for Borsh-serialized instructions.
//!
//! Account types (`CpiAccount`, `InstructionAccount`, `Seed`, `Signer`) come
//! from `solana-instruction-view`. Invocation goes directly through
//! `sol_invoke_signed_c` with no intermediate borrow checking.

pub mod buf;
pub mod system;

pub use buf::BufCpiCall;
pub use solana_instruction_view::cpi::{CpiAccount, Seed, Signer};
pub use solana_instruction_view::{InstructionAccount, InstructionView};

use solana_account_view::{AccountView, RuntimeAccount};
use solana_address::Address;
use solana_program_error::{ProgramError, ProgramResult};

const RUNTIME_ACCOUNT_SIZE: usize = core::mem::size_of::<RuntimeAccount>();

// Layout-compatible helper for batched flag extraction.
// Transmuted to `CpiAccount` after construction.
#[repr(C)]
struct RawCpiBuilder {
    address: *const Address,
    lamports: *const u64,
    data_len: u64,
    data: *const u8,
    owner: *const Address,
    rent_epoch: u64,
    // [is_signer, is_writable, executable, 0, 0, 0, 0, 0]
    flags: u64,
}

const _: () = assert!(core::mem::size_of::<RawCpiBuilder>() == 56);
const _: () = assert!(core::mem::size_of::<RawCpiBuilder>() == core::mem::size_of::<CpiAccount>());
const _: () =
    assert!(core::mem::align_of::<RawCpiBuilder>() == core::mem::align_of::<CpiAccount>());

/// Construct a `CpiAccount` from an `AccountView` with batched flag extraction.
///
/// Reads the 4-byte header as u32, shifts right 8 to drop borrow_state,
/// keeping [is_signer, is_writable, executable]. The result is transmuted
/// to the upstream `CpiAccount` which has an identical `#[repr(C)]` layout.
#[inline(always)]
pub(crate) fn cpi_account_from_view(view: &AccountView) -> CpiAccount<'_> {
    let raw = view.account_ptr();
    unsafe {
        let flags = (raw as *const u32).read_unaligned() >> 8;
        let builder = RawCpiBuilder {
            address: &(*raw).address,
            lamports: &(*raw).lamports,
            data_len: (*raw).data_len,
            data: (raw as *const u8).add(RUNTIME_ACCOUNT_SIZE),
            owner: &(*raw).owner,
            rent_epoch: 0,
            flags: flags as u64,
        };
        core::mem::transmute(builder)
    }
}

// --- Direct syscall wrapper ---

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[repr(C)]
struct CInstruction<'a> {
    program_id: *const Address,
    accounts: *const InstructionAccount<'a>,
    accounts_len: u64,
    data: *const u8,
    data_len: u64,
}

#[inline(always)]
#[allow(clippy::too_many_arguments, unused_variables)]
pub(crate) unsafe fn invoke_raw(
    program_id: *const Address,
    instruction_accounts: *const InstructionAccount,
    instruction_accounts_len: usize,
    data: *const u8,
    data_len: usize,
    cpi_accounts: *const CpiAccount,
    cpi_accounts_len: usize,
    signers: &[Signer],
) -> u64 {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        use solana_define_syscall::definitions::sol_invoke_signed_c;

        let instruction = CInstruction {
            program_id,
            accounts: instruction_accounts,
            accounts_len: instruction_accounts_len as u64,
            data,
            data_len: data_len as u64,
        };

        sol_invoke_signed_c(
            &instruction as *const _ as *const u8,
            cpi_accounts as *const u8,
            cpi_accounts_len as u64,
            signers as *const _ as *const u8,
            signers.len() as u64,
        )
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    0
}

/// Convert a raw syscall result to `ProgramResult`.
#[inline(always)]
pub(crate) fn result_from_raw(result: u64) -> ProgramResult {
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

/// Initialize a `[CpiAccount; N]` from an array of views.
#[inline(always)]
pub(crate) fn init_cpi_accounts<'a, const N: usize>(
    views: [&'a AccountView; N],
) -> [CpiAccount<'a>; N] {
    let mut buf = core::mem::MaybeUninit::<[CpiAccount<'a>; N]>::uninit();
    let ptr = buf.as_mut_ptr() as *mut CpiAccount<'a>;
    let mut i = 0;
    while i < N {
        unsafe { ptr.add(i).write(cpi_account_from_view(views[i])) };
        i += 1;
    }
    unsafe { buf.assume_init() }
}

// --- CpiCall ---

/// Const-generic CPI builder. All data lives on the stack.
///
/// `ACCTS` = account count, `DATA` = instruction data byte length.
pub struct CpiCall<'a, const ACCTS: usize, const DATA: usize> {
    program_id: &'a Address,
    accounts: [InstructionAccount<'a>; ACCTS],
    cpi_accounts: [CpiAccount<'a>; ACCTS],
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
        Self {
            program_id,
            accounts,
            cpi_accounts: init_cpi_accounts(views),
            data,
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
                DATA,
                self.cpi_accounts.as_ptr(),
                ACCTS,
                signers,
            )
        };
        result_from_raw(result)
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data(&self) -> &[u8] {
        &self.data
    }
}

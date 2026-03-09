//! Cross-program invocation (CPI) builder with const-generic stack allocation.
//!
//! `CpiCall` is the primary type — a const-generic struct where account count
//! and data size are known at compile time, keeping everything on the stack.
//! `BufCpiCall` is the variable-length variant for Borsh-serialized instructions.
//!
//! Both types invoke the `sol_invoke_signed_c` syscall directly, bypassing
//! any intermediate instruction representation.

pub mod buf;
pub mod system;

pub use buf::BufCpiCall;
pub use solana_instruction_view::cpi::{Seed, Signer};
pub use solana_instruction_view::InstructionAccount;

use core::marker::PhantomData;
use solana_account_view::{AccountView, RuntimeAccount};
use solana_address::Address;
use solana_program_error::{ProgramError, ProgramResult};

const RUNTIME_ACCOUNT_SIZE: usize = core::mem::size_of::<RuntimeAccount>();

// Layout-compatible with the runtime's CpiAccount (u8 flags variant).
#[repr(C)]
pub(crate) struct RawCpiAccount<'a> {
    address: *const Address,
    lamports: *const u64,
    data_len: u64,
    data: *const u8,
    owner: *const Address,
    rent_epoch: u64,
    is_signer: u8,
    is_writable: u8,
    executable: u8,
    _pad: [u8; 5],
    _lifetime: PhantomData<&'a AccountView>,
}

const _: () = assert!(core::mem::size_of::<RawCpiAccount>() == 56);
const _: () = assert!(core::mem::align_of::<RawCpiAccount>() == 8);

impl<'a> RawCpiAccount<'a> {
    #[inline(always)]
    pub(crate) fn from_view(view: &'a AccountView) -> Self {
        let raw = view.account_ptr();
        unsafe {
            let mut cpi = RawCpiAccount {
                address: &(*raw).address,
                lamports: &(*raw).lamports,
                data_len: (*raw).data_len,
                data: (raw as *const u8).add(RUNTIME_ACCOUNT_SIZE),
                owner: &(*raw).owner,
                rent_epoch: 0,
                is_signer: 0,
                is_writable: 0,
                executable: 0,
                _pad: [0u8; 5],
                _lifetime: PhantomData,
            };
            // Read the 4-byte header as u32, shift right 8 to drop borrow_state,
            // keeping [is_signer, is_writable, executable]. Write as u64 so
            // zero-extension covers the 5 pad bytes.
            let flags = (raw as *const u32).read_unaligned() >> 8;
            core::ptr::write(
                core::ptr::addr_of_mut!(cpi.is_signer) as *mut u64,
                flags as u64,
            );
            cpi
        }
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
    cpi_accounts: *const RawCpiAccount,
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

/// Initialize a `MaybeUninit<[RawCpiAccount; N]>` from an array of views.
#[inline(always)]
pub(crate) fn init_cpi_accounts<'a, const N: usize>(
    views: [&'a AccountView; N],
) -> [RawCpiAccount<'a>; N] {
    let mut buf = core::mem::MaybeUninit::<[RawCpiAccount<'a>; N]>::uninit();
    let ptr = buf.as_mut_ptr() as *mut RawCpiAccount<'a>;
    let mut i = 0;
    while i < N {
        unsafe { ptr.add(i).write(RawCpiAccount::from_view(views[i])) };
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
    cpi_accounts: [RawCpiAccount<'a>; ACCTS],
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

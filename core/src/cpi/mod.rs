pub mod buf;
pub mod system;

pub use buf::BufCpiCall;
pub use solana_instruction_view::cpi::{Seed, Signer};
pub use solana_instruction_view::InstructionAccount;

use core::marker::PhantomData;
use solana_account_view::{AccountView, RuntimeAccount};
use solana_address::Address;
use solana_program_error::{ProgramError, ProgramResult};

// --- Raw CPI account (layout-compatible with CpiAccount, uses u8 flags) ---

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
        // SAFETY: raw is a valid pointer to RuntimeAccount from the SVM input buffer.
        // All fields are read through their pub accessors on RuntimeAccount.
        unsafe {
            RawCpiAccount {
                address: &(*raw).address,
                lamports: &(*raw).lamports,
                data_len: (*raw).data_len,
                data: (raw as *const u8).add(core::mem::size_of::<RuntimeAccount>()),
                owner: &(*raw).owner,
                rent_epoch: 0,
                is_signer: (*raw).is_signer,
                is_writable: (*raw).is_writable,
                executable: (*raw).executable,
                _pad: [0u8; 5],
                _lifetime: PhantomData,
            }
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
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn invoke_raw(
    _program_id: *const Address,
    _instruction_accounts: *const InstructionAccount,
    _instruction_accounts_len: usize,
    _data: *const u8,
    _data_len: usize,
    _cpi_accounts: *const RawCpiAccount,
    _cpi_accounts_len: usize,
    _signers: &[Signer],
) -> u64 {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        use solana_define_syscall::definitions::sol_invoke_signed_c;

        let instruction = CInstruction {
            program_id: _program_id,
            accounts: _instruction_accounts,
            accounts_len: _instruction_accounts_len as u64,
            data: _data,
            data_len: _data_len as u64,
        };

        sol_invoke_signed_c(
            &instruction as *const _ as *const u8,
            _cpi_accounts as *const u8,
            _cpi_accounts_len as u64,
            _signers as *const _ as *const u8,
            _signers.len() as u64,
        )
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    0
}

// --- CpiCall ---

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
        let cpi_accounts = views.map(RawCpiAccount::from_view);
        Self {
            program_id,
            accounts,
            cpi_accounts,
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
        if result == 0 {
            Ok(())
        } else {
            Err(ProgramError::from(result))
        }
    }

    /// Returns the serialized instruction data.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data(&self) -> &[u8] {
        &self.data
    }
}

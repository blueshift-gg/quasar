//! Cross-program invocation (CPI) builder with const-generic stack allocation.
//!
//! `CpiCall` is the primary type — a const-generic struct where account count
//! and data size are known at compile time, keeping everything on the stack.
//! `BufCpiCall` is the variable-length variant for Borsh-serialized
//! instructions.
//!
//! Account types (`CpiAccount`, `InstructionAccount`, `Seed`, `Signer`) come
//! from `solana-instruction-view`. Invocation goes through the upstream
//! `invoke_signed_unchecked` with no intermediate borrow checking.

pub mod buf;
pub mod system;

pub use {
    buf::BufCpiCall,
    solana_instruction_view::{
        cpi::{CpiAccount, Seed, Signer},
        InstructionAccount, InstructionView,
    },
};
use {
    solana_account_view::{AccountView, RuntimeAccount},
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[repr(C)]
struct CInstruction<'a> {
    program_id: *const Address,
    accounts: *const InstructionAccount<'a>,
    accounts_len: u64,
    data: *const u8,
    data_len: u64,
}

/// Direct CPI syscall — passes raw pointers to `sol_invoke_signed_c`.
///
/// Uses SDK types (`InstructionAccount`, `CpiAccount`, `Seed`, `Signer`)
/// but bypasses `InstructionView` / `invoke_signed_unchecked` to go
/// directly to the `sol_invoke_signed_c` syscall.
///
/// # Safety
///
/// - `program_id` must point to a valid `Address`.
/// - `instruction_accounts[..instruction_accounts_len]` must be valid.
/// - `data[..data_len]` must be valid for reads.
/// - `cpi_accounts[..cpi_accounts_len]` must be valid.
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
        let instruction = CInstruction {
            program_id,
            accounts: instruction_accounts,
            accounts_len: instruction_accounts_len as u64,
            data,
            data_len: data_len as u64,
        };

        // SAFETY: `CInstruction` is `#[repr(C)]` and layout-compatible with
        // the C struct expected by `sol_invoke_signed_c`. All pointer fields
        // are valid per this function's safety contract.
        solana_instruction_view::cpi::sol_invoke_signed_c(
            &instruction as *const _ as *const u8,
            cpi_accounts as *const u8,
            cpi_accounts_len as u64,
            signers as *const _ as *const u8,
            signers.len() as u64,
        )
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        // SAFETY: Caller guarantees all pointer/length pairs are valid.
        // Reconstructs safe Rust types for the off-chain invoke path.
        let instruction = InstructionView {
            program_id: &*program_id,
            accounts: core::slice::from_raw_parts(instruction_accounts, instruction_accounts_len),
            data: core::slice::from_raw_parts(data, data_len),
        };
        let cpi_slice = core::slice::from_raw_parts(cpi_accounts, cpi_accounts_len);
        solana_instruction_view::cpi::invoke_signed_unchecked(&instruction, cpi_slice, signers);
        0
    }
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

// Guard the 4-byte header layout assumed by `cpi_account_from_view`.
// The flag extraction reads bytes 0-3 as u32 and shifts right 8 to drop
// borrow_state, keeping [is_signer, is_writable, executable].
// If solana-account-view reorders these fields, these assertions catch it.
const _: () = assert!(core::mem::offset_of!(RuntimeAccount, borrow_state) == 0);
const _: () = assert!(core::mem::offset_of!(RuntimeAccount, is_signer) == 1);
const _: () = assert!(core::mem::offset_of!(RuntimeAccount, is_writable) == 2);
const _: () = assert!(core::mem::offset_of!(RuntimeAccount, executable) == 3);

/// Construct a `CpiAccount` from an `AccountView` with batched flag extraction.
///
/// Reads the 4-byte header `[borrow_state, is_signer, is_writable, executable]`
/// as u32, shifts right 8 to drop `borrow_state`, keeping the three flag bytes.
/// The result is transmuted to `CpiAccount` which has an identical `#[repr(C)]`
/// layout (verified by compile-time assertions above).
#[inline(always)]
pub(crate) fn cpi_account_from_view(view: &AccountView) -> CpiAccount<'_> {
    let raw = view.account_ptr();
    // SAFETY:
    // - `raw` points to a valid `RuntimeAccount` (guaranteed by `AccountView`).
    // - The u32 read is unaligned but SBF handles this natively; on other targets
    //   `read_unaligned` is correct by definition.
    // - `RawCpiBuilder` has identical size/alignment as `CpiAccount` (verified by
    //   compile-time assertions). The transmute reinterprets the builder as the
    //   upstream type with no layout change.
    // - Account data immediately follows the `RuntimeAccount` header.
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

/// Initialize a `[CpiAccount; N]` from an array of account views.
#[inline(always)]
pub(crate) fn init_cpi_accounts<'a, const N: usize>(
    views: [&'a AccountView; N],
) -> [CpiAccount<'a>; N] {
    let mut buf = core::mem::MaybeUninit::<[CpiAccount<'a>; N]>::uninit();
    let ptr = buf.as_mut_ptr() as *mut CpiAccount<'a>;
    let mut i = 0;
    while i < N {
        // SAFETY: `ptr.add(i)` is within the `MaybeUninit` allocation for i < N.
        unsafe { ptr.add(i).write(cpi_account_from_view(views[i])) };
        i += 1;
    }
    // SAFETY: All N elements written above.
    unsafe { buf.assume_init() }
}

// --- CpiCall ---

/// Const-generic CPI builder where account count and data size are known
/// at compile time. All data lives on the stack — no heap allocation.
///
/// Typically constructed by helper functions (e.g. `system::transfer`,
/// `system::create_account`) rather than directly.
///
/// # Type parameters
///
/// - `ACCTS`: number of accounts in the instruction.
/// - `DATA`: byte length of the instruction data.
pub struct CpiCall<'a, const ACCTS: usize, const DATA: usize> {
    /// Target program to invoke.
    program_id: &'a Address,
    /// Instruction-level account metadata (address + flags).
    accounts: [InstructionAccount<'a>; ACCTS],
    /// Full account views for the runtime to pass through.
    cpi_accounts: [CpiAccount<'a>; ACCTS],
    /// Serialized instruction data (discriminator + arguments).
    data: [u8; DATA],
}

impl<'a, const ACCTS: usize, const DATA: usize> CpiCall<'a, ACCTS, DATA> {
    /// Create a new CPI call.
    ///
    /// `accounts` carries the instruction-level metadata (address, signer,
    /// writable flags). `views` provides the full account data for the
    /// runtime. `data` is the serialized instruction payload.
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

    /// Invoke the CPI without any PDA signers.
    #[inline(always)]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_inner(&[])
    }

    /// Invoke the CPI with a single PDA signer (seeds for one address).
    #[inline(always)]
    pub fn invoke_signed(&self, seeds: &[Seed]) -> ProgramResult {
        self.invoke_inner(&[Signer::from(seeds)])
    }

    /// Invoke the CPI with multiple PDA signers.
    #[inline(always)]
    pub fn invoke_with_signers(&self, signers: &[Signer]) -> ProgramResult {
        self.invoke_inner(signers)
    }

    #[inline(always)]
    fn invoke_inner(&self, signers: &[Signer]) -> ProgramResult {
        // SAFETY: All pointer/length pairs derive from owned fixed-size arrays
        // with const-generic lengths, so they are always valid and in-bounds.
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

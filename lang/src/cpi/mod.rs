//! Cross-program invocation (CPI) builders with const-generic stack allocation.
//!
//! `CpiCall` is the primary type: a const-generic struct where account count
//! and data size are known at compile time, keeping everything on the stack.
//! `CpiDynamic` is the variable-length variant where both account count and
//! data size are runtime-tracked within compile-time capacity bounds.
//!
//! Account types (`CpiAccount`, `InstructionAccount`, `Seed`, `Signer`) come
//! from `solana-instruction-view`. Invocation goes through the upstream
//! `invoke_signed_unchecked` with no intermediate borrow checking.

pub mod dyn_cpi;
/// System Program CPI builders.
pub mod system;

use {
    crate::{error::QuasarError, instruction_arg::InstructionArg},
    core::mem::MaybeUninit,
    solana_account_view::{AccountView, RuntimeAccount},
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};
pub use {
    dyn_cpi::CpiDynamic,
    solana_instruction_view::{
        cpi::{CpiAccount, Seed, Signer, MAX_RETURN_DATA},
        InstructionAccount, InstructionView,
    },
};

/// Signer seed source for a single PDA signer.
///
/// Generated account-owned signer helpers return values that implement this
/// trait, so CPI callers do not need to materialize or expose raw seed slices.
pub trait CpiSignerSeeds {
    /// Borrows this seed source as the signer array expected by CPI calls.
    fn with_signers<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&[Signer<'_, '_>]) -> R;
}

impl<'seed, const N: usize> CpiSignerSeeds for [Seed<'seed>; N] {
    #[inline(always)]
    fn with_signers<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&[Signer<'_, '_>]) -> R,
    {
        let signer = Signer::from(self);
        f(core::slice::from_ref(&signer))
    }
}

impl<'seed> CpiSignerSeeds for [Seed<'seed>] {
    #[inline(always)]
    fn with_signers<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&[Signer<'_, '_>]) -> R,
    {
        let signer = Signer::from(self);
        f(core::slice::from_ref(&signer))
    }
}

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
#[repr(C)]
struct CInstruction<'a> {
    program_id: *const Address,
    accounts: *const InstructionAccount<'a>,
    accounts_len: u64,
    data: *const u8,
    data_len: u64,
}

/// Direct CPI syscall: passes raw pointers to `sol_invoke_signed_c`.
///
/// Uses SDK types (`InstructionAccount`, `CpiAccount`, `Seed`, `Signer`)
/// but bypasses `InstructionView` / `invoke_signed_unchecked` to go
/// directly to the `sol_invoke_signed_c` syscall.
///
/// Kani proof: `invoke_raw_length_cast_lossless`.
///
/// # Safety
///
/// - `program_id` must point to a valid `Address`.
/// - `instruction_accounts[..instruction_accounts_len]` must be valid.
/// - `data[..data_len]` must be valid for reads.
/// - `cpi_accounts[..cpi_accounts_len]` must be valid.
#[inline(always)]
#[allow(clippy::too_many_arguments, unused_variables)]
pub unsafe fn invoke_raw(
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
        let instruction = unsafe {
            InstructionView {
                program_id: &*program_id,
                accounts: core::slice::from_raw_parts(
                    instruction_accounts,
                    instruction_accounts_len,
                ),
                data: core::slice::from_raw_parts(data, data_len),
            }
        };
        // SAFETY: Same pointer/length contract as above for CPI account metas.
        let cpi_slice = unsafe { core::slice::from_raw_parts(cpi_accounts, cpi_accounts_len) };
        solana_instruction_view::cpi::invoke_signed_unchecked(&instruction, cpi_slice, signers);
        0
    }
}

/// Convert a raw syscall result to `ProgramResult`.
#[inline(always)]
pub fn result_from_raw(result: u64) -> ProgramResult {
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

/// Return data captured from a CPI invocation.
pub struct CpiReturn {
    program_id: Address,
    /// Return-data buffer. Only the first `data_len` bytes are initialized;
    /// `sol_get_return_data` leaves the tail untouched, so the buffer is kept
    /// as `MaybeUninit` and never fully `assume_init`ed.
    data: MaybeUninit<[u8; MAX_RETURN_DATA]>,
    data_len: usize,
}

impl CpiReturn {
    /// Program that most recently set the return data.
    #[inline(always)]
    pub fn program_id(&self) -> &Address {
        &self.program_id
    }

    /// Raw return-data bytes (the initialized `data_len`-byte prefix).
    ///
    /// Kani proof: `cpi_return_as_slice_in_bounds`.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `sol_get_return_data` initialized `data_len <= MAX_RETURN_DATA`
        // bytes at the front of `data`; slice only that initialized prefix so the
        // uninitialized tail is never read.
        unsafe { &*core::ptr::slice_from_raw_parts(self.data.as_ptr() as *const u8, self.data_len) }
    }

    /// Decode return data as a fixed-size Quasar instruction-arg type.
    ///
    /// Kani proof: `cpi_return_decode_copy_in_bounds`.
    #[inline(always)]
    pub fn decode<T: InstructionArg>(&self) -> Result<T, ProgramError> {
        let expected_len = core::mem::size_of::<T::Zc>();
        if self.data_len != expected_len {
            return Err(QuasarError::InvalidReturnData.into());
        }

        let mut zc = MaybeUninit::<T::Zc>::uninit();
        // SAFETY: The length check above proves the initialized return-data
        // prefix is exactly `size_of::<T::Zc>()`, so the copy fully initializes
        // `zc` without reading past `self.data`.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr() as *const u8,
                zc.as_mut_ptr() as *mut u8,
                expected_len,
            );
        }
        // SAFETY: The previous copy initialized every byte of `T::Zc`.
        let zc = unsafe { zc.assume_init() };
        // Return data is untrusted: validate before `from_zc`, whose
        // contract assumes a validated companion (e.g. enum `from_zc` calls
        // `unreachable_unchecked` on undeclared discriminants).
        T::validate_zc(&zc)?;
        Ok(T::from_zc(&zc))
    }
}

#[inline(always)]
fn get_cpi_return() -> Result<CpiReturn, ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut program_id = MaybeUninit::<Address>::uninit();
        // SAFETY: sol_get_return_data writes `min(size, MAX_RETURN_DATA)` bytes
        // into the buffer. All consumer paths (as_slice, decode) are bounded by
        // `data_len`, so uninitialized trailing bytes are never read.
        let mut data = MaybeUninit::<[u8; MAX_RETURN_DATA]>::uninit();
        let size = unsafe {
            solana_define_syscall::definitions::sol_get_return_data(
                data.as_mut_ptr() as *mut u8,
                MAX_RETURN_DATA as u64,
                program_id.as_mut_ptr() as *mut _ as *mut u8,
            )
        } as usize;

        if size == 0 {
            return Err(QuasarError::MissingReturnData.into());
        }

        // `sol_get_return_data` initialized `min(size, MAX_RETURN_DATA)` bytes;
        // the tail stays uninitialized. The buffer is kept as `MaybeUninit`
        // (never `assume_init`ed) so moving it into `CpiReturn` is well-defined,
        // and both consumers (`as_slice`, `decode`) read only the `data_len`
        // prefix.
        return Ok(CpiReturn {
            // SAFETY: sol_get_return_data wrote the 32-byte program id.
            program_id: unsafe { program_id.assume_init() },
            data,
            data_len: core::cmp::min(size, MAX_RETURN_DATA),
        });
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        Err(QuasarError::MissingReturnData.into())
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

// Guard the destination offsets of the transmuted flag bytes. After the
// transmute, `CpiAccount`'s `is_signer`/`is_writable`/`executable` occupy the
// three low bytes of `RawCpiBuilder::flags` at offsets 48/49/50 (little-endian
// byte 0/1/2 of the u64). `CpiAccount`'s own fields are crate-private in
// solana-instruction-view, so we cannot `offset_of!` them directly; instead we
// pin the equivalent offset on the layout-compatible `RawCpiBuilder` (whose
// size and alignment already equal `CpiAccount`'s, asserted above). The runtime
// test `cpi_account_from_view_matches_upstream_layout` cross-checks the actual
// upstream field values.
const _: () = assert!(core::mem::offset_of!(RawCpiBuilder, flags) == 48);

/// Construct a `CpiAccount` from an `AccountView` with batched flag extraction.
///
/// Reads the 4-byte header `[borrow_state, is_signer, is_writable, executable]`
/// as u32, shifts right 8 to drop `borrow_state`, keeping the three flag bytes.
/// The result is transmuted to `CpiAccount` which has an identical `#[repr(C)]`
/// layout (verified by compile-time assertions above and by the
/// `cpi_account_from_view_matches_upstream_layout` test).
///
/// Kani proofs: `flag_extraction_shift_correctness`,
/// `raw_cpi_builder_layout_matches_cpi_account`,
/// `cpi_account_data_offset_valid`.
#[inline(always)]
pub(crate) fn cpi_account_from_view(view: &AccountView) -> CpiAccount<'_> {
    let raw = view.account_ptr();
    // SAFETY:
    // - `raw` points to a valid `RuntimeAccount` (guaranteed by `AccountView`).
    // - The u32 read is unaligned but SBF handles this natively; on other targets
    //   `read_unaligned` is correct by definition.
    // - `RawCpiBuilder` has identical size/alignment/field order as `CpiAccount`
    //   (compile-time assertions + unit test). The transmute reinterprets the
    //   builder as the upstream type with no layout change.
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
///
/// Kani proof: `init_cpi_accounts_loop_covers_all_indices`.
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

/// Const-generic CPI builder where account count and data size are known
/// at compile time. All data lives on the stack; no heap allocation.
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
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_inner(&[])
    }

    /// Invoke the CPI with a single PDA signer (seeds for one address).
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_signed<S>(&self, seeds: &S) -> ProgramResult
    where
        S: CpiSignerSeeds + ?Sized,
    {
        seeds.with_signers(|signers| self.invoke_inner(signers))
    }

    /// Invoke the CPI with multiple PDA signers.
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_with_signers(&self, signers: &[Signer]) -> ProgramResult {
        self.invoke_inner(signers)
    }

    /// Invoke the CPI and read back raw return data.
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_with_return(&self) -> Result<CpiReturn, ProgramError> {
        self.invoke_with_return_inner(&[])
    }

    /// Invoke the CPI with one PDA signer and read back raw return data.
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_signed_with_return<S>(&self, seeds: &S) -> Result<CpiReturn, ProgramError>
    where
        S: CpiSignerSeeds + ?Sized,
    {
        seeds.with_signers(|signers| self.invoke_with_return_inner(signers))
    }

    /// Invoke the CPI with multiple PDA signers and read back raw return data.
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_with_signers_with_return(
        &self,
        signers: &[Signer],
    ) -> Result<CpiReturn, ProgramError> {
        self.invoke_with_return_inner(signers)
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

    #[inline(always)]
    fn invoke_with_return_inner(&self, signers: &[Signer]) -> Result<CpiReturn, ProgramError> {
        crate::return_data::set_return_data(&[]);
        // SAFETY: Same pointer/length argument as `invoke_inner`; all arrays
        // are owned fixed-size fields and fully initialized.
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
        result_from_raw(result)?;
        let ret = get_cpi_return()?;
        if !crate::keys_eq(ret.program_id(), self.program_id) {
            return Err(QuasarError::ReturnDataFromWrongProgram.into());
        }
        Ok(ret)
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    /// Returns the encoded instruction data for off-chain inspection and tests.
    pub fn instruction_data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(kani)]
#[path = "../../kani/cpi/account_buffer.rs"]
mod account_buffer;
#[cfg(kani)]
pub(crate) use account_buffer::{AccountBuffer, MIN_ACCOUNT_BUF};
#[cfg(test)]
mod tests {
    extern crate std;

    use {
        super::*,
        quasar_derive::QuasarSerialize,
        solana_account_view::{RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
    };

    pub(super) struct AccountBuffer {
        inner: std::vec::Vec<u64>,
    }

    impl AccountBuffer {
        pub(in crate::cpi) fn new(data_len: usize) -> Self {
            let byte_len =
                core::mem::size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE;
            Self {
                inner: std::vec![0u64; byte_len.div_ceil(8)],
            }
        }

        pub(in crate::cpi) fn raw(&mut self) -> *mut RuntimeAccount {
            self.inner.as_mut_ptr() as *mut RuntimeAccount
        }

        pub(in crate::cpi) fn init(
            &mut self,
            address: [u8; 32],
            owner: [u8; 32],
            data_len: usize,
            is_signer: bool,
            is_writable: bool,
            executable: bool,
        ) {
            let raw = self.raw();
            // SAFETY: `raw` points into `inner`, which was allocated large
            // enough for a `RuntimeAccount` header plus requested data bytes.
            unsafe {
                (*raw).borrow_state = NOT_BORROWED;
                (*raw).is_signer = is_signer as u8;
                (*raw).is_writable = is_writable as u8;
                (*raw).executable = executable as u8;
                (*raw).padding = [0u8; 4];
                (*raw).address = Address::new_from_array(address);
                (*raw).owner = Address::new_from_array(owner);
                (*raw).lamports = 123;
                (*raw).data_len = data_len as u64;
            }
        }

        pub(in crate::cpi) unsafe fn view(&mut self) -> AccountView {
            // SAFETY: Caller ensures the buffer was initialized with `init`
            // before constructing an `AccountView`.
            unsafe { AccountView::new_unchecked(self.raw()) }
        }
    }

    fn cpi_account_bytes<'a>(account: &'a CpiAccount<'a>) -> &'a [u8] {
        // SAFETY: `account` is a valid initialized `CpiAccount`; the returned
        // byte slice covers exactly its object representation for comparison.
        unsafe {
            core::slice::from_raw_parts(
                account as *const CpiAccount<'_> as *const u8,
                core::mem::size_of::<CpiAccount<'_>>(),
            )
        }
    }

    fn zc_bytes<T: InstructionArg>(zc: &T::Zc) -> &[u8] {
        // SAFETY: `zc` is a valid initialized reference to exactly `size_of::<T::Zc>()`
        // bytes.
        unsafe {
            core::slice::from_raw_parts(
                zc as *const T::Zc as *const u8,
                core::mem::size_of::<T::Zc>(),
            )
        }
    }

    fn return_with_data(program_id: [u8; 32], bytes: &[u8]) -> CpiReturn {
        assert!(bytes.len() <= MAX_RETURN_DATA);
        // Leave the tail uninitialized, mirroring `sol_get_return_data`, so the
        // Miri run exercises reads bounded by `data_len` against real uninit
        // memory rather than a zeroed buffer.
        let mut data = MaybeUninit::<[u8; MAX_RETURN_DATA]>::uninit();
        // SAFETY: `bytes.len() <= MAX_RETURN_DATA`, so the copy stays in bounds
        // and initializes exactly the `data_len` prefix.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                data.as_mut_ptr() as *mut u8,
                bytes.len(),
            );
        }
        CpiReturn {
            program_id: Address::new_from_array(program_id),
            data,
            data_len: bytes.len(),
        }
    }

    #[test]
    fn cpi_return_reads_only_initialized_prefix() {
        // The buffer tail is uninitialized; `as_slice`/`decode` must read only
        // the `data_len` prefix. Running under Miri catches any access to the
        // uninitialized region.
        let ret = return_with_data([9u8; 32], &[1, 2, 3, 4]);
        assert_eq!(ret.as_slice(), &[1u8, 2, 3, 4]);
        assert_eq!(ret.program_id(), &Address::new_from_array([9u8; 32]));
        assert_eq!(
            ret.decode::<u32>().unwrap(),
            u32::from_le_bytes([1, 2, 3, 4])
        );
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, QuasarSerialize)]
    struct ReturnPayload {
        amount: u64,
        flag: bool,
    }

    #[test]
    fn decode_primitive_return_uses_instruction_arg_layout() {
        let pod = <u64 as InstructionArg>::to_zc(&777u64);
        let ret = return_with_data([1u8; 32], zc_bytes::<u64>(&pod));
        assert_eq!(ret.decode::<u64>().unwrap(), 777u64);
    }

    #[test]
    fn decode_struct_return_uses_zc_companion_layout() {
        let payload = ReturnPayload {
            amount: 55,
            flag: true,
        };
        let zc = <ReturnPayload as InstructionArg>::to_zc(&payload);
        let ret = return_with_data([2u8; 32], zc_bytes::<ReturnPayload>(&zc));
        assert_eq!(ret.decode::<ReturnPayload>().unwrap(), payload);
    }

    #[test]
    fn decode_rejects_invalid_bool() {
        let ret = return_with_data([1u8; 32], &[2u8]);
        assert_eq!(
            ret.decode::<bool>(),
            Err(ProgramError::InvalidInstructionData)
        );
    }

    #[test]
    fn decode_rejects_invalid_option_tag() {
        let mut bytes = std::vec![0u8; core::mem::size_of::<<Option<u64> as InstructionArg>::Zc>()];
        bytes[0] = 2;
        let ret = return_with_data([1u8; 32], &bytes);
        assert_eq!(
            ret.decode::<Option<u64>>(),
            Err(ProgramError::InvalidInstructionData)
        );
    }

    #[test]
    fn decode_rejects_invalid_nested_struct_field() {
        let payload = ReturnPayload {
            amount: 55,
            flag: true,
        };
        let zc = <ReturnPayload as InstructionArg>::to_zc(&payload);
        let mut bytes = zc_bytes::<ReturnPayload>(&zc).to_vec();
        *bytes.last_mut().unwrap() = 2;
        let ret = return_with_data([1u8; 32], &bytes);
        assert_eq!(
            ret.decode::<ReturnPayload>(),
            Err(ProgramError::InvalidInstructionData)
        );
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, QuasarSerialize)]
    #[repr(u8)]
    enum Status {
        Active = 1,
        Closed = 2,
    }

    #[test]
    fn decode_rejects_undeclared_enum_discriminant() {
        let ret = return_with_data([1u8; 32], &[9u8]);
        assert_eq!(
            ret.decode::<Status>(),
            Err(ProgramError::InvalidInstructionData)
        );
    }

    #[test]
    fn cpi_account_from_view_matches_upstream_layout() {
        // CpiAccount is 56 bytes: six u64 fields + 3 bool flags + 5 padding.
        // Upstream's From impl leaves the 5 trailing padding bytes
        // uninitialized, so only compare the first 51 meaningful bytes.
        const CMP_LEN: usize = 51;

        for (is_signer, is_writable, executable) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            let mut buf = AccountBuffer::new(16);
            buf.init(
                [0x11; 32],
                [0x22; 32],
                16,
                is_signer,
                is_writable,
                executable,
            );
            let view = unsafe { buf.view() };

            let upstream = CpiAccount::from(&view);
            let quasar = cpi_account_from_view(&view);

            assert_eq!(
                &cpi_account_bytes(&quasar)[..CMP_LEN],
                &cpi_account_bytes(&upstream)[..CMP_LEN],
            );
        }
    }
}

#[cfg(kani)]
#[path = "../../kani/cpi/mod.rs"]
mod kani_proofs;

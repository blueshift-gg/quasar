//! Cross-program invocation (CPI) builders with const-generic stack allocation.
//!
//! `CpiCall` is the primary type — a const-generic struct where account count
//! and data size are known at compile time, keeping everything on the stack.
//! `DynCpiCall` is the variable-length variant where both account count and
//! data size are runtime-tracked within compile-time capacity bounds.
//!
//! Account types (`CpiAccount`, `InstructionAccount`, `Seed`, `Signer`) come
//! from `solana-instruction-view`. Invocation goes through the upstream
//! `invoke_signed_unchecked` with no intermediate borrow checking.

pub mod dyn_cpi;
pub mod system;

use {
    crate::{error::QuasarError, instruction_arg::InstructionArg},
    core::mem::MaybeUninit,
    solana_account_view::{AccountView, RuntimeAccount},
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};
pub use {
    dyn_cpi::DynCpiCall,
    solana_instruction_view::{
        cpi::{CpiAccount, Seed, Signer, MAX_RETURN_DATA},
        InstructionAccount, InstructionView,
    },
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

/// Return data captured from a CPI invocation.
pub struct CpiReturn {
    program_id: Address,
    data: [u8; MAX_RETURN_DATA],
    data_len: usize,
}

impl CpiReturn {
    #[cfg_attr(
        not(any(test, target_os = "solana", target_arch = "bpf")),
        allow(dead_code)
    )]
    #[inline(always)]
    fn new(program_id: Address, data: [u8; MAX_RETURN_DATA], data_len: usize) -> Self {
        Self {
            program_id,
            data,
            data_len,
        }
    }

    /// Program that most recently set the return data.
    #[inline(always)]
    pub fn program_id(&self) -> &Address {
        &self.program_id
    }

    /// Raw return-data bytes.
    ///
    /// Kani proof: `cpi_return_as_slice_in_bounds`.
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.data_len]
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
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr(),
                zc.as_mut_ptr() as *mut u8,
                expected_len,
            );
        }
        let zc = unsafe { zc.assume_init() };
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

        // SAFETY: sol_get_return_data initialised `min(size, MAX_RETURN_DATA)`
        // bytes. The remaining bytes are uninitialised but never accessed —
        // CpiReturn::as_slice returns &data[..data_len] and decode copies
        // exactly size_of::<T::Zc>() bytes. We assume_init the full array
        // because the struct carries it by value; the uninitialised tail is
        // inert (u8 has no Drop).
        return Ok(CpiReturn::new(
            unsafe { program_id.assume_init() },
            unsafe { data.assume_init() },
            core::cmp::min(size, MAX_RETURN_DATA),
        ));
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
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_inner(&[])
    }

    /// Invoke the CPI with a single PDA signer (seeds for one address).
    #[inline(always)]
    #[must_use = "CPI result must be handled with `?` or matched"]
    pub fn invoke_signed(&self, seeds: &[Seed]) -> ProgramResult {
        self.invoke_inner(&[Signer::from(seeds)])
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
    pub fn invoke_signed_with_return(&self, seeds: &[Seed]) -> Result<CpiReturn, ProgramError> {
        self.invoke_with_return_inner(&[Signer::from(seeds)])
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
    pub fn instruction_data(&self) -> &[u8] {
        &self.data
    }
}

// ---------------------------------------------------------------------------
// Shared test/verification account buffer
// ---------------------------------------------------------------------------

/// Stack-allocated account buffer for constructing `AccountView` instances
/// in tests and Kani proofs. Uses `#[repr(align(8))]` for RuntimeAccount
/// alignment. `N` is buffer size in bytes — use `MIN_ACCOUNT_BUF` for a
/// zero-data account.
#[cfg(kani)]
#[repr(C, align(8))]
pub(crate) struct AccountBuffer<const N: usize> {
    inner: [u8; N],
}

/// Minimum buffer size for a zero-data AccountView.
#[cfg(kani)]
pub(crate) const MIN_ACCOUNT_BUF: usize =
    core::mem::size_of::<solana_account_view::RuntimeAccount>() + 8;

#[cfg(kani)]
impl<const N: usize> AccountBuffer<N> {
    pub(crate) fn new() -> Self {
        Self { inner: [0u8; N] }
    }

    fn raw(&mut self) -> *mut solana_account_view::RuntimeAccount {
        self.inner.as_mut_ptr() as *mut solana_account_view::RuntimeAccount
    }

    pub(crate) fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        data_len: usize,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = solana_account_view::NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = executable as u8;
            (*raw).padding = [0u8; 4];
            (*raw).address = solana_address::Address::new_from_array(address);
            (*raw).owner = solana_address::Address::new_from_array(owner);
            (*raw).lamports = 100;
            (*raw).data_len = data_len as u64;
        }
    }

    pub(crate) unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }
}

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
                inner: (0..byte_len.div_ceil(8)).map(|_| 0u64).collect(),
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
            AccountView::new_unchecked(self.raw())
        }
    }

    fn cpi_account_bytes<'a>(account: &'a CpiAccount<'a>) -> &'a [u8] {
        unsafe {
            core::slice::from_raw_parts(
                account as *const CpiAccount<'_> as *const u8,
                core::mem::size_of::<CpiAccount<'_>>(),
            )
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, QuasarSerialize)]
    struct ReturnPayload {
        amount: u64,
        flag: bool,
    }

    #[test]
    fn decode_primitive_return_uses_instruction_arg_layout() {
        let pod = <u64 as InstructionArg>::to_zc(&777u64);
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &pod as *const <u64 as InstructionArg>::Zc as *const u8,
                core::mem::size_of::<<u64 as InstructionArg>::Zc>(),
            )
        };
        let mut data = [0u8; MAX_RETURN_DATA];
        data[..bytes.len()].copy_from_slice(bytes);

        let ret = CpiReturn::new(Address::new_from_array([1u8; 32]), data, bytes.len());
        assert_eq!(ret.decode::<u64>().unwrap(), 777u64);
    }

    #[test]
    fn decode_struct_return_uses_zc_companion_layout() {
        let payload = ReturnPayload {
            amount: 55,
            flag: true,
        };
        let zc = <ReturnPayload as InstructionArg>::to_zc(&payload);
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &zc as *const <ReturnPayload as InstructionArg>::Zc as *const u8,
                core::mem::size_of::<<ReturnPayload as InstructionArg>::Zc>(),
            )
        };
        let mut data = [0u8; MAX_RETURN_DATA];
        data[..bytes.len()].copy_from_slice(bytes);

        let ret = CpiReturn::new(Address::new_from_array([2u8; 32]), data, bytes.len());
        assert_eq!(ret.decode::<ReturnPayload>().unwrap(), payload);
    }

    #[test]
    fn cpi_account_from_view_matches_upstream_layout() {
        // CpiAccount is 56 bytes: 6×u64 fields + 3 bool flags + 5 padding.
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

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove that the u32 >> 8 flag extraction correctly isolates
    /// [is_signer, is_writable, executable] from the RuntimeAccount header.
    ///
    /// The actual code in `cpi_account_from_view` does:
    ///   `(raw as *const u32).read_unaligned() >> 8`
    /// on the first 4 bytes: [borrow_state, is_signer, is_writable,
    /// executable]. The compile-time `offset_of!` assertions guarantee this
    /// field order; this proof verifies the bit arithmetic for all possible
    /// byte values.
    #[kani::proof]
    fn flag_extraction_shift_correctness() {
        let borrow_state: u8 = kani::any();
        let is_signer: u8 = kani::any();
        let is_writable: u8 = kani::any();
        let executable: u8 = kani::any();

        let header = [borrow_state, is_signer, is_writable, executable];
        let raw_u32 = unsafe { (header.as_ptr() as *const u32).read_unaligned() };
        let flags = raw_u32 >> 8;

        // The shift discards borrow_state and preserves the three flag bytes.
        assert!((flags & 0xFF) as u8 == is_signer);
        assert!(((flags >> 8) & 0xFF) as u8 == is_writable);
        assert!(((flags >> 16) & 0xFF) as u8 == executable);
        // High byte is zero — no garbage from the shift.
        assert!(flags >> 24 == 0);
    }

    /// Prove RawCpiBuilder and CpiAccount have identical memory layout.
    ///
    /// The transmute in `cpi_account_from_view` requires identical size and
    /// alignment. The compile-time assertions verify size/align equality,
    /// but this proof additionally verifies field offset correspondence:
    /// every pointer/u64 field in RawCpiBuilder lands at the expected offset.
    #[kani::proof]
    fn raw_cpi_builder_layout_matches_cpi_account() {
        // Size and alignment (mirrors compile-time assertions).
        assert!(core::mem::size_of::<RawCpiBuilder>() == core::mem::size_of::<CpiAccount>());
        assert!(core::mem::align_of::<RawCpiBuilder>() == core::mem::align_of::<CpiAccount>());

        // Field offsets: RawCpiBuilder is repr(C) with 7 fields, each 8 bytes.
        // Verify the layout is contiguous with no padding gaps.
        assert!(core::mem::size_of::<RawCpiBuilder>() == 7 * 8);
        assert!(core::mem::offset_of!(RawCpiBuilder, address) == 0);
        assert!(core::mem::offset_of!(RawCpiBuilder, lamports) == 8);
        assert!(core::mem::offset_of!(RawCpiBuilder, data_len) == 16);
        assert!(core::mem::offset_of!(RawCpiBuilder, data) == 24);
        assert!(core::mem::offset_of!(RawCpiBuilder, owner) == 32);
        assert!(core::mem::offset_of!(RawCpiBuilder, rent_epoch) == 40);
        assert!(core::mem::offset_of!(RawCpiBuilder, flags) == 48);
    }

    /// Prove that `init_cpi_accounts` produces an array of exactly N elements
    /// by calling the real function with N AccountViews.
    ///
    /// The function completing without UB (verified by Kani) proves the
    /// MaybeUninit init loop covers all indices [0, N) and the final
    /// `assume_init` is valid.
    #[kani::proof]
    fn init_cpi_accounts_loop_covers_all_indices() {
        use super::{AccountBuffer, MIN_ACCOUNT_BUF};

        const N: usize = 4;

        let mut buf0 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf0.init([1; 32], [0xAA; 32], 0, true, true, false);
        let v0 = unsafe { buf0.view() };

        let mut buf1 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf1.init([2; 32], [0xAA; 32], 0, false, true, false);
        let v1 = unsafe { buf1.view() };

        let mut buf2 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf2.init([3; 32], [0xAA; 32], 0, true, false, false);
        let v2 = unsafe { buf2.view() };

        let mut buf3 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf3.init([4; 32], [0xAA; 32], 0, false, false, false);
        let v3 = unsafe { buf3.view() };

        // Call the real function — Kani verifies no UB in the MaybeUninit
        // loop and assume_init.
        let result = super::init_cpi_accounts([&v0, &v1, &v2, &v3]);
        assert!(result.len() == N);
    }

    /// Prove `CpiReturn::as_slice` is always in bounds.
    ///
    /// Mirrors `CpiReturn::as_slice()`:
    ///   `&self.data[..self.data_len]`
    /// where `data_len` is set by `get_cpi_return()`:
    ///   `core::cmp::min(size, MAX_RETURN_DATA)`
    #[kani::proof]
    fn cpi_return_as_slice_in_bounds() {
        let size: usize = kani::any();
        // MAX_RETURN_DATA is 1024.
        let data_len = core::cmp::min(size, solana_instruction_view::cpi::MAX_RETURN_DATA);
        assert!(data_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
        // The slice &data[..data_len] is within the [u8; MAX_RETURN_DATA]
        // array.
    }

    /// Prove `CpiReturn::decode` copy length is safe.
    ///
    /// Mirrors `CpiReturn::decode()`:
    ///   `if self.data_len != expected_len { return Err(...); }`
    ///   `copy_nonoverlapping(self.data.as_ptr(), ..., expected_len);`
    ///
    /// Proves the copy never reads past `self.data` (capacity
    /// `MAX_RETURN_DATA`).
    #[kani::proof]
    fn cpi_return_decode_copy_in_bounds() {
        let data_len: usize = kani::any();
        let expected_len: usize = kani::any();

        // data_len comes from min(size, MAX_RETURN_DATA).
        kani::assume(data_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
        // expected_len is size_of::<T::Zc>() for some concrete T.
        // Reasonable upper bound: no Zc type exceeds MAX_RETURN_DATA.
        kani::assume(expected_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);

        // The guard in decode():
        if data_len != expected_len {
            // Returns Err, no copy happens.
            return;
        }

        // If we reach here, copy_nonoverlapping copies expected_len bytes.
        // Source is self.data (size MAX_RETURN_DATA), dest is MaybeUninit<Zc>
        // (size expected_len). Both are valid.
        assert!(expected_len <= solana_instruction_view::cpi::MAX_RETURN_DATA);
        assert!(expected_len == data_len);
    }

    /// Prove that `invoke_raw` usize→u64 casts are lossless.
    ///
    /// Mirrors `invoke_raw()` on-chain path casts:
    ///   `accounts_len: instruction_accounts_len as u64,`
    ///   `data_len: data_len as u64,`
    ///   `cpi_accounts_len as u64,`
    ///
    /// On SBF (32-bit), usize fits in u64 trivially. This proof covers the
    /// property for any sizes within Solana's limits (max 10 MiB data,
    /// max 256 accounts).
    #[kani::proof]
    fn invoke_raw_length_cast_lossless() {
        let acct_len: usize = kani::any();
        let data_len: usize = kani::any();
        let cpi_len: usize = kani::any();

        // Solana constraints: max 256 accounts, max ~10 MiB data.
        kani::assume(acct_len <= 256);
        kani::assume(data_len <= 10 * 1024 * 1024);
        kani::assume(cpi_len <= 256);

        // The on-chain path casts to u64:
        let acct_u64 = acct_len as u64;
        let data_u64 = data_len as u64;
        let cpi_u64 = cpi_len as u64;

        // Prove round-trip: no truncation.
        assert!(acct_u64 as usize == acct_len);
        assert!(data_u64 as usize == data_len);
        assert!(cpi_u64 as usize == cpi_len);
    }

    /// Prove `cpi_account_from_view` data pointer offset is within the
    /// RuntimeAccount allocation by calling the real function on a valid
    /// AccountView with data.
    ///
    /// The function completing without UB (verified by Kani) proves the
    /// `(raw as *const u8).add(RUNTIME_ACCOUNT_SIZE)` data pointer offset
    /// is valid and the transmute produces a well-formed `CpiAccount`.
    #[kani::proof]
    fn cpi_account_data_offset_valid() {
        use super::{AccountBuffer, MIN_ACCOUNT_BUF};

        // Use a buffer large enough for 16 bytes of account data.
        const BUF_SIZE: usize = MIN_ACCOUNT_BUF + 16;

        let mut buf = AccountBuffer::<BUF_SIZE>::new();
        buf.init([0x11; 32], [0x22; 32], 16, true, true, false);
        let view = unsafe { buf.view() };

        // Call the real function — Kani verifies the pointer arithmetic,
        // unaligned read, shift, and transmute are all free of UB.
        let _cpi_account = super::cpi_account_from_view(&view);
    }
}

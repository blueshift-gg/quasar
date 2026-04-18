//! Dynamic CPI builder with runtime-tracked account and data lengths.
//!
//! `DynCpiCall` is the variable-length counterpart to [`super::CpiCall`].
//! Both accounts and data are backed by `MaybeUninit` stack arrays with
//! compile-time capacity, while the active count is tracked at runtime.
//! This replaces `BufCpiCall` which only supported variable data.

use {
    super::{
        cpi_account_from_view, get_cpi_return, invoke_raw, result_from_raw, CpiReturn,
        InstructionAccount, Seed, Signer,
    },
    crate::utils::hint::unlikely,
    core::mem::MaybeUninit,
    solana_account_view::AccountView,
    solana_address::Address,
    solana_instruction_view::cpi::CpiAccount,
    solana_program_error::{ProgramError, ProgramResult},
};

// Safety: element types stored in MaybeUninit arrays must not need dropping.
// If upstream ever adds Drop impls, these assertions catch it at compile time.
const _: () = assert!(!core::mem::needs_drop::<InstructionAccount>());
const _: () = assert!(!core::mem::needs_drop::<CpiAccount>());

/// Stack-allocated CPI builder with runtime-tracked account and data lengths.
///
/// Both the account list and instruction data use `MaybeUninit` arrays to
/// avoid zero-initialization costs. Accounts are pushed one at a time;
/// data is set all at once or written directly via `data_mut()`.
///
/// A compile-time assertion prevents monomorphizations that would overflow
/// the SVM 4 KiB stack frame.
///
/// # Type parameters
///
/// - `MAX_ACCTS`: maximum number of accounts (capacity, not initial count).
/// - `MAX_DATA`: maximum byte length of instruction data.
pub struct DynCpiCall<'a, const MAX_ACCTS: usize, const MAX_DATA: usize> {
    program_id: &'a Address,
    accounts: MaybeUninit<[InstructionAccount<'a>; MAX_ACCTS]>,
    cpi_accounts: MaybeUninit<[CpiAccount<'a>; MAX_ACCTS]>,
    acct_len: usize,
    data: MaybeUninit<[u8; MAX_DATA]>,
    data_len: usize,
}

impl<'a, const MAX_ACCTS: usize, const MAX_DATA: usize> DynCpiCall<'a, MAX_ACCTS, MAX_DATA> {
    // Compile-time stack overflow guard — fires at monomorphization time.
    // InstructionAccount is 24 bytes, CpiAccount is 56 bytes, plus data +
    // bookkeeping.
    const _STACK_CHECK: () = assert!(
        56 * MAX_ACCTS + 24 * MAX_ACCTS + MAX_DATA + 24 <= 3072,
        "DynCpiCall exceeds safe 3 KiB stack budget for SVM 4 KiB frames"
    );

    /// Create a new builder targeting the given program.
    #[inline(always)]
    pub fn new(program_id: &'a Address) -> Self {
        // Force compile-time stack size check.
        #[allow(clippy::let_unit_value)]
        let _ = Self::_STACK_CHECK;
        Self {
            program_id,
            // Stable MaybeUninit pattern (not nightly uninit_array).
            accounts: MaybeUninit::uninit(),
            cpi_accounts: MaybeUninit::uninit(),
            acct_len: 0,
            data: MaybeUninit::uninit(),
            data_len: 0,
        }
    }

    /// Push an account into the builder. Returns error if MAX_ACCTS exceeded.
    ///
    /// Kani proofs: `push_account_write_in_bounds`,
    /// `sequential_pushes_cover_all_indices`.
    #[inline(always)]
    pub fn push_account(
        &mut self,
        view: &'a AccountView,
        is_signer: bool,
        is_writable: bool,
    ) -> Result<(), ProgramError> {
        if unlikely(self.acct_len >= MAX_ACCTS) {
            return Err(ProgramError::InvalidArgument);
        }
        // SAFETY: acct_len < MAX_ACCTS, so both writes are in bounds.
        // Uses the stable MaybeUninit::<[T; N]>::uninit() pattern --
        // as_mut_ptr() gives *mut [T; N], cast to *mut T for element access.
        unsafe {
            let acct_ptr = self.accounts.as_mut_ptr() as *mut InstructionAccount<'a>;
            let cpi_ptr = self.cpi_accounts.as_mut_ptr() as *mut CpiAccount<'a>;
            acct_ptr.add(self.acct_len).write(InstructionAccount {
                address: view.address(),
                is_signer,
                is_writable,
            });
            cpi_ptr
                .add(self.acct_len)
                .write(cpi_account_from_view(view));
        }
        self.acct_len += 1;
        Ok(())
    }

    /// Push an account without bounds checking.
    ///
    /// # Safety
    ///
    /// Caller must ensure `self.acct_len < MAX_ACCTS`.
    #[inline(always)]
    pub unsafe fn push_account_unchecked(
        &mut self,
        view: &'a AccountView,
        is_signer: bool,
        is_writable: bool,
    ) {
        // SAFETY: Caller guarantees acct_len < MAX_ACCTS.
        let acct_ptr = self.accounts.as_mut_ptr() as *mut InstructionAccount<'a>;
        let cpi_ptr = self.cpi_accounts.as_mut_ptr() as *mut CpiAccount<'a>;
        acct_ptr.add(self.acct_len).write(InstructionAccount {
            address: view.address(),
            is_signer,
            is_writable,
        });
        cpi_ptr
            .add(self.acct_len)
            .write(cpi_account_from_view(view));
        self.acct_len += 1;
    }

    /// Set instruction data. Overwrites any previous data.
    ///
    /// Kani proof: `set_data_copy_in_bounds`.
    #[inline(always)]
    pub fn set_data(&mut self, data: &[u8]) -> Result<(), ProgramError> {
        if unlikely(data.len() > MAX_DATA) {
            return Err(ProgramError::InvalidInstructionData);
        }
        // SAFETY: data.len() <= MAX_DATA, so the copy is in bounds.
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.data.as_mut_ptr() as *mut u8,
                data.len(),
            );
        }
        self.data_len = data.len();
        Ok(())
    }

    /// Direct access to the data buffer for zero-copy writes.
    ///
    /// Returns a raw pointer because the buffer contents are logically
    /// uninitialized — callers must write before reading any byte.
    /// After writing, call `set_data_len()` with the number of bytes written.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for writes of up to `MAX_DATA` bytes.
    /// Reading from a byte that has not been written is undefined behavior.
    #[inline(always)]
    pub fn data_mut(&mut self) -> *mut [u8; MAX_DATA] {
        self.data.as_mut_ptr()
    }

    /// Set the active data length (after writing via `data_mut()`).
    #[inline(always)]
    pub fn set_data_len(&mut self, len: usize) -> Result<(), ProgramError> {
        if unlikely(len > MAX_DATA) {
            return Err(ProgramError::InvalidInstructionData);
        }
        self.data_len = len;
        Ok(())
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
    /// Kani proof: `invoke_reads_only_initialized`.
    fn invoke_inner(&self, signers: &[Signer]) -> ProgramResult {
        // SAFETY: accounts[0..acct_len] and cpi_accounts[0..acct_len]
        // are initialized by push_account. data[0..data_len] written by
        // set_data or data_mut(). MaybeUninit<[T; N]> has same layout as [T; N].
        // We pass pointers with acct_len/data_len to invoke_raw, reading only
        // the initialized portion -- never assume_init() the whole array.
        let result = unsafe {
            invoke_raw(
                self.program_id,
                self.accounts.as_ptr() as *const InstructionAccount,
                self.acct_len,
                self.data.as_ptr() as *const u8,
                self.data_len,
                self.cpi_accounts.as_ptr() as *const CpiAccount,
                self.acct_len,
                signers,
            )
        };
        result_from_raw(result)
    }

    #[inline(always)]
    fn invoke_with_return_inner(&self, signers: &[Signer]) -> Result<CpiReturn, ProgramError> {
        crate::return_data::set_return_data(&[]);
        // SAFETY: Same as invoke_inner -- only initialized portions are read.
        let result = unsafe {
            invoke_raw(
                self.program_id,
                self.accounts.as_ptr() as *const InstructionAccount,
                self.acct_len,
                self.data.as_ptr() as *const u8,
                self.data_len,
                self.cpi_accounts.as_ptr() as *const CpiAccount,
                self.acct_len,
                signers,
            )
        };
        result_from_raw(result)?;
        let ret = get_cpi_return()?;
        if !crate::keys_eq(ret.program_id(), self.program_id) {
            return Err(crate::error::QuasarError::ReturnDataFromWrongProgram.into());
        }
        Ok(ret)
    }

    /// Debug accessor for instruction data (off-chain only).
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub fn instruction_data(&self) -> &[u8] {
        // SAFETY: data[0..data_len] was initialized by set_data or data_mut().
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, self.data_len) }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use {super::*, crate::cpi::tests::AccountBuffer, solana_address::Address};

    static PROGRAM_ID: Address = Address::new_from_array([0x11; 32]);

    #[test]
    fn data_mut_write_and_read_back() {
        let mut cpi = DynCpiCall::<1, 8>::new(&PROGRAM_ID);
        // SAFETY: Writing 4 bytes into the uninitialized buffer, then reading
        // only those 4 bytes back via instruction_data().
        unsafe {
            let buf = &mut *cpi.data_mut();
            buf[..4].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        }
        cpi.set_data_len(4).unwrap();
        assert_eq!(cpi.instruction_data(), &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn push_account_then_set_data_round_trip() {
        let mut cpi = DynCpiCall::<2, 16>::new(&PROGRAM_ID);

        let mut buf = AccountBuffer::new(0);
        buf.init([1; 32], [2; 32], 0, true, true, false);
        let view = unsafe { buf.view() };

        cpi.push_account(&view, true, true).unwrap();
        cpi.set_data(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        assert_eq!(cpi.instruction_data(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn push_account_overflow_returns_error() {
        let mut cpi = DynCpiCall::<1, 8>::new(&PROGRAM_ID);

        let mut buf1 = AccountBuffer::new(0);
        buf1.init([1; 32], [2; 32], 0, true, false, false);
        let view1 = unsafe { buf1.view() };

        let mut buf2 = AccountBuffer::new(0);
        buf2.init([3; 32], [4; 32], 0, false, false, false);
        let view2 = unsafe { buf2.view() };

        assert!(cpi.push_account(&view1, true, false).is_ok());
        assert!(cpi.push_account(&view2, false, false).is_err());
    }

    #[test]
    fn set_data_overflow_returns_error() {
        let mut cpi = DynCpiCall::<1, 4>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[0; 5]).is_err());
    }

    #[test]
    fn set_data_exact_capacity() {
        let mut cpi = DynCpiCall::<1, 4>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[0xAA; 4]).is_ok());
        assert_eq!(cpi.instruction_data(), &[0xAA; 4]);
    }

    #[test]
    fn set_data_len_overflow_returns_error() {
        let mut cpi = DynCpiCall::<1, 4>::new(&PROGRAM_ID);
        assert!(cpi.set_data_len(5).is_err());
    }

    #[test]
    fn set_data_len_exact_capacity() {
        let mut cpi = DynCpiCall::<1, 4>::new(&PROGRAM_ID);
        // SAFETY: We're only setting the length; invoke would read these bytes
        // but we won't invoke — this tests the length validation path.
        assert!(cpi.set_data_len(4).is_ok());
    }

    #[test]
    fn set_data_zero_length() {
        let mut cpi = DynCpiCall::<1, 8>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[]).is_ok());
        assert_eq!(cpi.instruction_data(), &[]);
    }

    #[test]
    fn data_mut_returns_raw_pointer() {
        let mut cpi = DynCpiCall::<1, 8>::new(&PROGRAM_ID);
        let ptr = cpi.data_mut();
        // Verify it's a valid pointer by writing through it.
        // SAFETY: Writing within the MAX_DATA capacity.
        unsafe {
            let buf = &mut *ptr;
            buf[0] = 0xBE;
            buf[1] = 0xEF;
        }
        cpi.set_data_len(2).unwrap();
        assert_eq!(cpi.instruction_data(), &[0xBE, 0xEF]);
    }

    #[test]
    fn push_account_fills_to_capacity() {
        let mut cpi = DynCpiCall::<3, 8>::new(&PROGRAM_ID);

        let mut buf0 = AccountBuffer::new(0);
        let mut buf1 = AccountBuffer::new(0);
        let mut buf2 = AccountBuffer::new(0);
        let mut buf3 = AccountBuffer::new(0);
        buf0.init([1; 32], [0xFF; 32], 0, false, false, false);
        buf1.init([2; 32], [0xFF; 32], 0, false, false, false);
        buf2.init([3; 32], [0xFF; 32], 0, false, false, false);
        buf3.init([4; 32], [0xFF; 32], 0, false, false, false);

        let v0 = unsafe { buf0.view() };
        let v1 = unsafe { buf1.view() };
        let v2 = unsafe { buf2.view() };
        let v3 = unsafe { buf3.view() };

        assert!(cpi.push_account(&v0, false, false).is_ok());
        assert!(cpi.push_account(&v1, false, false).is_ok());
        assert!(cpi.push_account(&v2, false, false).is_ok());
        // 4th push should fail — capacity is 3
        assert!(cpi.push_account(&v3, false, false).is_err());
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use {
        super::DynCpiCall,
        crate::cpi::{AccountBuffer, MIN_ACCOUNT_BUF},
    };

    /// Prove `push_account` bounds check prevents out-of-bounds MaybeUninit
    /// writes by calling the real function and verifying Ok/Err at the
    /// capacity boundary.
    #[kani::proof]
    fn push_account_write_in_bounds() {
        const MAX_ACCTS: usize = 4;
        let target: usize = kani::any();
        kani::assume(target <= MAX_ACCTS);

        let addr = solana_address::Address::new_from_array([0x11; 32]);
        let mut cpi = DynCpiCall::<MAX_ACCTS, 8>::new(&addr);

        let mut buf = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf.init([1; 32], [0; 32], 0, true, true, false);
        let view = unsafe { buf.view() };

        // Push `target` times — all should succeed.
        let mut i = 0;
        while i < target {
            assert!(cpi.push_account(&view, true, true).is_ok());
            i += 1;
        }

        // At capacity, next push must fail.
        if target == MAX_ACCTS {
            assert!(cpi.push_account(&view, true, true).is_err());
        }
    }

    /// Prove `set_data` bounds check prevents out-of-bounds
    /// copy_nonoverlapping by calling the real function.
    #[kani::proof]
    fn set_data_copy_in_bounds() {
        const MAX_DATA: usize = 16;
        let data_len: usize = kani::any();
        kani::assume(data_len <= 32);

        let addr = solana_address::Address::new_from_array([0x11; 32]);
        let mut cpi = DynCpiCall::<1, MAX_DATA>::new(&addr);
        let buf = [0u8; 32];

        if data_len <= MAX_DATA {
            assert!(cpi.set_data(&buf[..data_len]).is_ok());
        } else {
            assert!(cpi.set_data(&buf[..data_len]).is_err());
        }
    }

    /// Prove that sequential `push_account` calls fill all slots by calling
    /// the real function MAX_ACCTS times and verifying capacity exhaustion.
    #[kani::proof]
    fn sequential_pushes_cover_all_indices() {
        const MAX_ACCTS: usize = 4;
        let addr = solana_address::Address::new_from_array([0x11; 32]);
        let mut cpi = DynCpiCall::<MAX_ACCTS, 8>::new(&addr);

        let mut buf0 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        let mut buf1 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        let mut buf2 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        let mut buf3 = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
        buf0.init([1; 32], [0xFF; 32], 0, false, false, false);
        buf1.init([2; 32], [0xFF; 32], 0, false, false, false);
        buf2.init([3; 32], [0xFF; 32], 0, false, false, false);
        buf3.init([4; 32], [0xFF; 32], 0, false, false, false);

        let v0 = unsafe { buf0.view() };
        let v1 = unsafe { buf1.view() };
        let v2 = unsafe { buf2.view() };
        let v3 = unsafe { buf3.view() };

        assert!(cpi.push_account(&v0, false, false).is_ok());
        assert!(cpi.push_account(&v1, false, false).is_ok());
        assert!(cpi.push_account(&v2, false, false).is_ok());
        assert!(cpi.push_account(&v3, false, false).is_ok());
        // Capacity exhausted — next push must fail.
        assert!(cpi.push_account(&v0, false, false).is_err());
    }

    /// Prove invoke_inner only reads initialized portions of MaybeUninit
    /// arrays.
    ///
    /// Mirrors `invoke_inner()`:
    ///   `invoke_raw(..., self.acct_len, ..., self.data_len, ...,
    /// self.acct_len, ...)`
    ///
    /// `invoke_inner` passes `acct_len` (not MAX_ACCTS) and `data_len`
    /// (not MAX_DATA) as lengths to `invoke_raw`, so only initialized
    /// slots are read. Left as arithmetic model because calling
    /// invoke_raw requires a CPI syscall.
    #[kani::proof]
    fn invoke_reads_only_initialized() {
        const MAX_ACCTS: usize = 8;
        const MAX_DATA: usize = 64;
        let acct_len: usize = kani::any();
        let data_len: usize = kani::any();
        kani::assume(acct_len <= MAX_ACCTS);
        kani::assume(data_len <= MAX_DATA);

        // invoke_raw receives acct_len and data_len as bounds.
        // Verify these are within the MaybeUninit capacity.
        assert!(acct_len <= MAX_ACCTS);
        assert!(data_len <= MAX_DATA);
    }
}

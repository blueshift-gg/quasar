//! Dynamic CPI builder with runtime-tracked account and data lengths.
//!
//! `CpiDynamic` is the variable-length counterpart to [`super::CpiCall`].
//! Both accounts and data are backed by `MaybeUninit` stack arrays with
//! compile-time capacity, while the active count is tracked at runtime.

use {
    super::{
        cpi_account_from_view, get_cpi_return, invoke_raw, result_from_raw, CpiReturn,
        CpiSignerSeeds, InstructionAccount, Signer,
    },
    crate::utils::hint::unlikely,
    core::mem::MaybeUninit,
    solana_account_view::AccountView,
    solana_address::Address,
    solana_instruction_view::cpi::CpiAccount,
    solana_program_error::{ProgramError, ProgramResult},
};

// SAFETY: element types stored in MaybeUninit arrays must not need dropping.
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
pub struct CpiDynamic<'a, const MAX_ACCTS: usize, const MAX_DATA: usize> {
    program_id: &'a Address,
    accounts: MaybeUninit<[InstructionAccount<'a>; MAX_ACCTS]>,
    cpi_accounts: MaybeUninit<[CpiAccount<'a>; MAX_ACCTS]>,
    acct_len: usize,
    data: MaybeUninit<[u8; MAX_DATA]>,
    data_len: usize,
}

impl<'a, const MAX_ACCTS: usize, const MAX_DATA: usize> CpiDynamic<'a, MAX_ACCTS, MAX_DATA> {
    // Compile-time stack overflow guard; fires at monomorphization time.
    // InstructionAccount is 24 bytes, CpiAccount is 56 bytes, plus data +
    // bookkeeping. The whole builder lives on the caller's frame while it
    // is filled and invoked, so 1 KiB of the frame is reserved for the
    // rest of the caller and the invoke path.
    const _STACK_CHECK: () = assert!(
        56 * MAX_ACCTS + 24 * MAX_ACCTS + MAX_DATA + 24
            <= crate::__internal::SBF_STACK_FRAME - 1024,
        "CpiDynamic exceeds the sBPF stack budget (frame minus 1 KiB headroom)"
    );

    /// Create a new builder targeting the given program.
    ///
    /// Includes a compile-time assertion that the monomorphized struct fits
    /// within the SVM 3 KiB safe stack budget. Use
    /// [`new_unchecked`](Self::new_unchecked) to bypass this check when you
    /// manage stack usage yourself.
    #[inline(always)]
    pub fn new(program_id: &'a Address) -> Self {
        // Force compile-time stack size check.
        #[allow(clippy::let_unit_value)]
        let _ = Self::_STACK_CHECK;
        Self {
            program_id,
            accounts: MaybeUninit::uninit(),
            cpi_accounts: MaybeUninit::uninit(),
            acct_len: 0,
            data: MaybeUninit::uninit(),
            data_len: 0,
        }
    }

    /// Create a new builder, skipping the compile-time stack-budget check.
    ///
    /// Safe despite the `_unchecked` suffix: the only thing bypassed is the
    /// static stack-budget assertion of [`new`](Self::new) — never a memory or
    /// bounds check. Behavior is otherwise identical to [`new`](Self::new).
    ///
    /// Use it when the call site has verified stack headroom, e.g. this CPI is
    /// the frame's only large allocation.
    #[inline(always)]
    pub fn new_unchecked(program_id: &'a Address) -> Self {
        Self {
            program_id,
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
        unsafe {
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
    /// uninitialized; callers must write before reading any byte.
    /// After writing, call unsafe [`set_data_len`](Self::set_data_len) with
    /// the number of bytes written.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for writes of up to `MAX_DATA` bytes.
    /// Reading from a byte that has not been written is undefined behavior.
    #[inline(always)]
    pub fn data_mut(&mut self) -> *mut [u8; MAX_DATA] {
        self.data.as_mut_ptr()
    }

    /// Set the active data length after writing via
    /// [`data_mut`](Self::data_mut).
    ///
    /// # Safety
    ///
    /// The caller must guarantee that every byte in `data[0..len]` has been
    /// initialized. CPI invocation and
    /// [`instruction_data`](Self::instruction_data) read exactly that byte
    /// range.
    #[inline(always)]
    pub unsafe fn set_data_len(&mut self, len: usize) -> Result<(), ProgramError> {
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

    /// Kani proof: `invoke_reads_only_initialized`.
    #[inline(always)]
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
        let mut cpi = CpiDynamic::<1, 8>::new(&PROGRAM_ID);
        // SAFETY: Writing 4 bytes into the uninitialized buffer, then reading
        // only those 4 bytes back via instruction_data().
        unsafe {
            let buf = &mut *cpi.data_mut();
            buf[..4].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        }
        // SAFETY: The first four bytes were written above.
        unsafe { cpi.set_data_len(4) }.unwrap();
        assert_eq!(cpi.instruction_data(), &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn push_account_then_set_data_round_trip() {
        let mut cpi = CpiDynamic::<2, 16>::new(&PROGRAM_ID);

        let mut buf = AccountBuffer::new(0);
        buf.init([1; 32], [2; 32], 0, true, true, false);
        let view = unsafe { buf.view() };

        cpi.push_account(&view, true, true).unwrap();
        cpi.set_data(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        assert_eq!(cpi.instruction_data(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn push_account_overflow_returns_error() {
        let mut cpi = CpiDynamic::<1, 8>::new(&PROGRAM_ID);

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
        let mut cpi = CpiDynamic::<1, 4>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[0; 5]).is_err());
    }

    #[test]
    fn set_data_exact_capacity() {
        let mut cpi = CpiDynamic::<1, 4>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[0xAA; 4]).is_ok());
        assert_eq!(cpi.instruction_data(), &[0xAA; 4]);
    }

    #[test]
    fn set_data_len_overflow_returns_error() {
        let mut cpi = CpiDynamic::<1, 4>::new(&PROGRAM_ID);
        // SAFETY: The call must fail before any byte can become active.
        assert!(unsafe { cpi.set_data_len(5) }.is_err());
    }

    #[test]
    fn set_data_len_exact_capacity() {
        let mut cpi = CpiDynamic::<1, 4>::new(&PROGRAM_ID);
        unsafe {
            let buf = &mut *cpi.data_mut();
            buf.copy_from_slice(&[0xAA; 4]);
        }
        // SAFETY: All four bytes were written above.
        assert!(unsafe { cpi.set_data_len(4) }.is_ok());
    }

    #[test]
    fn set_data_zero_length() {
        let mut cpi = CpiDynamic::<1, 8>::new(&PROGRAM_ID);
        assert!(cpi.set_data(&[]).is_ok());
        assert_eq!(cpi.instruction_data(), &[] as &[u8]);
    }

    #[test]
    fn data_mut_returns_raw_pointer() {
        let mut cpi = CpiDynamic::<1, 8>::new(&PROGRAM_ID);
        let ptr = cpi.data_mut();
        // SAFETY: Writing within the MAX_DATA capacity.
        unsafe {
            let buf = &mut *ptr;
            buf[0] = 0xBE;
            buf[1] = 0xEF;
        }
        // SAFETY: The first two bytes were written above.
        unsafe { cpi.set_data_len(2) }.unwrap();
        assert_eq!(cpi.instruction_data(), &[0xBE, 0xEF]);
    }

    #[test]
    fn push_account_fills_to_capacity() {
        let mut cpi = CpiDynamic::<3, 8>::new(&PROGRAM_ID);

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
        assert!(cpi.push_account(&v3, false, false).is_err());
    }
}

#[cfg(kani)]
#[path = "../../kani/cpi/dyn_cpi.rs"]
mod kani_proofs;

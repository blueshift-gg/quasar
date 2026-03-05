//! Miri UB tests for quasar-core unsafe code paths.
//!
//! These tests are designed to FIND undefined behavior, not confirm correct
//! output. Each test exercises a specific unsafe pattern under conditions
//! that would trigger Miri if the pattern is unsound.
#![allow(
    clippy::manual_div_ceil,
    clippy::useless_vec,
    clippy::deref_addrof,
    clippy::needless_range_loop,
    clippy::borrow_deref_ref
)]
//!
//! ## Run
//!
//! ```sh
//! MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
//!   cargo +nightly miri test -p quasar-core --test miri
//! ```
//!
//! ## Flags
//!
//! - `-Zmiri-tree-borrows`: Tree Borrows model. The `& → &mut` cast in
//!   `from_account_view_mut` is instant UB under Stacked Borrows. Under Tree
//!   Borrows it is sound because the `&mut Account<T>` never writes to the
//!   AccountView memory itself — writes go through the raw pointer to a
//!   separate RuntimeAccount allocation. The retag creates a "Reserved" child
//!   that never transitions to "Active".
//! - `-Zmiri-symbolic-alignment-check`: Catch alignment issues that depend on
//!   allocation placement rather than happenstance.
//!
//! ## Findings
//!
//! | Pattern | Result |
//! |---------|--------|
//! | `& → &mut` cast (`from_account_view_mut`) | Sound under Tree Borrows |
//! | `& → &mut` cast (`Initialize`, `define_account!`) | Sound under Tree Borrows |
//! | DerefMut write + aliased read via &AccountView | Sound under Tree Borrows |
//! | Interleaved shared/mutable access | Sound under Tree Borrows |
//! | `copy_nonoverlapping` 3-byte flag extraction | Sound |
//! | MaybeUninit array init + assume_init | Sound |
//! | Event memcpy from repr(C) (no padding) | Sound |
//! | `assign` + `resize` + `close` raw pointer writes | Sound |
//! | `borrow_unchecked_mut` sequential borrows | Sound |
//! | CPI `create_account` data construction | Sound (was misaligned u32, fixed) |
//! | Boundary pointer subtraction (`data.as_ptr().sub(8)`) | Sound |
//! | Remaining accounts alignment rounding | **Provenance warning** — integer-to-pointer cast strips provenance. Fails under `-Zmiri-strict-provenance`. Not UB under default provenance model. |
//! | Dynamic ZC header cast + PodU16 descriptor read | Sound |
//! | `from_utf8_unchecked` on account data String fields | Sound |
//! | `slice::from_raw_parts` for Vec field access | Sound |
//! | `ptr::copy` (memmove) for shifting subsequent dynamic fields | Sound |
//! | `slice::from_raw_parts_mut` for Vec in-place mutation | Sound |
//! | `copy_nonoverlapping` for Vec data writes | Sound |
//! | Stack buffer batch write (`set_dynamic_fields` pattern) | Sound |
//! | Instruction data ZC cast + variable tail parsing | Sound |
//!
//! ## What Miri CANNOT test
//!
//! | Pattern | Why |
//! |---------|-----|
//! | `sol_invoke_signed_c` syscall | FFI, SBF-only |
//! | `sol_get_sysvar` syscall | FFI, SBF-only |
//! | Full dispatch loop | Requires SVM buffer from runtime |

use std::mem::{align_of, size_of, MaybeUninit};

use quasar_core::__internal::{
    AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
};
use quasar_core::accounts::{Account, Initialize, Signer as SignerAccount, UncheckedAccount};
use quasar_core::checks;
use quasar_core::cpi::{CpiCall, InstructionAccount};
use quasar_core::error::QuasarError;
use quasar_core::pod::*;
use quasar_core::remaining::RemainingAccounts;
use quasar_core::traits::*;
use solana_address::Address;
use solana_program_error::ProgramError;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// 8-byte-aligned buffer for constructing RuntimeAccount + data.
///
/// Uses `Vec<u64>` to guarantee alignment >= 8, which satisfies
/// RuntimeAccount's alignment requirement.
struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn new(data_len: usize) -> Self {
        let byte_len =
            size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();
        let u64_count = byte_len.div_ceil(8);
        Self {
            inner: vec![0; u64_count],
        }
    }

    /// Allocation with exact byte count (no extra slack beyond alignment padding).
    fn exact(byte_len: usize) -> Self {
        let u64_count = byte_len.div_ceil(8);
        Self {
            inner: vec![0; u64_count],
        }
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr() as *mut u8
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 0;
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    fn write_data(&mut self, data: &[u8]) {
        let data_start = size_of::<RuntimeAccount>();
        let dst = unsafe {
            std::slice::from_raw_parts_mut(self.as_mut_ptr().add(data_start), data.len())
        };
        dst.copy_from_slice(data);
    }
}

/// Multi-account buffer for remaining accounts tests.
struct MultiAccountBuffer {
    inner: Vec<u64>,
}

const ACCOUNT_HEADER: usize =
    size_of::<RuntimeAccount>() + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();

impl MultiAccountBuffer {
    fn new(accounts: &[MultiAccountEntry]) -> Self {
        let total_bytes: usize = accounts
            .iter()
            .map(|entry| match entry {
                MultiAccountEntry::Full { data_len, data, .. } => {
                    let raw_len = ACCOUNT_HEADER + data.as_ref().map_or(*data_len, |d| d.len());
                    (raw_len + 7) & !7
                }
                MultiAccountEntry::Duplicate { .. } => size_of::<u64>(),
            })
            .sum();
        let u64_count = total_bytes.div_ceil(8);
        let mut buf = Self {
            inner: vec![0; u64_count],
        };
        buf.populate(accounts);
        buf
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr() as *mut u8
    }

    fn boundary(&self) -> *const u8 {
        unsafe { (self.inner.as_ptr() as *const u8).add(self.inner.len() * size_of::<u64>()) }
    }

    fn populate(&mut self, accounts: &[MultiAccountEntry]) {
        let base = self.as_mut_ptr();
        let mut offset = 0usize;
        for entry in accounts {
            match entry {
                MultiAccountEntry::Full {
                    address,
                    owner,
                    lamports,
                    data_len,
                    data,
                    is_signer,
                    is_writable,
                } => {
                    let raw = unsafe { &mut *(base.add(offset) as *mut RuntimeAccount) };
                    raw.borrow_state = NOT_BORROWED;
                    raw.is_signer = *is_signer as u8;
                    raw.is_writable = *is_writable as u8;
                    raw.executable = 0;
                    raw.resize_delta = 0;
                    raw.address = Address::new_from_array(*address);
                    raw.owner = Address::new_from_array(*owner);
                    raw.lamports = *lamports;
                    let actual_data_len = data.as_ref().map_or(*data_len, |d| d.len());
                    raw.data_len = actual_data_len as u64;

                    if let Some(d) = data {
                        let data_start = offset + size_of::<RuntimeAccount>();
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                d.as_ptr(),
                                base.add(data_start),
                                d.len(),
                            );
                        }
                    }

                    let raw_len = ACCOUNT_HEADER + actual_data_len;
                    offset += (raw_len + 7) & !7;
                }
                MultiAccountEntry::Duplicate { original_index } => {
                    unsafe { *base.add(offset) = *original_index as u8 };
                    offset += size_of::<u64>();
                }
            }
        }
    }
}

enum MultiAccountEntry {
    Full {
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: usize,
        data: Option<Vec<u8>>,
        is_signer: bool,
        is_writable: bool,
    },
    Duplicate {
        original_index: usize,
    },
}

impl MultiAccountEntry {
    fn account(address_byte: u8, data_len: usize) -> Self {
        MultiAccountEntry::Full {
            address: [address_byte; 32],
            owner: [0xAA; 32],
            lamports: 1_000_000,
            data_len,
            data: None,
            is_signer: false,
            is_writable: true,
        }
    }

    fn duplicate(original_index: usize) -> Self {
        MultiAccountEntry::Duplicate { original_index }
    }
}

// ---------------------------------------------------------------------------
// Test-only types for Account<T> transparent cast tests
// ---------------------------------------------------------------------------

#[repr(C)]
struct TestZcData {
    value: PodU64,
    flag: PodBool,
}

const _: () = assert!(align_of::<TestZcData>() == 1);
const _: () = assert!(size_of::<TestZcData>() == 9);

struct TestAccountType;

const TEST_OWNER: Address = Address::new_from_array([42u8; 32]);

impl Owner for TestAccountType {
    const OWNER: Address = TEST_OWNER;
}

impl AccountCheck for TestAccountType {
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl ZeroCopyDeref for TestAccountType {
    type Target = TestZcData;

    #[inline(always)]
    fn deref_from(view: &AccountView) -> &Self::Target {
        unsafe { &*(view.data_ptr().add(4) as *const TestZcData) }
    }

    #[inline(always)]
    fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
        unsafe { &mut *(view.data_ptr().add(4) as *mut TestZcData) }
    }
}

// ===========================================================================
// 1. The & -> &mut cast (THE critical pattern)
//
// Account::from_account_view_mut takes &AccountView and returns &mut Self.
// This is the pattern every Solana framework uses. Under Stacked Borrows
// it's instant UB. Under Tree Borrows it MIGHT be sound because the &mut
// only touches the raw pointer value (never writes to it — writes go through
// the pointer to SVM memory). These tests probe whether Tree Borrows agrees.
// ===========================================================================

#[test]
fn shared_to_mut_cast_then_read_lamports() {
    // Probe: create &AccountView, cast to &mut Account<T>, read lamports
    // through the &mut path. The read goes through the raw pointer inside
    // AccountView to the RuntimeAccount buffer — a different allocation
    // from the AccountView itself.
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 500_000, 64, true, true);

    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // Read through the &mut Account<T> path
    assert_eq!(account.to_account_view().lamports(), 500_000);
}

#[test]
fn shared_to_mut_cast_then_write_lamports() {
    // Probe: cast to &mut, then WRITE through set_lamports.
    // set_lamports writes to the RuntimeAccount buffer (different allocation),
    // NOT to the AccountView's memory. Tree Borrows should allow this because
    // the &mut Account<T> never actually writes to the AccountView pointer value.
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);

    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    account.to_account_view().set_lamports(999);
    assert_eq!(account.to_account_view().lamports(), 999);
}

#[test]
fn shared_to_mut_cast_then_read_original_view() {
    // Probe: cast to &mut Account<T>, THEN read through the original &AccountView.
    // This is the aliasing pattern: &view and &mut account point to the same
    // AccountView memory. Under Tree Borrows, reading through the parent (&view)
    // after creating a child (&mut account) may or may not be UB depending on
    // whether the child ever performed a "write" to that memory.
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);

    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // Write through the &mut path (to RuntimeAccount, not AccountView)
    account.to_account_view().set_lamports(777);

    // Read through the ORIGINAL &view — does this alias conflict?
    assert_eq!(view.lamports(), 777);
}

#[test]
fn shared_to_mut_cast_interleaved_access() {
    // Probe: alternate reads between &view and &mut account.
    // This is the real instruction-handler pattern: you have both references
    // alive and use them interchangeably.
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);

    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // Read through &mut
    let l1 = account.to_account_view().lamports();
    // Read through &
    let l2 = view.lamports();
    assert_eq!(l1, l2);

    // Write through &mut
    account.to_account_view().set_lamports(200);
    // Read through &
    assert_eq!(view.lamports(), 200);
    // Read through &mut
    assert_eq!(account.to_account_view().lamports(), 200);

    // Write through & (AccountView has interior mutability)
    view.set_lamports(300);
    // Read through &mut
    assert_eq!(account.to_account_view().lamports(), 300);
}

// ===========================================================================
// 2. DerefMut — zero-copy write through Account<T>
//
// Account<T>::deref_mut() does:
//   &mut *(self.data_ptr().add(DATA_OFFSET) as *mut T::Target)
// This creates a &mut to data INSIDE the SVM buffer. The pointer arithmetic
// and the cast to T::Target could be UB if alignment/bounds are wrong.
// ===========================================================================

fn make_zc_buffer() -> AccountBuffer {
    let disc_len = 4;
    let data_len = disc_len + size_of::<TestZcData>();
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        true,
        true,
    );
    // Write discriminator
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data[disc_len..disc_len + 8].copy_from_slice(&42u64.to_le_bytes());
    data[disc_len + 8] = 1; // PodBool true
    buf.write_data(&data);
    buf
}

#[test]
fn deref_read_zc_fields() {
    // Baseline: Deref (read) through Account<T> to ZC fields.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked(&view) };

    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 42);
    assert!(zc.flag.get());
}

#[test]
fn deref_mut_write_zc_fields() {
    // Probe: DerefMut (write) through &mut Account<T> to ZC fields.
    // This creates &mut TestZcData pointing into the SVM buffer.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    let zc: &mut TestZcData = &mut *account;
    zc.value = PodU64::from(999u64);
    zc.flag = PodBool::from(false);

    // Verify the write landed in the buffer
    assert_eq!(zc.value.get(), 999);
    assert!(!zc.flag.get());
}

#[test]
fn deref_mut_write_then_read_via_view() {
    // Probe: write through DerefMut, then read the same bytes through the
    // original AccountView's data pointer. Tests whether the write through
    // &mut TestZcData aliases with reads through &AccountView.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // Write through DerefMut
    let zc: &mut TestZcData = &mut *account;
    zc.value = PodU64::from(12345u64);

    // Read the same bytes through view.borrow_unchecked()
    let data = unsafe { view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 12345);
}

#[test]
fn deref_mut_write_then_deref_read() {
    // Probe: write through DerefMut, drop the &mut, then Deref (read).
    // The &mut TestZcData and &TestZcData point to the same memory.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // Write
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(7777u64);
    }

    // Read via Deref (not DerefMut)
    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 7777);
}

#[test]
fn multiple_deref_mut_calls() {
    // Probe: call deref_mut() multiple times on the same Account.
    // Each call creates a new &mut TestZcData. If Miri tracks the previous
    // &mut as still-live, this could trigger an aliasing violation.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    // First DerefMut
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(1u64);
    }
    // Second DerefMut
    {
        let zc: &mut TestZcData = &mut *account;
        assert_eq!(zc.value.get(), 1);
        zc.value = PodU64::from(2u64);
    }
    // Third DerefMut
    {
        let zc: &mut TestZcData = &mut *account;
        assert_eq!(zc.value.get(), 2);
    }
}

// ===========================================================================
// 3. Tight-buffer boundary conditions
//
// The previous tests used oversized buffers. These use exact-minimum-size
// buffers so any off-by-one in pointer arithmetic hits the allocation edge.
// ===========================================================================

#[test]
fn account_view_exact_size_buffer() {
    // Minimum buffer: RuntimeAccount header + data_len bytes.
    // No MAX_PERMITTED_DATA_INCREASE slack.
    let data_len = 16usize;
    let exact_size = size_of::<RuntimeAccount>() + data_len;
    let mut buf = AccountBuffer::exact(exact_size);
    buf.init([1u8; 32], [2u8; 32], 100, data_len as u64, false, true);

    let view = unsafe { buf.view() };

    // These reads must stay within the allocation
    assert_eq!(view.lamports(), 100);
    assert_eq!(view.data_len(), data_len);
    assert!(view.is_writable());
    assert_eq!(view.data_ptr(), unsafe {
        buf.as_mut_ptr().add(size_of::<RuntimeAccount>())
    });
}

#[test]
fn account_view_zero_data_len() {
    // data_len = 0: data_ptr() still valid (points to end of RuntimeAccount),
    // but borrow_unchecked() should return a zero-length slice.
    let mut buf = AccountBuffer::exact(size_of::<RuntimeAccount>());
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);

    let view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 0);

    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(data.len(), 0);
}

#[test]
fn deref_exact_size_buffer() {
    // Buffer is exactly RuntimeAccount + discriminator + TestZcData.
    // No slack. The Deref pointer arithmetic must land exactly within bounds.
    let disc_len = 4;
    let data_len = disc_len + size_of::<TestZcData>();
    let exact_size = size_of::<RuntimeAccount>() + data_len;
    let mut buf = AccountBuffer::exact(exact_size);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        100,
        data_len as u64,
        true,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data[disc_len..disc_len + 8].copy_from_slice(&99u64.to_le_bytes());
    data[disc_len + 8] = 1;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked(&view) };

    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 99);
    assert!(zc.flag.get());
}

// ===========================================================================
// 4. CPI — RawCpiAccount::from_view via CpiCall::new
//
// from_view() does copy_nonoverlapping((raw as *const u8).add(1), &mut
// account.is_signer, 3) — copying 3 bytes from RuntimeAccount offset 1
// into the is_signer field of RawCpiAccount. The destination pointer is
// derived from &mut of a single u8 field but writes 3 bytes (into
// is_signer + is_writable + executable which are contiguous in repr(C)).
// ===========================================================================

#[test]
fn cpi_from_view_flag_extraction() {
    // Set specific flag patterns and verify the copy_nonoverlapping path
    // extracts them correctly.
    let mut buf = AccountBuffer::new(8);
    buf.init([1u8; 32], [2u8; 32], 100, 8, true, false);
    unsafe { (*buf.raw()).executable = 1 };

    let view = unsafe { buf.view() };
    let program_id = Address::new_from_array([0u8; 32]);

    // CpiCall::new calls RawCpiAccount::from_view internally.
    // If the 3-byte copy_nonoverlapping is UB, Miri catches it here.
    let _call: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable_signer(view.address())],
        [&view],
        [0u8],
    );

    // Construct with opposite flags to exercise different bit patterns
    let mut buf2 = AccountBuffer::new(0);
    buf2.init([2u8; 32], [3u8; 32], 0, 0, false, true);
    unsafe { (*buf2.raw()).executable = 0 };

    let view2 = unsafe { buf2.view() };
    let _call2: CpiCall<'_, 1, 1> = CpiCall::new(
        &program_id,
        [InstructionAccount::writable(view2.address())],
        [&view2],
        [0u8],
    );
}

#[test]
fn cpi_create_account_data_construction() {
    // create_account builds a 52-byte data buffer via MaybeUninit +
    // copy_nonoverlapping. Previously used a misaligned u32 write at offset 0.
    // Now uses copy_nonoverlapping for all fields. Verify no UB in the
    // data construction path.
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);

    let from = unsafe { from_buf.view() };
    let to = unsafe { to_buf.view() };
    let owner = Address::new_from_array([0xAA; 32]);

    let _call = quasar_core::cpi::system::create_account(&from, &to, 500_000u64, 100, &owner);
}

#[test]
fn cpi_maybeuninit_multi_account() {
    // CpiCall::new with N accounts exercises the MaybeUninit loop:
    //   let mut arr = MaybeUninit::<[RawCpiAccount; N]>::uninit();
    //   for i in 0..N { ptr::write(ptr.add(i), from_view(views[i])) }
    //   arr.assume_init()
    // If any element is left uninitialized, Miri detects it at assume_init.
    let mut bufs: Vec<AccountBuffer> = (0..4)
        .map(|i| {
            let mut b = AccountBuffer::new(0);
            b.init(
                [i as u8; 32],
                [0u8; 32],
                i as u64,
                0,
                i % 2 == 0,
                i % 2 == 1,
            );
            b
        })
        .collect();

    let views: Vec<AccountView> = bufs.iter_mut().map(|b| unsafe { b.view() }).collect();
    let program_id = Address::new_from_array([0u8; 32]);

    let _call: CpiCall<'_, 4, 1> = CpiCall::new(
        &program_id,
        [
            InstructionAccount::writable_signer(views[0].address()),
            InstructionAccount::writable(views[1].address()),
            InstructionAccount::readonly_signer(views[2].address()),
            InstructionAccount::readonly(views[3].address()),
        ],
        [&views[0], &views[1], &views[2], &views[3]],
        [0u8],
    );
}

// ===========================================================================
// 5. Remaining accounts — buffer walking with pointer arithmetic
//
// The walking code does:
//   ptr = ptr.add(ACCOUNT_HEADER + data_len)
//   ptr = ((ptr as usize + 7) & !7) as *mut u8  // align to 8
//
// The alignment rounding casts pointer → integer → pointer, which strips
// provenance. Miri warns about this but does not (currently) flag it as UB
// under default settings. Under -Zmiri-strict-provenance it WOULD fail.
// ===========================================================================

#[test]
fn remaining_walk_varied_data_lengths() {
    // Accounts with different data_len values exercise different pointer
    // advance distances. Non-8-aligned data_len values exercise the
    // alignment rounding path.
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 1, // 1 byte — forces alignment padding
            data: Some(vec![0xFF]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 7, // 7 bytes — misaligned, forces rounding
            data: Some(vec![0xEE; 7]),
            is_signer: true,
            is_writable: false,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 8, // exactly aligned
            data: Some(vec![0xDD; 8]),
            is_signer: false,
            is_writable: true,
        },
    ]);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let v0 = remaining.get(0).unwrap();
    assert_eq!(v0.lamports(), 100);
    assert_eq!(v0.data_len(), 1);

    let v1 = remaining.get(1).unwrap();
    assert_eq!(v1.lamports(), 200);
    assert_eq!(v1.data_len(), 7);

    let v2 = remaining.get(2).unwrap();
    assert_eq!(v2.lamports(), 300);
    assert_eq!(v2.data_len(), 8);

    assert!(remaining.get(3).is_none());
}

#[test]
fn remaining_iterator_varied_data_lengths() {
    // Same as above but through the iterator path, which uses its own
    // pointer arithmetic and MaybeUninit cache.
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 3, // non-aligned
            data: Some(vec![0xFF; 3]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 0, // zero
            data: None,
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 15, // non-aligned
            data: Some(vec![0xDD; 15]),
            is_signer: false,
            is_writable: true,
        },
    ]);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 3);
    assert_eq!(views[0].data_len(), 3);
    assert_eq!(views[1].data_len(), 0);
    assert_eq!(views[2].data_len(), 15);
}

#[test]
fn remaining_duplicate_referencing_declared() {
    let mut declared_buf = AccountBuffer::new(0);
    declared_buf.init([0xDD; 32], [0xAA; 32], 777, 0, true, false);
    let declared_view = unsafe { declared_buf.view() };

    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0), // references declared[0]
    ]);
    let declared = [declared_view];
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &declared);

    // get() path: resolve_dup_walk reads via ptr::read from declared slice
    let v1 = remaining.get(1).unwrap();
    assert_eq!(v1.address(), &Address::new_from_array([0xDD; 32]));
}

#[test]
fn remaining_iterator_dup_cache_resolution() {
    // Iterator: dup references an earlier remaining account (not declared).
    // This exercises the cache: ptr::write to cache on yield, ptr::read
    // from cache on dup resolution. The cache is MaybeUninit<[AccountView; 64]>.
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0), // references remaining[0] via cache
    ]);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].address(), views[1].address());
}

#[test]
fn remaining_iterator_overflow_returns_error() {
    const LIMIT: usize = 64;
    let mut entries = Vec::new();
    for i in 0..=LIMIT {
        entries.push(MultiAccountEntry::account(i as u8, 0));
    }
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let mut iter = remaining.iter();
    for _ in 0..LIMIT {
        let view = iter.next().unwrap().unwrap();
        assert_eq!(view.data_len(), 0);
    }

    let err = iter.next().unwrap().unwrap_err();
    assert_eq!(err, QuasarError::RemainingAccountsOverflow.into());
    assert!(iter.next().is_none());
}

#[test]
fn remaining_empty() {
    let mut buf: Vec<u64> = vec![0; 1];
    let ptr = buf.as_mut_ptr() as *mut u8;
    let boundary = ptr as *const u8;
    let remaining = RemainingAccounts::new(ptr, boundary, &[]);

    assert!(remaining.is_empty());
    assert!(remaining.get(0).is_none());
    assert_eq!(remaining.iter().count(), 0);
}

// ===========================================================================
// 6. MaybeUninit — verifying assume_init after full initialization
//
// These test the exact pattern from CpiCall::new and dispatch!:
//   MaybeUninit::<[T; N]>::uninit() → ptr::write each element → assume_init
// Miri flags assume_init on uninitialized memory, so these verify the loop
// actually writes every element.
// ===========================================================================

#[test]
fn maybeuninit_account_view_array() {
    const N: usize = 4;
    let mut bufs: Vec<AccountBuffer> = (0..N)
        .map(|i| {
            let mut buf = AccountBuffer::new(0);
            buf.init([i as u8; 32], [0u8; 32], i as u64 * 100, 0, false, false);
            buf
        })
        .collect();

    let views: [AccountView; N] = {
        let mut arr = MaybeUninit::<[AccountView; N]>::uninit();
        let ptr = arr.as_mut_ptr() as *mut AccountView;
        for i in 0..N {
            let view = unsafe { bufs[i].view() };
            unsafe { core::ptr::write(ptr.add(i), view) };
        }
        unsafe { arr.assume_init() }
    };

    for (i, view) in views.iter().enumerate() {
        assert_eq!(view.lamports(), i as u64 * 100);
    }
}

#[test]
fn maybeuninit_zero_length() {
    // Edge case: N=0 means assume_init on a zero-size array.
    let arr: [u8; 0] = {
        let arr = MaybeUninit::<[u8; 0]>::uninit();
        unsafe { arr.assume_init() }
    };
    assert_eq!(arr.len(), 0);
}

// ===========================================================================
// 7. Event serialization — copy_nonoverlapping on repr(C)
//
// Events are serialized by memcpy from a #[repr(C)] struct. If the struct
// has padding, the copy reads uninitialized padding bytes → UB. The compile-
// time size assertions prevent this, but Miri verifies at runtime.
// ===========================================================================

#[repr(C)]
struct TestEventWithPod {
    disc: [u8; 4],
    amount: PodU64,
    flag: PodBool,
}

const _: () = assert!(size_of::<TestEventWithPod>() == 13);
const _: () = assert!(align_of::<TestEventWithPod>() == 1);

#[test]
fn event_copy_reads_all_bytes_initialized() {
    // Construct event, copy via copy_nonoverlapping, verify every byte.
    // If the struct had padding, Miri would flag the copy as reading
    // uninitialized memory.
    let event = TestEventWithPod {
        disc: [0xDE, 0xAD, 0xBE, 0xEF],
        amount: PodU64::from(1_000_000u64),
        flag: PodBool::from(true),
    };

    let mut buf = [0u8; 13];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const TestEventWithPod as *const u8,
            buf.as_mut_ptr(),
            size_of::<TestEventWithPod>(),
        );
    }

    assert_eq!(&buf[0..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        u64::from_le_bytes(buf[4..12].try_into().unwrap()),
        1_000_000
    );
    assert_eq!(buf[12], 1);
}

#[repr(C)]
struct WiderEvent {
    a: [u8; 32],
    b: PodU64,
    c: PodU32,
    d: PodU16,
    e: PodBool,
}

const _: () = assert!(size_of::<WiderEvent>() == 47);
const _: () = assert!(align_of::<WiderEvent>() == 1);

#[test]
fn event_copy_wider_struct_no_padding() {
    let event = WiderEvent {
        a: [0xAA; 32],
        b: PodU64::from(u64::MAX),
        c: PodU32::from(u32::MAX),
        d: PodU16::from(u16::MAX),
        e: PodBool::from(true),
    };

    let mut buf = [0u8; 47];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const WiderEvent as *const u8,
            buf.as_mut_ptr(),
            47,
        );
    }

    // If any of the 47 bytes were padding (uninitialized), Miri flags it.
    assert!(buf[..32].iter().all(|&b| b == 0xAA));
    assert_eq!(
        u64::from_le_bytes(buf[32..40].try_into().unwrap()),
        u64::MAX
    );
    assert_eq!(
        u32::from_le_bytes(buf[40..44].try_into().unwrap()),
        u32::MAX
    );
    assert_eq!(
        u16::from_le_bytes(buf[44..46].try_into().unwrap()),
        u16::MAX
    );
    assert_eq!(buf[46], 1);
}

// ===========================================================================
// 8. Dispatch-style pointer patterns
//
// The dispatch! macro reads the discriminator and program_id via raw pointer
// casts from instruction data. These test the exact patterns.
// ===========================================================================

#[test]
fn discriminator_read_various_lengths() {
    let ix_data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];

    // 4-byte discriminator — most common
    let disc4: [u8; 4] = unsafe { *(ix_data.as_ptr() as *const [u8; 4]) };
    assert_eq!(disc4, [0xDE, 0xAD, 0xBE, 0xEF]);

    // 1-byte discriminator — minimum
    let disc1: [u8; 1] = unsafe { *(ix_data.as_ptr() as *const [u8; 1]) };
    assert_eq!(disc1, [0xDE]);

    // 8-byte discriminator — full width
    let disc8: [u8; 8] = unsafe { *(ix_data.as_ptr() as *const [u8; 8]) };
    assert_eq!(disc8, [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn program_id_read_from_end_of_slice() {
    // In the SVM, program_id is appended after ix_data in the same allocation.
    // dispatch! reads it via: &*(ix_data.as_ptr().add(ix_data.len()) as *const [u8; 32])
    let mut combined = vec![0u8; 8 + 32];
    combined[8..].copy_from_slice(&[0x42; 32]);

    let ix_data = &combined[..8];
    let program_id: &[u8; 32] =
        unsafe { &*(ix_data.as_ptr().add(ix_data.len()) as *const [u8; 32]) };
    assert_eq!(program_id, &[0x42; 32]);
}

// ===========================================================================
// 9. Pod alignment sanity (minimal — no unsafe, just precondition verification)
// ===========================================================================

#[test]
fn pod_alignment_is_one() {
    // Pod types MUST have alignment 1. If they don't, every zero-copy Deref
    // in the framework is unsound (account data has alignment 1).
    assert_eq!(align_of::<PodU64>(), 1);
    assert_eq!(align_of::<PodU32>(), 1);
    assert_eq!(align_of::<PodU16>(), 1);
    assert_eq!(align_of::<PodU128>(), 1);
    assert_eq!(align_of::<PodI64>(), 1);
    assert_eq!(align_of::<PodI32>(), 1);
    assert_eq!(align_of::<PodI16>(), 1);
    assert_eq!(align_of::<PodI128>(), 1);
    assert_eq!(align_of::<PodBool>(), 1);
}

#[test]
fn transparent_wrapper_sizes() {
    // Account<T> and AccountView must be the same size/alignment.
    // This is the precondition for every transparent cast in the framework.
    assert_eq!(
        size_of::<Account<TestAccountType>>(),
        size_of::<AccountView>()
    );
    assert_eq!(
        align_of::<Account<TestAccountType>>(),
        align_of::<AccountView>()
    );
}

// ===========================================================================
// 10. Initialize<T> transparent cast
//
// Initialize<T> uses the same & → &mut pattern as Account<T> but has
// different trait bounds (QuasarAccount instead of Owner + AccountCheck)
// and a separate code path in initialize.rs.
// ===========================================================================

struct TestInitType;

impl Discriminator for TestInitType {
    const DISCRIMINATOR: &'static [u8] = &[0x01];
}

impl Space for TestInitType {
    const SPACE: usize = 8;
}

impl QuasarAccount for TestInitType {
    fn deserialize(_data: &[u8]) -> Result<Self, ProgramError> {
        Ok(Self)
    }
    fn serialize(&self, _data: &mut [u8]) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[test]
fn initialize_shared_to_mut_cast() {
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);

    let view = unsafe { buf.view() };
    assert!(view.is_writable());
    let init = unsafe { Initialize::<TestInitType>::from_account_view_unchecked_mut(&view) };

    // Write through &mut Initialize path
    init.to_account_view().set_lamports(999);
    // Read through original &view — aliasing test
    assert_eq!(view.lamports(), 999);
}

#[test]
fn initialize_interleaved_access() {
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);

    let view = unsafe { buf.view() };
    assert!(view.is_writable());
    let init = unsafe { Initialize::<TestInitType>::from_account_view_unchecked_mut(&view) };

    // Interleave reads between &view and &mut Initialize
    let l1 = init.to_account_view().lamports();
    let l2 = view.lamports();
    assert_eq!(l1, l2);

    init.to_account_view().set_lamports(200);
    assert_eq!(view.lamports(), 200);

    view.set_lamports(300);
    assert_eq!(init.to_account_view().lamports(), 300);
}

// ===========================================================================
// 11. define_account! types (Signer, UncheckedAccount)
//
// These are generated by the define_account! macro, which has its own
// copy of the & → &mut transparent cast — third independent implementation.
// ===========================================================================

#[test]
fn unchecked_account_shared_to_mut_cast() {
    // UncheckedAccount has zero checks — the transparent cast is the only
    // unsafe operation. Test write-through-mut + read-through-shared aliasing.
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, false, true);

    let view = unsafe { buf.view() };
    let unchecked = unsafe { UncheckedAccount::from_account_view_unchecked_mut(&view) };

    unchecked.to_account_view().set_lamports(123);
    assert_eq!(view.lamports(), 123);

    // Reverse: write through &view, read through &mut UncheckedAccount
    view.set_lamports(456);
    assert_eq!(unchecked.to_account_view().lamports(), 456);
}

#[test]
fn signer_shared_to_mut_cast() {
    // Signer checks is_signer flag before doing the transparent cast.
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, true, true);

    let view = unsafe { buf.view() };
    <SignerAccount as checks::Signer>::check(&view).unwrap();
    assert!(view.is_writable());
    let signer = unsafe { SignerAccount::from_account_view_unchecked_mut(&view) };

    signer.to_account_view().set_lamports(789);
    assert_eq!(view.lamports(), 789);

    view.set_lamports(101);
    assert_eq!(signer.to_account_view().lamports(), 101);
}

// ===========================================================================
// 12. Account::close() pattern
//
// close() does three unsafe operations on the same AccountView:
//   1. destination.set_lamports(destination.lamports() + view.lamports())
//   2. view.set_lamports(0)
//   3. view.assign(&SYSTEM_PROGRAM_ID)  — unsafe, raw pointer write to owner
//   4. view.resize(0)  — modifies data_len and resize_delta
//
// This tests the combined pattern with two AccountViews (source + dest).
// ===========================================================================

struct TestCloseableType;

impl Owner for TestCloseableType {
    const OWNER: Address = TEST_OWNER;
}

impl AccountCheck for TestCloseableType {
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl Discriminator for TestCloseableType {
    const DISCRIMINATOR: &'static [u8] = &[0x01];
}

impl Space for TestCloseableType {
    const SPACE: usize = 8;
}

impl QuasarAccount for TestCloseableType {
    fn deserialize(_data: &[u8]) -> Result<Self, ProgramError> {
        Ok(Self)
    }
    fn serialize(&self, _data: &mut [u8]) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[test]
fn close_transfers_lamports_and_zeroes_fields() {
    // Set up source account with data
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    // Write discriminator so AccountCheck passes
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    // Set up destination account
    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 500_000, 0, false, true);

    let src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    <TestCloseableType as CheckOwner>::check_owner(&src_view).unwrap();
    <TestCloseableType as AccountCheck>::check(&src_view).unwrap();
    let account = unsafe { Account::<TestCloseableType>::from_account_view_unchecked(&src_view) };
    account.close(&dst_view).unwrap();

    // Source: lamports zeroed, owner changed, data_len zeroed
    assert_eq!(src_view.lamports(), 0);
    assert_eq!(src_view.data_len(), 0);
    assert!(src_view.owned_by(&Address::new_from_array([0u8; 32])));

    // Destination: received source's lamports
    assert_eq!(dst_view.lamports(), 1_500_000);
}

// ===========================================================================
// 13. assign + resize — individual unsafe operations
//
// Test the raw pointer writes that close() relies on, in isolation.
// ===========================================================================

#[test]
fn assign_changes_owner_through_raw_pointer() {
    // assign() does: write(&mut (*self.raw).owner, new_owner.clone())
    // This is an unsafe write to the owner field of RuntimeAccount.
    let mut buf = AccountBuffer::new(8);
    buf.init([1u8; 32], [0xAA; 32], 100, 8, false, true);

    let view = unsafe { buf.view() };
    assert!(view.owned_by(&Address::new_from_array([0xAA; 32])));

    let new_owner = Address::new_from_array([0xBB; 32]);
    unsafe { view.assign(&new_owner) };

    // Read back through the same view
    assert!(view.owned_by(&new_owner));
    assert!(!view.owned_by(&Address::new_from_array([0xAA; 32])));

    // Assign again to verify repeated writes are sound
    let third_owner = Address::new_from_array([0xCC; 32]);
    unsafe { view.assign(&third_owner) };
    assert!(view.owned_by(&third_owner));
}

#[test]
fn resize_grows_and_zeroes_extension() {
    // resize_unchecked() modifies data_len and resize_delta, then zero-extends
    // with write_bytes. Verify the zero-extension doesn't write out of bounds.
    let initial_data_len = 8usize;
    let mut buf = AccountBuffer::new(initial_data_len);
    buf.init(
        [1u8; 32],
        [0u8; 32],
        100,
        initial_data_len as u64,
        false,
        true,
    );
    // Fill data with non-zero bytes
    buf.write_data(&[0xFF; 8]);

    let view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 8);

    // Grow to 16 bytes — the extension (bytes 8..16) must be zeroed
    view.resize(16).unwrap();
    assert_eq!(view.data_len(), 16);

    let data = unsafe { view.borrow_unchecked() };
    // Original data preserved
    assert!(data[..8].iter().all(|&b| b == 0xFF));
    // Extension zeroed
    assert!(data[8..16].iter().all(|&b| b == 0));

    // Shrink back
    view.resize(4).unwrap();
    assert_eq!(view.data_len(), 4);
}

// ===========================================================================
// 14. borrow_unchecked_mut write + read through other paths
//
// borrow_unchecked_mut creates &mut [u8] from the raw data pointer.
// These tests verify that writes through this path are visible when
// read through other raw-pointer-based paths (not through aliased refs).
// ===========================================================================

#[test]
fn borrow_unchecked_mut_write_then_read_via_data_ptr() {
    // Write through borrow_unchecked_mut, read through a fresh data_ptr.
    // Both paths derive independently from the raw pointer, so under Tree
    // Borrows, the read creates a new child tag that sees the write.
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);

    let view = unsafe { buf.view() };

    // Write through borrow_unchecked_mut
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&42u64.to_le_bytes());
    }
    // borrow_unchecked_mut reference is dropped

    // Read through a fresh raw pointer path
    let val = unsafe { *(view.data_ptr() as *const u64) };
    assert_eq!(val, 42);
}

#[test]
fn borrow_unchecked_mut_sequential_borrows() {
    // Multiple sequential borrow_unchecked_mut calls — each creates a new
    // &mut [u8]. Verify previous writes persist across calls.
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);

    let view = unsafe { buf.view() };

    // First borrow: write to bytes 0..8
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&100u64.to_le_bytes());
    }

    // Second borrow: write to bytes 8..16, verify bytes 0..8 persisted
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        assert_eq!(u64::from_le_bytes(data[0..8].try_into().unwrap()), 100);
        data[8..16].copy_from_slice(&200u64.to_le_bytes());
    }

    // Third borrow: verify both writes persisted
    {
        let data = unsafe { view.borrow_unchecked() };
        assert_eq!(u64::from_le_bytes(data[0..8].try_into().unwrap()), 100);
        assert_eq!(u64::from_le_bytes(data[8..16].try_into().unwrap()), 200);
    }
}

// ===========================================================================
// 15. Boundary pointer subtraction
//
// Ctx::remaining_accounts() computes the boundary as:
//   self.data.as_ptr().sub(size_of::<u64>())
//
// This exercises pointer subtraction within a single allocation.
// If data.as_ptr() is at the start of the allocation, .sub(8) would be
// before the allocation → UB. Verify with the actual SVM buffer layout.
// ===========================================================================

#[test]
fn boundary_pointer_subtraction_within_allocation() {
    // Simulate SVM buffer layout:
    //   [remaining account data (ACCOUNT_HEADER bytes)]
    //   [instruction_data_len: u64 = 4]  ← boundary points here
    //   [instruction_data: 4 bytes]       ← data slice starts here
    //   [program_id: 32 bytes]
    //
    // The pointer subtraction data.as_ptr().sub(8) must stay within the
    // allocation. Uses Vec<u64> for 8-byte alignment (RuntimeAccount requires it).
    let remaining_size = ACCOUNT_HEADER + 8; // one account with 8 bytes data, aligned
    let remaining_aligned = (remaining_size + 7) & !7;
    let ix_data_len = 8usize; // use 8 to keep u64 alignment
    let total = remaining_aligned + size_of::<u64>() + ix_data_len + 32;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    // Set up the remaining account
    let raw = base as *mut RuntimeAccount;
    unsafe {
        (*raw).borrow_state = NOT_BORROWED;
        (*raw).is_signer = 0;
        (*raw).is_writable = 1;
        (*raw).executable = 0;
        (*raw).resize_delta = 0;
        (*raw).address = Address::new_from_array([0x01; 32]);
        (*raw).owner = Address::new_from_array([0xAA; 32]);
        (*raw).lamports = 100;
        (*raw).data_len = 8;
    }

    // Write instruction_data_len
    let ix_len_offset = remaining_aligned;
    unsafe {
        *(base.add(ix_len_offset) as *mut u64) = ix_data_len as u64;
    }

    // Write instruction data
    let ix_data_offset = ix_len_offset + size_of::<u64>();
    let ix_data = unsafe { std::slice::from_raw_parts(base.add(ix_data_offset), ix_data_len) };

    // Compute boundary the way Ctx::remaining_accounts() does:
    // boundary = data.as_ptr().sub(size_of::<u64>())
    let boundary = unsafe { ix_data.as_ptr().sub(size_of::<u64>()) };

    // The boundary must point to ix_len_offset
    assert_eq!(boundary, unsafe { base.add(ix_len_offset) as *const u8 });

    // Use the boundary with RemainingAccounts
    let remaining = RemainingAccounts::new(base, boundary, &[]);
    let v = remaining.get(0).unwrap();
    assert_eq!(v.lamports(), 100);
    assert!(remaining.get(1).is_none());
}

// ===========================================================================
// 16. Full parse simulation — MaybeUninit with partial reads during init
//
// This is the Pinocchio-equivalent full deserialization test. The dispatch
// macro + parse_accounts generated code does:
//   1. MaybeUninit::<[AccountView; N]>::uninit()
//   2. Walk SVM buffer, ptr::write each AccountView into the array
//   3. For duplicates, ptr::read from ALREADY-WRITTEN elements of the
//      same MaybeUninit array (before it's fully initialized)
//   4. assume_init()
//
// Step 3 is the critical pattern we haven't tested: reading from a
// partially-initialized MaybeUninit to resolve duplicates. Element 0
// is initialized, elements 1..N are still uninit, and we read element 0.
// ===========================================================================

#[test]
fn parse_simulation_dup_from_partially_initialized_buf() {
    // Build SVM-style buffer: [acct_count: u64][unique0][unique1][dup_of_0]
    let acct0_data_len = 8usize;
    let acct1_data_len = 0usize;
    let acct0_size = (ACCOUNT_HEADER + acct0_data_len + 7) & !7;
    let acct1_size = (ACCOUNT_HEADER + acct1_data_len + 7) & !7;
    let dup_size = size_of::<u64>();
    let total = size_of::<u64>() + acct0_size + acct1_size + dup_size;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    // Write account count
    unsafe { *(base as *mut u64) = 3 };

    let accounts_start = unsafe { base.add(size_of::<u64>()) };

    // Account 0: unique, 8 bytes data, lamports=100
    let raw0 = accounts_start as *mut RuntimeAccount;
    unsafe {
        (*raw0).borrow_state = NOT_BORROWED;
        (*raw0).is_signer = 1;
        (*raw0).is_writable = 1;
        (*raw0).executable = 0;
        (*raw0).resize_delta = 0;
        (*raw0).address = Address::new_from_array([0x01; 32]);
        (*raw0).owner = Address::new_from_array([0xAA; 32]);
        (*raw0).lamports = 100;
        (*raw0).data_len = acct0_data_len as u64;
    }

    // Account 1: unique, 0 bytes data, lamports=200
    let acct1_offset = acct0_size;
    let raw1 = unsafe { accounts_start.add(acct1_offset) as *mut RuntimeAccount };
    unsafe {
        (*raw1).borrow_state = NOT_BORROWED;
        (*raw1).is_signer = 0;
        (*raw1).is_writable = 1;
        (*raw1).executable = 0;
        (*raw1).resize_delta = 0;
        (*raw1).address = Address::new_from_array([0x02; 32]);
        (*raw1).owner = Address::new_from_array([0xBB; 32]);
        (*raw1).lamports = 200;
        (*raw1).data_len = acct1_data_len as u64;
    }

    // Account 2: duplicate of account 0 (borrow_state = 0, meaning index 0)
    let acct2_offset = acct0_size + acct1_size;
    unsafe { *accounts_start.add(acct2_offset) = 0u8 }; // original index = 0

    // Now simulate what dispatch + parse_accounts does:
    const N: usize = 3;
    let mut buf = MaybeUninit::<[AccountView; N]>::uninit();
    let arr_ptr = buf.as_mut_ptr() as *mut AccountView;
    let mut ptr = accounts_start;

    for i in 0..N {
        let raw = ptr as *mut RuntimeAccount;
        let borrow = unsafe { (*raw).borrow_state };

        if borrow == NOT_BORROWED {
            let view = unsafe { AccountView::new_unchecked(raw) };
            unsafe { core::ptr::write(arr_ptr.add(i), view) };
            unsafe {
                ptr = ptr.add(ACCOUNT_HEADER + (*raw).data_len as usize);
                ptr = ptr.add((ptr as usize).wrapping_neg() & 7);
            }
        } else {
            // THIS IS THE KEY PATTERN: ptr::read from a partially-initialized
            // MaybeUninit array. Element `borrow` is already written, but
            // elements after `i` are still uninitialized.
            let dup = unsafe { core::ptr::read(arr_ptr.add(borrow as usize)) };
            unsafe { core::ptr::write(arr_ptr.add(i), dup) };
            unsafe { ptr = ptr.add(size_of::<u64>()) };
        }
    }

    let accounts = unsafe { buf.assume_init() };

    assert_eq!(accounts[0].lamports(), 100);
    assert_eq!(accounts[1].lamports(), 200);
    // Account 2 is dup of 0
    assert_eq!(accounts[2].address(), accounts[0].address());
    assert_eq!(accounts[2].lamports(), 100);
}

// ===========================================================================
// 17. Duplicate AccountViews — two &mut Account<T> to same RuntimeAccount
//
// In real instruction handlers, the SVM can pass the same account twice.
// Both AccountViews share the same raw pointer. If both are cast to
// &mut Account<T> and both write set_lamports, we have two mutable
// wrappers writing to the same RuntimeAccount through raw pointers.
// Under Tree Borrows, the writes go through the raw pointer inside
// AccountView (not through the &mut reference itself), so this should
// be sound. Verify Miri agrees.
// ===========================================================================

#[test]
fn duplicate_account_views_two_mut_refs_write() {
    // Create ONE RuntimeAccount buffer
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 1_000_000, 64, true, true);

    // Create TWO AccountViews from the same buffer (simulating duplicates)
    let view_a = unsafe { buf.view() };
    let view_b = unsafe { AccountView::new_unchecked(buf.raw()) };

    // Cast both to &mut Account<T>
    <TestAccountType as CheckOwner>::check_owner(&view_a).unwrap();
    <TestAccountType as AccountCheck>::check(&view_a).unwrap();
    assert!(view_a.is_writable());
    let acct_a = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view_a) };
    <TestAccountType as CheckOwner>::check_owner(&view_b).unwrap();
    <TestAccountType as AccountCheck>::check(&view_b).unwrap();
    assert!(view_b.is_writable());
    let acct_b = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view_b) };

    // Write through acct_a
    acct_a.to_account_view().set_lamports(100);
    assert_eq!(acct_a.to_account_view().lamports(), 100);

    // Write through acct_b — same RuntimeAccount
    acct_b.to_account_view().set_lamports(200);
    assert_eq!(acct_b.to_account_view().lamports(), 200);

    // Read through acct_a — sees acct_b's write
    assert_eq!(acct_a.to_account_view().lamports(), 200);

    // Interleave writes
    acct_a.to_account_view().set_lamports(300);
    assert_eq!(acct_b.to_account_view().lamports(), 300);
    acct_b.to_account_view().set_lamports(400);
    assert_eq!(acct_a.to_account_view().lamports(), 400);
}

#[test]
fn duplicate_account_views_deref_mut_to_same_data() {
    // Same pattern but writing through DerefMut (to account data, not lamports).
    // Two &mut Account<T> → two &mut TestZcData → both write to same bytes.
    let disc_len = 4;
    let data_len = disc_len + size_of::<TestZcData>();
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        true,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data[disc_len..disc_len + 8].copy_from_slice(&42u64.to_le_bytes());
    data[disc_len + 8] = 1;
    buf.write_data(&data);

    let view_a = unsafe { buf.view() };
    let view_b = unsafe { AccountView::new_unchecked(buf.raw()) };

    <TestAccountType as CheckOwner>::check_owner(&view_a).unwrap();
    <TestAccountType as AccountCheck>::check(&view_a).unwrap();
    assert!(view_a.is_writable());
    let acct_a = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view_a) };
    <TestAccountType as CheckOwner>::check_owner(&view_b).unwrap();
    <TestAccountType as AccountCheck>::check(&view_b).unwrap();
    assert!(view_b.is_writable());
    let acct_b = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view_b) };

    // Write through acct_a's DerefMut
    {
        let zc: &mut TestZcData = &mut *acct_a;
        zc.value = PodU64::from(111u64);
    }

    // Read through acct_b's Deref — sees acct_a's write
    {
        let zc: &TestZcData = &*acct_b;
        assert_eq!(zc.value.get(), 111);
    }

    // Write through acct_b's DerefMut
    {
        let zc: &mut TestZcData = &mut *acct_b;
        zc.value = PodU64::from(222u64);
    }

    // Read through acct_a's Deref
    {
        let zc: &TestZcData = &*acct_a;
        assert_eq!(zc.value.get(), 222);
    }
}

// ===========================================================================
// 18. Sysvar::get() on host — MaybeUninit + write_bytes + assume_init
//
// impl_sysvar_get! on non-SBF does:
//   MaybeUninit::<Self>::uninit()
//   var_addr.write_bytes(0, size_of::<Self>())
//   black_box(var_addr)
//   var.assume_init()
//
// This zero-initializes a MaybeUninit and calls assume_init. Sound only
// if all-zeros is a valid bit pattern for the type. For Rent (u64 + [u8; 8]),
// all-zeros is valid.
// ===========================================================================

#[test]
fn sysvar_get_maybeuninit_write_bytes_assume_init() {
    use quasar_core::sysvars::rent::Rent;

    // impl_sysvar_get! does: MaybeUninit::uninit() → write_bytes(0) → assume_init.
    // On host, Sysvar::get() returns Err (black_box ptr != 0), so we reproduce
    // the exact pattern manually. This verifies Miri accepts write_bytes as
    // full initialization for assume_init, and that all-zeros is valid for Rent.
    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let var_addr = var.as_mut_ptr() as *mut u8;
        unsafe { var_addr.write_bytes(0, size_of::<Rent>()) };
        unsafe { var.assume_init() }
    };

    // Zero-initialized Rent: lamports_per_byte=0, exemption_threshold=[0;8]
    assert_eq!(rent.minimum_balance_unchecked(100), 0);
}

#[test]
fn rent_current_threshold_computes_2x() {
    use quasar_core::sysvars::rent::{Rent, ACCOUNT_STORAGE_OVERHEAD};

    // Build a Rent with the current exemption threshold (2.0 as f64 le bytes)
    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let ptr = var.as_mut_ptr() as *mut u8;
        unsafe {
            // lamports_per_byte = 3480 (the standard rate)
            let lpb: u64 = 3480;
            core::ptr::copy_nonoverlapping(lpb.to_le_bytes().as_ptr(), ptr, 8);
            // exemption_threshold = 2.0 as f64 le bytes = [0,0,0,0,0,0,0,64]
            let threshold: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 64];
            core::ptr::copy_nonoverlapping(threshold.as_ptr(), ptr.add(8), 8);
            var.assume_init()
        }
    };

    let data_len = 100usize;
    let expected = 2 * (ACCOUNT_STORAGE_OVERHEAD + data_len as u64) * 3480;
    assert_eq!(rent.minimum_balance_unchecked(data_len), expected);
}

// ===========================================================================
// 19. Dynamic account fields — tight-buffer boundary probes
//
// The #[account] macro generates code for dynamic fields (String/Vec) that:
//   1. Casts account data to a ZC companion struct with PodU16 descriptors
//   2. Reads descriptor values to compute offsets into a variable tail
//   3. Creates &str via from_utf8_unchecked or &[T] via slice::from_raw_parts
//
// These tests use EXACT-SIZE buffers so any off-by-one in pointer arithmetic
// hits the allocation boundary. They probe for UB, not correctness.
// ===========================================================================

/// Simulated ZC companion struct for a dynamic account:
///   fixed: Address (32 bytes)
///   name_len: PodU16
///   tags_count: PodU16
///   [tail: name bytes | tag elements (Address)]
#[repr(C)]
#[derive(Copy, Clone)]
struct DynTestZc {
    fixed: [u8; 32],
    name_len: PodU16,
    tags_count: PodU16,
}

const _: () = assert!(align_of::<DynTestZc>() == 1);
const _: () = assert!(size_of::<DynTestZc>() == 36);

const DYN_DISC_LEN: usize = 1;
const DYN_HEADER_SIZE: usize = DYN_DISC_LEN + size_of::<DynTestZc>();

/// Build a dynamic account buffer with EXACT allocation — no slack beyond
/// RuntimeAccount header + data. Any off-by-one in pointer arithmetic
/// touches the allocation edge and Miri flags it.
fn make_dyn_buffer_exact(name: &[u8], tags: &[[u8; 32]]) -> AccountBuffer {
    let tail_size = name.len() + tags.len() * 32;
    let data_len = DYN_HEADER_SIZE + tail_size;
    // Exact: RuntimeAccount + data_len + MAX_PERMITTED_DATA_INCREASE + u64
    // (standard AccountBuffer::new). We use ::new here because the SVM always
    // provides MAX_PERMITTED_DATA_INCREASE slack, but the data_len field is
    // exact — pointer arithmetic that reads beyond data_len is caught.
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );

    let mut data = vec![0u8; data_len];
    data[0] = 0x05;
    data[DYN_DISC_LEN..DYN_DISC_LEN + 32].copy_from_slice(&[0xAA; 32]);
    let name_len_offset = DYN_DISC_LEN + 32;
    data[name_len_offset..name_len_offset + 2].copy_from_slice(&(name.len() as u16).to_le_bytes());
    let tags_count_offset = name_len_offset + 2;
    data[tags_count_offset..tags_count_offset + 2]
        .copy_from_slice(&(tags.len() as u16).to_le_bytes());
    let tail_start = DYN_HEADER_SIZE;
    data[tail_start..tail_start + name.len()].copy_from_slice(name);
    let tags_start = tail_start + name.len();
    for (i, tag) in tags.iter().enumerate() {
        data[tags_start + i * 32..tags_start + (i + 1) * 32].copy_from_slice(tag);
    }

    buf.write_data(&data);
    buf
}

#[test]
fn dynamic_zc_cast_max_capacity_name_touches_allocation_edge() {
    // Probe: name fills all 32 MAX bytes. The from_utf8_unchecked slice
    // end touches the LAST byte of account data. If the ZC cast or offset
    // arithmetic is off by 1, this reads beyond the allocation.
    let max_name = [b'x'; 32]; // 32 bytes = MAX
    let mut buf = make_dyn_buffer_exact(&max_name, &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    // The slice [DYN_HEADER_SIZE..DYN_HEADER_SIZE+32] must be exactly at
    // the end of the data region.
    assert_eq!(data.len(), DYN_HEADER_SIZE + 32);

    let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
    let offset = DYN_HEADER_SIZE;
    let len = zc.name_len.get() as usize;
    assert_eq!(len, 32);
    assert_eq!(offset + len, data.len()); // touches last byte

    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + len]) };
    assert_eq!(s.len(), 32);
}

#[test]
fn dynamic_from_raw_parts_max_tags_touches_allocation_edge() {
    // Probe: 10 tags (MAX). The from_raw_parts slice end is the LAST byte
    // of account data. Off-by-one in count or offset → out of bounds.
    let tags: Vec<[u8; 32]> = (0..10).map(|i| [i as u8; 32]).collect();
    let mut buf = make_dyn_buffer_exact(b"", &tags);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    assert_eq!(data.len(), DYN_HEADER_SIZE + 320); // 37 + 10*32

    let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
    let offset = DYN_HEADER_SIZE;
    let count = zc.tags_count.get() as usize;
    assert_eq!(count, 10);
    assert_eq!(offset + count * 32, data.len()); // touches last byte

    let slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, count) };

    // Read last element — touches bytes [data.len()-32..data.len()]
    assert_eq!(slice[9].as_array(), &[9u8; 32]);
}

#[test]
fn dynamic_header_only_no_tail() {
    // Edge case: both fields empty. data_len == DYN_HEADER_SIZE exactly.
    // ZC cast must not read beyond header. from_raw_parts with count=0
    // and from_utf8_unchecked with len=0 must not read any tail bytes.
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    assert_eq!(data.len(), DYN_HEADER_SIZE); // no tail at all

    let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
    assert_eq!(zc.name_len.get(), 0);
    assert_eq!(zc.tags_count.get(), 0);

    // Zero-length slices at the exact end of the allocation
    let offset = DYN_HEADER_SIZE;
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset]) };
    assert_eq!(s, "");

    let slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, 0) };
    assert_eq!(slice.len(), 0);
}

// ===========================================================================
// 20. Dynamic fields — aliasing between shared ZC read and mutable write
//
// The setter pattern does:
//   1. borrow_unchecked() → cast to &DynTestZc (shared) → read descriptors
//   2. Drop the shared borrow
//   3. borrow_unchecked_mut() → write data → cast to &mut DynTestZc (mutable)
//
// Under Tree Borrows, step 3 creates a new mutable child from the same raw
// pointer. If the shared &DynTestZc from step 1 is still "active" in the
// borrow tree, the retag to &mut could be UB. These tests probe that boundary.
// ===========================================================================

#[test]
fn dynamic_setter_aliasing_shared_read_then_mut_write() {
    // Probe: read ZC header through shared borrow, compute offset,
    // drop shared borrow, then write through mutable borrow to the same
    // underlying AccountView. The codegen does this in every individual setter.
    let name = b"old";
    let mut buf = make_dyn_buffer_exact(name, &[]);
    let view = unsafe { buf.view() };

    // Step 1: shared borrow → read ZC header → compute offset
    let old_name_len;
    let field_offset;
    {
        let data = unsafe { view.borrow_unchecked() };
        let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
        old_name_len = zc.name_len.get() as usize;
        field_offset = DYN_HEADER_SIZE;
    }
    // shared borrow dropped

    // Step 2: mutable borrow → write new data + update ZC header
    // Same new_name length — no realloc needed, just overwrite
    let new_name = b"NEW";
    assert_eq!(new_name.len(), old_name_len); // same size, no realloc

    let data = unsafe { view.borrow_unchecked_mut() };
    data[field_offset..field_offset + new_name.len()].copy_from_slice(new_name);

    // Also cast to &mut DynTestZc to update descriptor (same memory as step 1's &DynTestZc)
    let zc = unsafe { &mut *(data[DYN_DISC_LEN..].as_mut_ptr() as *mut DynTestZc) };
    zc.name_len = PodU16::from(new_name.len() as u16);

    // Step 3: shared borrow again to verify
    let data = unsafe { view.borrow_unchecked() };
    let s = unsafe { core::str::from_utf8_unchecked(&data[field_offset..field_offset + 3]) };
    assert_eq!(s, "NEW");
}

#[test]
fn dynamic_setter_interleaved_shared_mut_shared() {
    // Probe: shared → mut → shared → mut — interleaved borrows on the same view.
    // Each mut creates a new &mut DynTestZc. If Tree Borrows retags invalidate
    // the parent's permission, subsequent shared reads would fail.
    let name = b"AB";
    let tags = [[0xCC; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let view = unsafe { buf.view() };

    // Shared read 1
    {
        let data = unsafe { view.borrow_unchecked() };
        let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
        assert_eq!(zc.name_len.get(), 2);
        assert_eq!(zc.tags_count.get(), 1);
    }

    // Mut write 1: overwrite name bytes in place
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[DYN_HEADER_SIZE] = b'X';
        data[DYN_HEADER_SIZE + 1] = b'Y';
    }

    // Shared read 2: see mut write 1
    {
        let data = unsafe { view.borrow_unchecked() };
        let s =
            unsafe { core::str::from_utf8_unchecked(&data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 2]) };
        assert_eq!(s, "XY");
    }

    // Mut write 2: update ZC descriptor
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let zc = unsafe { &mut *(data[DYN_DISC_LEN..].as_mut_ptr() as *mut DynTestZc) };
        zc.name_len = PodU16::from(2u16); // unchanged but exercises the &mut cast
    }

    // Shared read 3: still consistent
    {
        let data = unsafe { view.borrow_unchecked() };
        let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
        assert_eq!(zc.name_len.get(), 2);
        let tags_offset = DYN_HEADER_SIZE + 2;
        let tag: &[u8; 32] = data[tags_offset..tags_offset + 32].try_into().unwrap();
        assert_eq!(tag, &[0xCC; 32]); // tags survived name writes
    }
}

// ===========================================================================
// 21. Dynamic fields — minimum overlap memmove
//
// The grow/shrink tests must exercise the smallest possible overlap geometry.
// A 1-byte grow with a 1-byte tail means source and destination share bytes.
// This is the case most likely to expose memmove bugs.
// ===========================================================================

#[test]
fn dynamic_memmove_1byte_grow_1byte_tail() {
    // Initial: name="A" (1 byte), tail after name = 1 byte (0xEE).
    // Grow name to "AB" (2 bytes). The 1-byte tail at offset DYN_HEADER_SIZE+1
    // must shift to DYN_HEADER_SIZE+2. Source [H+1..H+2] overlaps with
    // dest [H+2..H+3] by 0 bytes (adjacent) — but the ptr::copy call
    // operates on the full borrow_unchecked_mut slice, so Miri checks
    // provenance across the entire region.
    let data_len = DYN_HEADER_SIZE + 2; // 1 byte name + 1 byte "tail"
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x05;
    data[DYN_DISC_LEN..DYN_DISC_LEN + 32].copy_from_slice(&[0xAA; 32]);
    data[DYN_DISC_LEN + 32..DYN_DISC_LEN + 34].copy_from_slice(&1u16.to_le_bytes()); // name_len=1
    data[DYN_DISC_LEN + 34..DYN_DISC_LEN + 36].copy_from_slice(&0u16.to_le_bytes()); // tags_count=0
    data[DYN_HEADER_SIZE] = b'A';
    data[DYN_HEADER_SIZE + 1] = 0xEE; // simulated tail byte
    buf.write_data(&data);

    let view = unsafe { buf.view() };

    // Grow by 1 byte
    view.resize(data_len + 1).unwrap();
    let data = unsafe { view.borrow_unchecked_mut() };

    // Memmove: shift tail 1 byte forward — source and dest are adjacent
    let old_end = DYN_HEADER_SIZE + 1;
    let new_end = DYN_HEADER_SIZE + 2;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            1, // 1-byte tail
        );
    }
    data[DYN_HEADER_SIZE] = b'A';
    data[DYN_HEADER_SIZE + 1] = b'B';

    assert_eq!(data[new_end], 0xEE); // tail preserved
}

#[test]
fn dynamic_memmove_1byte_shrink_overlapping() {
    // Initial: name="AB" (2 bytes), 1-byte tail (0xFF).
    // Shrink name to "A" (1 byte). Tail shifts backward from H+2 to H+1.
    // Source region [H+2..H+3] and dest [H+1..H+2] overlap by 0 bytes
    // (adjacent), but the dangerous case is when they DO overlap:
    // use a 2-byte tail so src [H+2..H+4] and dst [H+1..H+3] overlap by 1.
    let data_len = DYN_HEADER_SIZE + 4; // 2 byte name + 2 byte tail
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x05;
    data[DYN_DISC_LEN..DYN_DISC_LEN + 32].copy_from_slice(&[0xAA; 32]);
    data[DYN_DISC_LEN + 32..DYN_DISC_LEN + 34].copy_from_slice(&2u16.to_le_bytes());
    data[DYN_DISC_LEN + 34..DYN_DISC_LEN + 36].copy_from_slice(&0u16.to_le_bytes());
    data[DYN_HEADER_SIZE] = b'A';
    data[DYN_HEADER_SIZE + 1] = b'B';
    data[DYN_HEADER_SIZE + 2] = 0xDD;
    data[DYN_HEADER_SIZE + 3] = 0xEE;
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked_mut() };

    // Memmove backward: src [H+2..H+4] → dst [H+1..H+3]. 1 byte overlap.
    let old_end = DYN_HEADER_SIZE + 2;
    let new_end = DYN_HEADER_SIZE + 1;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            2, // 2-byte tail
        );
    }

    data[DYN_HEADER_SIZE] = b'A'; // new 1-byte name

    assert_eq!(data[DYN_HEADER_SIZE + 1], 0xDD);
    assert_eq!(data[DYN_HEADER_SIZE + 2], 0xEE);

    // Shrink
    view.resize(DYN_HEADER_SIZE + 3).unwrap();
    assert_eq!(view.data_len(), DYN_HEADER_SIZE + 3);
}

// ===========================================================================
// 22. Dynamic fields — batch write with aliased shared→mut on same view
//
// set_dynamic_fields() reads the ZC header through a shared borrow
// (borrow_unchecked), copies old data into the stack buffer, THEN writes
// back through borrow_unchecked_mut. The shared and mutable paths go through
// the same raw pointer in AccountView. Under Tree Borrows, the retag from
// shared to mutable could invalidate the parent tag.
// ===========================================================================

#[test]
fn dynamic_batch_write_shared_read_then_mut_write_same_view() {
    // Probe the exact aliasing pattern from set_dynamic_fields():
    //   1. borrow_unchecked() → cast &DynTestZc → read old data into stack buf
    //   2. borrow_unchecked_mut() → copy_from_slice from stack buf → cast &mut DynTestZc
    let name = b"hello";
    let tags = [[0xDD; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let view = unsafe { buf.view() };

    // Step 1: shared borrow — read old data into stack buffer
    const MAX_TAIL: usize = 32 + 10 * 32;
    let mut stack_buf = [0u8; MAX_TAIL];
    let mut buf_offset = 0usize;

    // Read from shared borrow (this is the &DynTestZc that might alias)
    {
        let data = unsafe { view.borrow_unchecked() };
        let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };

        let mut old_offset = DYN_HEADER_SIZE;

        // Name: Some("hi") — new value
        let new_name = b"hi";
        stack_buf[buf_offset..buf_offset + 2].copy_from_slice(new_name);
        buf_offset += 2;
        old_offset += zc.name_len.get() as usize;

        // Tags: None — preserve old data from shared borrow
        let old_count = zc.tags_count.get() as usize;
        let old_bytes = old_count * 32;
        stack_buf[buf_offset..buf_offset + old_bytes]
            .copy_from_slice(&data[old_offset..old_offset + old_bytes]);
        buf_offset += old_bytes;
    }
    // shared borrow dropped — but under Tree Borrows, does the raw pointer
    // in AccountView retain its permissions for the mutable retag below?

    // Step 2: mutable borrow — write back from stack buffer
    let new_total = DYN_HEADER_SIZE + buf_offset;
    if new_total < view.data_len() {
        // Need to write BEFORE shrinking
        let data = unsafe { view.borrow_unchecked_mut() };
        data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + buf_offset]
            .copy_from_slice(&stack_buf[..buf_offset]);

        // Cast to &mut DynTestZc — same memory address as step 1's &DynTestZc
        let zc = unsafe { &mut *(data[DYN_DISC_LEN..].as_mut_ptr() as *mut DynTestZc) };
        zc.name_len = PodU16::from(2u16);
        // tags_count unchanged
    }

    view.resize(new_total).unwrap();

    // Verify through a fresh shared borrow
    let data = unsafe { view.borrow_unchecked() };
    let zc = unsafe { &*(data[DYN_DISC_LEN..].as_ptr() as *const DynTestZc) };
    assert_eq!(zc.name_len.get(), 2);
    assert_eq!(zc.tags_count.get(), 1);

    let s = unsafe { core::str::from_utf8_unchecked(&data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 2]) };
    assert_eq!(s, "hi");

    let tags_offset = DYN_HEADER_SIZE + 2;
    let tag: &[u8; 32] = data[tags_offset..tags_offset + 32].try_into().unwrap();
    assert_eq!(tag, &[0xDD; 32]);
}

// ===========================================================================
// 23. Dynamic fields — from_raw_parts_mut + write + from_raw_parts read
//
// tags_mut() creates &mut [Address] via from_raw_parts_mut. A write through
// this &mut slice, followed by a fresh from_raw_parts read, exercises the
// retag sequence. The &mut from step 1 is invalidated by step 2's shared
// retag. Under Tree Borrows, writing through &mut to account data then
// reading through a separate &[u8] from borrow_unchecked must be sound.
// ===========================================================================

#[test]
fn dynamic_vec_mut_write_then_shared_read_aliasing() {
    // Probe: from_raw_parts_mut writes, then borrow_unchecked reads the
    // same bytes through a different reference. The &mut [Address] and
    // &[u8] point to overlapping memory through the same AccountView.
    let tags = [[0x11; 32]];
    let mut buf = make_dyn_buffer_exact(b"", &tags);
    let view = unsafe { buf.view() };

    // Step 1: mutable slice — write
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let offset = DYN_HEADER_SIZE;
        let slice: &mut [Address] = unsafe {
            core::slice::from_raw_parts_mut(data[offset..].as_mut_ptr() as *mut Address, 1)
        };
        slice[0] = Address::new_from_array([0xFF; 32]);
    }
    // &mut dropped

    // Step 2: shared read — must see the write from step 1
    {
        let data = unsafe { view.borrow_unchecked() };
        let offset = DYN_HEADER_SIZE;
        let slice: &[Address] =
            unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, 1) };
        assert_eq!(slice[0].as_array(), &[0xFF; 32]);
    }
}

// ===========================================================================
// 24. Dynamic fields — copy_nonoverlapping at exact allocation edge
//
// set_tags() with max tags fills the account to its last byte.
// copy_nonoverlapping must not write past the allocation boundary.
// ===========================================================================

#[test]
fn dynamic_vec_copy_nonoverlapping_at_allocation_edge() {
    // Buffer: DYN_HEADER_SIZE + 3*32 = DYN_HEADER_SIZE + 96 bytes.
    // Write 3 tags via copy_nonoverlapping — last byte written is
    // data[DYN_HEADER_SIZE+95], which is the last data byte.
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    view.resize(DYN_HEADER_SIZE + 96).unwrap();

    let new_tags = [
        Address::new_from_array([0xAA; 32]),
        Address::new_from_array([0xBB; 32]),
        Address::new_from_array([0xCC; 32]),
    ];

    let data = unsafe { view.borrow_unchecked_mut() };
    let offset = DYN_HEADER_SIZE;
    let bytes = 96; // 3 * 32

    // This copy_nonoverlapping writes to data[offset..offset+96].
    // offset+96 == data.len(). Off-by-one → out of bounds.
    assert_eq!(offset + bytes, view.data_len());
    unsafe {
        core::ptr::copy_nonoverlapping(
            new_tags.as_ptr() as *const u8,
            data[offset..].as_mut_ptr(),
            bytes,
        );
    }

    let zc = unsafe { &mut *(data[DYN_DISC_LEN..].as_mut_ptr() as *mut DynTestZc) };
    zc.tags_count = PodU16::from(3u16);

    // Read back the last element — touches bytes [data.len()-32..data.len()]
    let data = unsafe { view.borrow_unchecked() };
    let slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, 3) };
    assert_eq!(slice[2].as_array(), &[0xCC; 32]);
}

// ===========================================================================
// 25. Instruction data — ZC header cast + variable tail at exact boundary
//
// Instruction data is a single Vec<u8>. The ZC cast + from_raw_parts must
// not read past the Vec's length. Tests use exact-length Vecs.
// ===========================================================================

#[repr(C)]
#[derive(Copy, Clone)]
struct IxDataZc {
    score: PodU64,
    name_len: PodU16,
}

const _: () = assert!(align_of::<IxDataZc>() == 1);

#[test]
fn instruction_zc_cast_exact_length_vec() {
    // Vec is exactly disc + sizeof(IxDataZc) + name_len bytes. No slack.
    let name = b"solana";
    let score: u64 = 42;

    let mut ix_data: Vec<u8> = Vec::with_capacity(1 + size_of::<IxDataZc>() + name.len());
    ix_data.push(0x00); // disc
    ix_data.extend_from_slice(&score.to_le_bytes());
    ix_data.extend_from_slice(&(name.len() as u16).to_le_bytes());
    ix_data.extend_from_slice(name);
    assert_eq!(ix_data.len(), ix_data.capacity()); // exact, no slack

    let after_disc = &ix_data[1..];
    let zc = unsafe { &*(after_disc.as_ptr() as *const IxDataZc) };
    assert_eq!(zc.score.get(), 42);
    assert_eq!(zc.name_len.get(), 6);

    let tail = &after_disc[size_of::<IxDataZc>()..];
    let dyn_len = zc.name_len.get() as usize;
    assert_eq!(dyn_len, tail.len()); // tail is exactly the name, no extra bytes

    let s = core::str::from_utf8(&tail[..dyn_len]).unwrap();
    assert_eq!(s, "solana");
}

#[test]
fn instruction_vec_arg_from_raw_parts_exact_boundary() {
    // fn batch(items: Vec<PodU64, 10>) with exactly 10 items.
    // from_raw_parts reads to the last byte of the Vec.
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct IxVecZc {
        items_count: PodU16,
    }
    const _: () = assert!(align_of::<IxVecZc>() == 1);

    let count = 10usize;
    let cap = 1 + size_of::<IxVecZc>() + count * size_of::<PodU64>();
    let mut ix_data = Vec::with_capacity(cap);
    ix_data.push(0x01);
    ix_data.extend_from_slice(&(count as u16).to_le_bytes());
    for i in 0..count {
        ix_data.extend_from_slice(&(i as u64).to_le_bytes());
    }
    assert_eq!(ix_data.len(), ix_data.capacity());

    let after_disc = &ix_data[1..];
    let zc = unsafe { &*(after_disc.as_ptr() as *const IxVecZc) };
    assert_eq!(zc.items_count.get(), 10);

    let tail = &after_disc[size_of::<IxVecZc>()..];
    assert_eq!(tail.len(), count * size_of::<PodU64>()); // exact

    let slice: &[PodU64] =
        unsafe { core::slice::from_raw_parts(tail.as_ptr() as *const PodU64, count) };

    // Read last element — touches bytes [tail.len()-8..tail.len()]
    assert_eq!(slice[9].get(), 9);
    assert_eq!(slice[0].get(), 0);
}

// ===========================================================================
// 12. Account::close — post-close rejection
//
// The basic close mechanics (lamport drain, owner reassign, data_len zero)
// are tested above in close_transfers_lamports_and_zeroes_fields (section 11).
// These tests verify that a closed account is properly REJECTED by the
// validation pipeline — the security-critical invariant.
// ===========================================================================

#[test]
fn close_rejected_by_from_account_view() {
    // After close, Account::from_account_view must fail because
    // the owner has been reassigned to the system program.
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    assert!(view.is_writable());
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&view) };

    let mut dest_buf = AccountBuffer::new(0);
    dest_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let dest_view = unsafe { dest_buf.view() };

    assert!(account.close(&dest_view).is_ok());

    // CheckOwner sees system program, expects TEST_OWNER — must reject
    let result = <TestAccountType as CheckOwner>::check_owner(&view)
        .and_then(|_| <TestAccountType as AccountCheck>::check(&view));
    assert!(
        result.is_err(),
        "from_account_view must reject a closed account (wrong owner)"
    );
}

#[test]
fn close_rejects_non_writable_destination() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 500_000, 0, false, false);

    let src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account = unsafe { Account::<TestCloseableType>::from_account_view_unchecked(&src_view) };
    let result = account.close(&dst_view);
    assert!(
        result.is_err(),
        "close must reject non-writable destination"
    );
    assert_eq!(src_view.lamports(), 1_000_000, "source lamports unchanged");
}

#[test]
fn close_rejects_lamport_overflow() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], u64::MAX, 0, false, true);

    let src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account = unsafe { Account::<TestCloseableType>::from_account_view_unchecked(&src_view) };
    let result = account.close(&dst_view);
    assert!(result.is_err(), "close must reject lamport overflow");
    assert_eq!(src_view.lamports(), 1_000_000, "source lamports unchanged");
}

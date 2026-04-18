//! Miri UB tests for quasar-lang unsafe code paths.
//!
//! **Design philosophy: adversarial.** Tests are designed to FIND undefined
//! behavior, not merely confirm correct output. Each test exercises a specific
//! unsafe pattern with inputs chosen to maximise the chance of catching UB:
//! exact-size buffers, boundary values, interleaved aliasing, and exhaustive
//! flag combinations.
//!
//! ## Run
//!
//! ```sh
//! MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
//!   cargo +nightly miri test -p quasar-lang --test miri
//! ```
//!
//! ## Flags
//!
//! - `-Zmiri-tree-borrows`: Tree Borrows model. The `& -> &mut` cast in
//!   `from_account_view_unchecked_mut` is instant UB under Stacked Borrows.
//!   Under Tree Borrows it is sound because the `&mut Account<T>` never writes
//!   to the AccountView memory itself -- writes go through the raw pointer to a
//!   separate RuntimeAccount allocation. The retag creates a "Reserved" child
//!   that never transitions to "Active".
//! - `-Zmiri-symbolic-alignment-check`: Catch alignment issues that depend on
//!   allocation placement rather than happenstance.
//!
//! ## Findings
//!
//! | Pattern | Result |
//! |---------|--------|
//! | `& -> &mut` cast (`from_account_view_unchecked_mut`) | Sound under Tree Borrows |
//! | `& -> &mut` cast (`define_account!` types) | Sound under Tree Borrows |
//! | DerefMut write + aliased read via &AccountView | Sound under Tree Borrows |
//! | Interleaved shared/mutable access (N cycles) | Sound under Tree Borrows |
//! | Internal aliasing helpers: 2/3/4 &mut to same RuntimeAccount | Sound under Tree Borrows |
//! | `borrow_unchecked_mut` rapid cycling (50 cycles) | Sound under Tree Borrows |
//! | RawCpiAccount flag extraction (all 8 combos) | Sound |
//! | MaybeUninit array init + assume_init (N=1..16) | Sound |
//! | Event memcpy from repr(C) (various sizes) | Sound |
//! | `assign` + `resize` + `close` raw pointer writes | Sound |
//! | CPI `create_account` / `transfer` / `assign` data construction | Sound |
//! | Boundary pointer subtraction (`data.as_ptr().sub(8)`) | Sound |
//! | Remaining accounts alignment rounding | **Provenance warning** |
//! | Strict remaining accounts reject duplicates | Sound |
//! | Passthrough remaining accounts preserve duplicates | Sound |
//! | Dynamic inline prefix read + boundary probes | Sound |
//! | `from_utf8_unchecked` on account data String fields | Sound |
//! | `slice::from_raw_parts` for Vec field access | Sound |
//! | `ptr::copy` (memmove) for shifting dynamic fields | Sound |
//! | `slice::from_raw_parts_mut` for Vec in-place mutation | Sound |
//! | Offset-cached view parse + O(1) accessor | Sound |
//! | Tail &str / &[u8] to end of buffer | Sound |
//!
//! Note: these aliasing tests exercise low-level internal constructors only.
//! The public `#[account(dup)]` API rejects writable duplicate field bindings
//! at macro-expansion time; duplicate runtime metas still follow SVM behavior.
//!
//! ## What Miri CANNOT test
//!
//! | Pattern | Why |
//! |---------|-----|
//! | `sol_invoke_signed_c` syscall | FFI, SBF-only |
//! | `sol_get_sysvar` syscall | FFI, SBF-only |
//! | Full dispatch loop | Requires SVM buffer from runtime |
#![allow(
    clippy::manual_div_ceil,
    clippy::useless_vec,
    clippy::deref_addrof,
    clippy::needless_range_loop,
    clippy::borrow_deref_ref
)]

use {
    quasar_lang::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        accounts::{
            account::{resize, set_lamports},
            Account, Signer as SignerAccount, UncheckedAccount,
        },
        checks,
        cpi::{CpiCall, InstructionAccount},
        error::QuasarError,
        instruction_arg::InstructionArg,
        pod::*,
        remaining::RemainingAccounts,
        traits::*,
    },
    solana_address::Address,
    solana_program_error::ProgramError,
    std::mem::{align_of, size_of, MaybeUninit},
};

// ===========================================================================
// Sweep constants -- reused across parameterized tests
// ===========================================================================

const SWEEP_DATA_LENS: &[usize] = &[0, 1, 7, 8, 15, 16, 31, 32, 64, 255];

const SWEEP_FLAG_COMBOS: &[(bool, bool, u8)] = &[
    // (is_signer, is_writable, executable)
    (false, false, 0),
    (true, false, 0),
    (false, true, 0),
    (true, true, 0),
    (false, false, 1),
    (true, false, 1),
    (false, true, 1),
    (true, true, 1),
];

// ===========================================================================
// Test helpers
// ===========================================================================

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
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn init_with_executable(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
        executable: u8,
    ) {
        self.init(address, owner, lamports, data_len, is_signer, is_writable);
        unsafe { (*self.raw()).executable = executable };
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
                    raw.padding = [0u8; 4];
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

    fn account_with_data(address_byte: u8, data: Vec<u8>) -> Self {
        let data_len = data.len();
        MultiAccountEntry::Full {
            address: [address_byte; 32],
            owner: [0xAA; 32],
            lamports: 1_000_000,
            data_len,
            data: Some(data),
            is_signer: false,
            is_writable: true,
        }
    }

    fn duplicate(original_index: usize) -> Self {
        MultiAccountEntry::Duplicate { original_index }
    }
}

// ===========================================================================
// Test-only types
// ===========================================================================

#[repr(C)]
struct TestZcData {
    value: PodU64,
    flag: PodBool,
}

const _: () = assert!(align_of::<TestZcData>() == 1);
const _: () = assert!(size_of::<TestZcData>() == 9);

#[repr(transparent)]
struct TestAccountType {
    __view: AccountView,
}

const TEST_OWNER: Address = Address::new_from_array([42u8; 32]);

unsafe impl StaticView for TestAccountType {}

impl AsAccountView for TestAccountType {
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

impl Owner for TestAccountType {
    const OWNER: Address = TEST_OWNER;
}

impl AccountCheck for TestAccountType {
    type Params = ();

    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl core::ops::Deref for TestAccountType {
    type Target = TestZcData;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.__view.data_ptr().add(4) as *const TestZcData) }
    }
}

impl core::ops::DerefMut for TestAccountType {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.__view.data_ptr().add(4) as *mut TestZcData) }
    }
}

impl ZeroCopyDeref for TestAccountType {
    type Target = TestZcData;

    #[inline(always)]
    unsafe fn deref_from(view: &AccountView) -> &Self::Target {
        &*(view.data_ptr().add(4) as *const TestZcData)
    }

    #[inline(always)]
    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
        &mut *(view.data_ptr().add(4) as *mut TestZcData)
    }
}

#[repr(transparent)]
struct TestCloseableType {
    __view: AccountView,
}

unsafe impl StaticView for TestCloseableType {}

impl AsAccountView for TestCloseableType {
    fn to_account_view(&self) -> &AccountView {
        &self.__view
    }
}

impl Owner for TestCloseableType {
    const OWNER: Address = TEST_OWNER;
}

impl AccountCheck for TestCloseableType {
    type Params = ();

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

/// Simulated ZC companion struct for a dynamic account.
#[repr(C)]
#[derive(Copy, Clone)]
struct DynTestZc {
    fixed: [u8; 32],
}

const _: () = assert!(align_of::<DynTestZc>() == 1);
const _: () = assert!(size_of::<DynTestZc>() == 32);

const DYN_DISC_LEN: usize = 1;
const DYN_FIXED_SIZE: usize = size_of::<DynTestZc>();
const DYN_HEADER_SIZE: usize = DYN_DISC_LEN + DYN_FIXED_SIZE;

/// Instruction data ZC companion struct.
#[repr(C)]
#[derive(Copy, Clone)]
struct IxDataZc {
    score: PodU64,
}

const _: () = assert!(align_of::<IxDataZc>() == 1);

// ===========================================================================
// Common test builders
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
    let mut data = vec![0u8; data_len];
    data[..disc_len].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    data[disc_len..disc_len + 8].copy_from_slice(&42u64.to_le_bytes());
    data[disc_len + 8] = 1;
    buf.write_data(&data);
    buf
}

fn make_dyn_buffer_exact(name: &[u8], tags: &[[u8; 32]]) -> AccountBuffer {
    let dyn_size = 4 + name.len() + 4 + tags.len() * 32;
    let data_len = DYN_HEADER_SIZE + dyn_size;
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
    let mut offset = 0;
    data[offset] = 0x05;
    offset += DYN_DISC_LEN;
    data[offset..offset + 32].copy_from_slice(&[0xAA; 32]);
    offset += DYN_FIXED_SIZE;
    data[offset..offset + 4].copy_from_slice(&(name.len() as u32).to_le_bytes());
    offset += 4;
    data[offset..offset + name.len()].copy_from_slice(name);
    offset += name.len();
    data[offset..offset + 4].copy_from_slice(&(tags.len() as u32).to_le_bytes());
    offset += 4;
    for (i, tag) in tags.iter().enumerate() {
        data[offset + i * 32..offset + (i + 1) * 32].copy_from_slice(tag);
    }

    buf.write_data(&data);
    buf
}

fn make_tail_buffer(tail_data: &[u8]) -> AccountBuffer {
    let data_len = DYN_HEADER_SIZE + tail_data.len();
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
    data[DYN_HEADER_SIZE..].copy_from_slice(tail_data);
    buf.write_data(&data);
    buf
}

// ###########################################################################
// SECTION 1: Aliasing & Cast Tests
//
// The & -> &mut cast is THE critical pattern. Account<T> is repr(transparent)
// over AccountView, which holds a raw pointer. from_account_view_unchecked_mut
// casts &AccountView to &mut Account<T>. Under Stacked Borrows this is instant
// UB. Under Tree Borrows it is sound because the &mut never writes to the
// AccountView memory -- writes go through the raw pointer to RuntimeAccount.
// ###########################################################################

#[test]
fn aliasing_shared_to_mut_cast_read_lamports() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 500_000, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    <TestAccountType as AccountCheck>::check(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(account.to_account_view().lamports(), 500_000);
}

#[test]
fn aliasing_shared_to_mut_cast_write_lamports() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    set_lamports(account.to_account_view(), 999);
    assert_eq!(account.to_account_view().lamports(), 999);
}

#[test]
fn aliasing_write_then_read_original_view() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 100, 64, true, true);
    let mut view = unsafe { buf.view() };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
    set_lamports(account.to_account_view(), 777);
    assert_eq!(view.lamports(), 777);
}

#[test]
fn aliasing_interleaved_50_cycles() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 0, 64, true, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..50 {
        set_lamports(account.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 1000);
        assert_eq!(account.to_account_view().lamports(), i + 1000);
    }
}

#[test]
fn aliasing_triple_ref_view_account_zc() {
    let mut buf = make_zc_buffer();
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <TestAccountType as CheckOwner>::check_owner(&view).unwrap();
    let account =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view2) };

    set_lamports(&view, 111);
    assert_eq!(account.to_account_view().lamports(), 111);
    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(777u64);
    }
    let data = unsafe { view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 777);
    assert_eq!(view.lamports(), 111);
}

#[test]
fn aliasing_deref_mut_offset_sweep() {
    let disc_len = 4;
    let zc_size = size_of::<TestZcData>();
    for &extra_slack in &[0usize, 1, 7, 8, 15, 100] {
        let data_len = disc_len + zc_size + extra_slack;
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
        data[disc_len..disc_len + 8].copy_from_slice(&(extra_slack as u64).to_le_bytes());
        data[disc_len + 8] = 1;
        buf.write_data(&data);

        let mut view = unsafe { buf.view() };
        let account =
            unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };
        let zc: &mut TestZcData = &mut *account;
        assert_eq!(zc.value.get(), extra_slack as u64);
        zc.value = PodU64::from(42u64);
        assert_eq!(zc.value.get(), 42);
    }
}

#[test]
fn aliasing_duplicate_accounts_2_mut_refs() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 1_000_000, 64, true, true);

    let mut view_a = unsafe { buf.view() };
    let mut view_b = unsafe { AccountView::new_unchecked(buf.raw()) };

    let acct_a =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_a) };
    let acct_b =
        unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_b) };

    for i in 0u64..20 {
        set_lamports(acct_a.to_account_view(), i);
        assert_eq!(acct_b.to_account_view().lamports(), i);
        set_lamports(acct_b.to_account_view(), i + 1000);
        assert_eq!(acct_a.to_account_view().lamports(), i + 1000);
    }
}

#[test]
fn aliasing_duplicate_accounts_3_mut_refs() {
    let mut buf = AccountBuffer::new(64);
    buf.init([1u8; 32], TEST_OWNER.to_bytes(), 0, 64, true, true);

    let mut view_a = unsafe { buf.view() };
    let mut view_b = unsafe { AccountView::new_unchecked(buf.raw()) };
    let mut view_c = unsafe { AccountView::new_unchecked(buf.raw()) };

    let a = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_a) };
    let b = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_b) };
    let c = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view_c) };

    set_lamports(a.to_account_view(), 1);
    assert_eq!(b.to_account_view().lamports(), 1);
    set_lamports(b.to_account_view(), 2);
    assert_eq!(c.to_account_view().lamports(), 2);
    set_lamports(c.to_account_view(), 3);
    assert_eq!(a.to_account_view().lamports(), 3);
}

#[test]
fn aliasing_duplicate_accounts_4_deref_mut_to_same_data() {
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
    buf.write_data(&data);

    let mut views: Vec<AccountView> = (0..4)
        .map(|_| unsafe { AccountView::new_unchecked(buf.raw()) })
        .collect();
    let accts: Vec<&mut Account<TestAccountType>> = views
        .iter_mut()
        .map(|v| unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(v) })
        .collect();

    for (i, acct) in accts.iter().enumerate() {
        let zc: &mut TestZcData =
            unsafe { &mut *(acct.to_account_view().data_ptr().add(4) as *mut TestZcData) };
        zc.value = PodU64::from((i as u64 + 1) * 100);
    }
    let final_val = unsafe {
        (*(accts[0].to_account_view().data_ptr().add(4) as *const TestZcData))
            .value
            .get()
    };
    assert_eq!(final_val, 400);
}

#[test]
fn aliasing_borrow_unchecked_mut_rapid_cycling() {
    let mut buf = AccountBuffer::new(32);
    buf.init([1u8; 32], [0u8; 32], 100, 32, false, true);
    let mut view = unsafe { buf.view() };

    for i in 0u64..50 {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&i.to_le_bytes());
    }
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(u64::from_le_bytes(data[0..8].try_into().unwrap()), 49);
}

#[test]
fn aliasing_unchecked_account_write_read() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, false, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    let unchecked = unsafe { UncheckedAccount::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..10 {
        set_lamports(unchecked.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 100);
        assert_eq!(unchecked.to_account_view().lamports(), i + 100);
    }
}

#[test]
fn aliasing_signer_write_read() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 500, 0, true, true);
    let view = unsafe { buf.view() };
    let mut view2 = unsafe { AccountView::new_unchecked(buf.raw()) };
    <SignerAccount as checks::Signer>::check(&view).unwrap();
    let signer = unsafe { SignerAccount::from_account_view_unchecked_mut(&mut view2) };

    for i in 0u64..10 {
        set_lamports(signer.to_account_view(), i);
        assert_eq!(view.lamports(), i);
        set_lamports(&view, i + 100);
        assert_eq!(signer.to_account_view().lamports(), i + 100);
    }
}

#[test]
fn aliasing_deref_mut_write_then_deref_read() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(7777u64);
    }
    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 7777);
}

#[test]
fn aliasing_deref_mut_write_then_read_via_view() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    let zc: &mut TestZcData = &mut *account;
    zc.value = PodU64::from(12345u64);

    let data = unsafe { view.borrow_unchecked() };
    let written = u64::from_le_bytes(data[4..12].try_into().unwrap());
    assert_eq!(written, 12345);
}

#[test]
fn aliasing_multiple_deref_mut_calls() {
    let mut buf = make_zc_buffer();
    let mut view = unsafe { buf.view() };
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked_mut(&mut view) };

    for i in 0u64..10 {
        let zc: &mut TestZcData = &mut *account;
        zc.value = PodU64::from(i);
        assert_eq!(zc.value.get(), i);
    }
}

// ###########################################################################
// SECTION 2: Bounds & Pointer Arithmetic
// ###########################################################################

#[test]
fn bounds_account_view_exact_size_sweep() {
    for &data_len in SWEEP_DATA_LENS {
        let exact_size = size_of::<RuntimeAccount>() + data_len;
        let mut buf = AccountBuffer::exact(exact_size);
        buf.init([1u8; 32], [2u8; 32], 100, data_len as u64, false, true);

        let view = unsafe { buf.view() };
        assert_eq!(view.lamports(), 100);
        assert_eq!(view.data_len(), data_len);
        assert!(view.is_writable());
        assert_eq!(view.data_ptr(), unsafe {
            buf.as_mut_ptr().add(size_of::<RuntimeAccount>())
        });

        if data_len > 0 {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data.len(), data_len);
        }
    }
}

#[test]
fn bounds_zero_data_len() {
    let mut buf = AccountBuffer::exact(size_of::<RuntimeAccount>());
    buf.init([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 0);
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(data.len(), 0);
}

#[test]
fn bounds_deref_exact_size_buffer() {
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
    let account = unsafe { Account::<TestAccountType>::from_account_view_unchecked(&view) };
    let zc: &TestZcData = &*account;
    assert_eq!(zc.value.get(), 99);
    assert!(zc.flag.get());
}

#[test]
fn bounds_remaining_data_len_sweep() {
    for &data_len in SWEEP_DATA_LENS {
        let mut buf = MultiAccountBuffer::new(&[MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len,
            data: Some(vec![0xCC; data_len]),
            is_signer: false,
            is_writable: true,
        }]);
        let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);
        let v = remaining.get(0).unwrap().unwrap();
        assert_eq!(v.data_len(), data_len);
        assert!(remaining.get(1).unwrap().is_none());
    }
}

#[test]
fn bounds_remaining_walk_varied_data_lengths() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 1,
            data: Some(vec![0xFF]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 7,
            data: Some(vec![0xEE; 7]),
            is_signer: true,
            is_writable: false,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 8,
            data: Some(vec![0xDD; 8]),
            is_signer: false,
            is_writable: true,
        },
    ]);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let v0 = remaining.get(0).unwrap().unwrap();
    assert_eq!(v0.lamports(), 100);
    assert_eq!(v0.data_len(), 1);
    let v1 = remaining.get(1).unwrap().unwrap();
    assert_eq!(v1.lamports(), 200);
    assert_eq!(v1.data_len(), 7);
    let v2 = remaining.get(2).unwrap().unwrap();
    assert_eq!(v2.lamports(), 300);
    assert_eq!(v2.data_len(), 8);
    assert!(remaining.get(3).unwrap().is_none());
}

#[test]
fn bounds_remaining_max_capacity_64_accounts() {
    let entries: Vec<_> = (0..64)
        .map(|i| MultiAccountEntry::account_with_data(i as u8, vec![i as u8]))
        .collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 64);
    for (i, v) in views.iter().enumerate() {
        assert_eq!(v.data_len(), 1);
        let data = unsafe { v.borrow_unchecked() };
        assert_eq!(data[0], i as u8);
    }
}

#[test]
fn bounds_remaining_dup_index_sweep() {
    let mut declared_bufs: Vec<AccountBuffer> = (0..5)
        .map(|i| {
            let mut b = AccountBuffer::new(0);
            b.init(
                [i as u8; 32],
                [0xAA; 32],
                (i as u64 + 1) * 100,
                0,
                true,
                false,
            );
            b
        })
        .collect();
    let declared: Vec<AccountView> = declared_bufs
        .iter_mut()
        .map(|b| unsafe { b.view() })
        .collect();

    for dup_idx in 0..5 {
        let mut buf = MultiAccountBuffer::new(&[
            MultiAccountEntry::account(0x10, 0),
            MultiAccountEntry::duplicate(dup_idx),
        ]);
        let remaining =
            RemainingAccounts::new_passthrough(buf.as_mut_ptr(), buf.boundary(), &declared);
        let v = remaining.get(1).unwrap().unwrap();
        assert_eq!(v.address(), &Address::new_from_array([dup_idx as u8; 32]));
    }
}

#[test]
fn bounds_remaining_iterator_duplicate_rejected_in_strict_mode() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);
    let err = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
    assert_eq!(err, QuasarError::RemainingAccountDuplicate.into());
}

#[test]
fn bounds_remaining_iterator_dup_cache_resolution_passthrough() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::account(0x01, 0),
        MultiAccountEntry::duplicate(0),
    ]);
    let remaining = RemainingAccounts::new_passthrough(buf.as_mut_ptr(), buf.boundary(), &[]);
    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].address(), views[1].address());
}

#[test]
fn bounds_remaining_iterator_overflow_returns_error() {
    const LIMIT: usize = 64;
    let entries: Vec<_> = (0..=LIMIT)
        .map(|i| MultiAccountEntry::account(i as u8, 0))
        .collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let mut iter = remaining.iter();
    for _ in 0..LIMIT {
        iter.next().unwrap().unwrap();
    }
    let err = iter.next().unwrap().unwrap_err();
    assert_eq!(err, QuasarError::RemainingAccountsOverflow.into());
    assert!(iter.next().is_none());
}

#[test]
fn bounds_remaining_empty() {
    let mut buf: Vec<u64> = vec![0; 1];
    let ptr = buf.as_mut_ptr() as *mut u8;
    let boundary = ptr as *const u8;
    let remaining = RemainingAccounts::new(ptr, boundary, &[]);
    assert!(remaining.is_empty());
    assert!(remaining.get(0).unwrap().is_none());
    assert_eq!(remaining.iter().count(), 0);
}

#[test]
fn bounds_remaining_boundary_pointer_subtraction() {
    let remaining_size = ACCOUNT_HEADER + 8;
    let remaining_aligned = (remaining_size + 7) & !7;
    let ix_data_len = 8usize;
    let total = remaining_aligned + size_of::<u64>() + ix_data_len + 32;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    let raw = base as *mut RuntimeAccount;
    unsafe {
        (*raw).borrow_state = NOT_BORROWED;
        (*raw).is_signer = 0;
        (*raw).is_writable = 1;
        (*raw).executable = 0;
        (*raw).padding = [0u8; 4];
        (*raw).address = Address::new_from_array([0x01; 32]);
        (*raw).owner = Address::new_from_array([0xAA; 32]);
        (*raw).lamports = 100;
        (*raw).data_len = 8;
    }

    let ix_len_offset = remaining_aligned;
    unsafe { *(base.add(ix_len_offset) as *mut u64) = ix_data_len as u64 };

    let ix_data_offset = ix_len_offset + size_of::<u64>();
    let ix_data = unsafe { std::slice::from_raw_parts(base.add(ix_data_offset), ix_data_len) };
    let boundary = unsafe { ix_data.as_ptr().sub(size_of::<u64>()) };
    assert_eq!(boundary, unsafe { base.add(ix_len_offset) as *const u8 });

    let remaining = RemainingAccounts::new(base, boundary, &[]);
    let v = remaining.get(0).unwrap().unwrap();
    assert_eq!(v.lamports(), 100);
    assert!(remaining.get(1).unwrap().is_none());
}

#[test]
fn bounds_discriminator_read_various_lengths() {
    let ix_data: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];

    let disc4: [u8; 4] = unsafe { *(ix_data.as_ptr() as *const [u8; 4]) };
    assert_eq!(disc4, [0xDE, 0xAD, 0xBE, 0xEF]);
    let disc1: [u8; 1] = unsafe { *(ix_data.as_ptr() as *const [u8; 1]) };
    assert_eq!(disc1, [0xDE]);
    let disc8: [u8; 8] = unsafe { *(ix_data.as_ptr() as *const [u8; 8]) };
    assert_eq!(disc8, [0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn bounds_program_id_read_from_end_of_slice() {
    let mut combined = vec![0u8; 8 + 32];
    combined[8..].copy_from_slice(&[0x42; 32]);
    let ix_data = &combined[..8];
    let program_id: &[u8; 32] =
        unsafe { &*(ix_data.as_ptr().add(ix_data.len()) as *const [u8; 32]) };
    assert_eq!(program_id, &[0x42; 32]);
}

// ###########################################################################
// SECTION 3: Uninitialized Memory (MaybeUninit patterns)
// ###########################################################################

#[test]
fn uninit_cpi_account_count_sweep() {
    for n in 1..=8 {
        let mut bufs: Vec<AccountBuffer> = (0..n)
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

        match n {
            1 => {
                let _: CpiCall<'_, 1, 1> = CpiCall::new(
                    &program_id,
                    [InstructionAccount::writable(views[0].address())],
                    [&views[0]],
                    [0u8],
                );
            }
            2 => {
                let _: CpiCall<'_, 2, 1> = CpiCall::new(
                    &program_id,
                    [
                        InstructionAccount::writable(views[0].address()),
                        InstructionAccount::readonly(views[1].address()),
                    ],
                    [&views[0], &views[1]],
                    [0u8],
                );
            }
            3 => {
                let _: CpiCall<'_, 3, 1> = CpiCall::new(
                    &program_id,
                    [
                        InstructionAccount::writable(views[0].address()),
                        InstructionAccount::readonly(views[1].address()),
                        InstructionAccount::writable_signer(views[2].address()),
                    ],
                    [&views[0], &views[1], &views[2]],
                    [0u8],
                );
            }
            4 => {
                let _: CpiCall<'_, 4, 1> = CpiCall::new(
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
            5 => {
                let _: CpiCall<'_, 5, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            6 => {
                let _: CpiCall<'_, 6, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            7 => {
                let _: CpiCall<'_, 7, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            8 => {
                let _: CpiCall<'_, 8, 1> = CpiCall::new(
                    &program_id,
                    core::array::from_fn(|i| InstructionAccount::writable(views[i].address())),
                    core::array::from_fn(|i| &views[i]),
                    [0u8],
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn uninit_cpi_flag_pattern_exhaustive() {
    let program_id = Address::new_from_array([0u8; 32]);

    for &(is_signer, is_writable, executable) in SWEEP_FLAG_COMBOS {
        let mut buf = AccountBuffer::new(0);
        buf.init_with_executable(
            [1u8; 32],
            [2u8; 32],
            100,
            0,
            is_signer,
            is_writable,
            executable,
        );
        let view = unsafe { buf.view() };
        let _: CpiCall<'_, 1, 1> = CpiCall::new(
            &program_id,
            [InstructionAccount::writable(view.address())],
            [&view],
            [0u8],
        );
    }
}

#[test]
fn uninit_cpi_create_account_data() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);

    let from = unsafe { from_buf.view() };
    let to = unsafe { to_buf.view() };
    let owner = Address::new_from_array([0xAA; 32]);

    let call = quasar_lang::cpi::system::create_account(&from, &to, 500_000u64, 100, &owner);
    let data = call.instruction_data();
    assert_eq!(data.len(), 52);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), 500_000);
    assert_eq!(u64::from_le_bytes(data[12..20].try_into().unwrap()), 100);
    assert_eq!(&data[20..52], &[0xAA; 32]);
}

#[test]
fn uninit_cpi_transfer_data() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let from = unsafe { from_buf.view() };
    let to = unsafe { to_buf.view() };

    let call = quasar_lang::cpi::system::transfer(&from, &to, 42u64);
    let data = call.instruction_data();
    assert_eq!(data.len(), 12);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 2);
    assert_eq!(u64::from_le_bytes(data[4..12].try_into().unwrap()), 42);
}

#[test]
fn uninit_cpi_assign_data() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, true, true);
    let view = unsafe { buf.view() };
    let owner = Address::new_from_array([0xBB; 32]);

    let call = quasar_lang::cpi::system::assign(&view, &owner);
    let data = call.instruction_data();
    assert_eq!(data.len(), 36);
    assert_eq!(u32::from_le_bytes(data[0..4].try_into().unwrap()), 1);
    assert_eq!(&data[4..36], &[0xBB; 32]);
}

#[test]
fn uninit_cpi_transfer_boundary_values() {
    for &lamports in &[0u64, 1, u64::MAX] {
        let mut from_buf = AccountBuffer::new(0);
        from_buf.init([1u8; 32], [0u8; 32], lamports, 0, true, true);
        let mut to_buf = AccountBuffer::new(0);
        to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
        let from = unsafe { from_buf.view() };
        let to = unsafe { to_buf.view() };

        let call = quasar_lang::cpi::system::transfer(&from, &to, lamports);
        let data = call.instruction_data();
        assert_eq!(
            u64::from_le_bytes(data[4..12].try_into().unwrap()),
            lamports
        );
    }
}

#[test]
fn uninit_maybeuninit_account_view_array() {
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
fn uninit_maybeuninit_zero_length() {
    let arr: [u8; 0] = {
        let arr = MaybeUninit::<[u8; 0]>::uninit();
        unsafe { arr.assume_init() }
    };
    assert_eq!(arr.len(), 0);
}

#[test]
fn uninit_parse_simulation_dup_from_partially_initialized() {
    let acct0_data_len = 8usize;
    let acct1_data_len = 0usize;
    let acct0_size = (ACCOUNT_HEADER + acct0_data_len + 7) & !7;
    let acct1_size = (ACCOUNT_HEADER + acct1_data_len + 7) & !7;
    let dup_size = size_of::<u64>();
    let total = size_of::<u64>() + acct0_size + acct1_size + dup_size;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;

    unsafe { *(base as *mut u64) = 3 };
    let accounts_start = unsafe { base.add(size_of::<u64>()) };

    let raw0 = accounts_start as *mut RuntimeAccount;
    unsafe {
        (*raw0).borrow_state = NOT_BORROWED;
        (*raw0).is_signer = 1;
        (*raw0).is_writable = 1;
        (*raw0).executable = 0;
        (*raw0).padding = [0u8; 4];
        (*raw0).address = Address::new_from_array([0x01; 32]);
        (*raw0).owner = Address::new_from_array([0xAA; 32]);
        (*raw0).lamports = 100;
        (*raw0).data_len = acct0_data_len as u64;
    }

    let raw1 = unsafe { accounts_start.add(acct0_size) as *mut RuntimeAccount };
    unsafe {
        (*raw1).borrow_state = NOT_BORROWED;
        (*raw1).is_signer = 0;
        (*raw1).is_writable = 1;
        (*raw1).executable = 0;
        (*raw1).padding = [0u8; 4];
        (*raw1).address = Address::new_from_array([0x02; 32]);
        (*raw1).owner = Address::new_from_array([0xBB; 32]);
        (*raw1).lamports = 200;
        (*raw1).data_len = acct1_data_len as u64;
    }

    let acct2_offset = acct0_size + acct1_size;
    unsafe { *accounts_start.add(acct2_offset) = 0u8 };

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
            let dup = unsafe { core::ptr::read(arr_ptr.add(borrow as usize)) };
            unsafe { core::ptr::write(arr_ptr.add(i), dup) };
            unsafe { ptr = ptr.add(size_of::<u64>()) };
        }
    }

    let accounts = unsafe { buf.assume_init() };
    assert_eq!(accounts[0].lamports(), 100);
    assert_eq!(accounts[1].lamports(), 200);
    assert_eq!(accounts[2].address(), accounts[0].address());
    assert_eq!(accounts[2].lamports(), 100);
}

#[test]
fn uninit_parse_simulation_many_dups() {
    let acct_size = (ACCOUNT_HEADER + 7) & !7;
    let dup_size = size_of::<u64>();
    let total = size_of::<u64>() + acct_size * 2 + dup_size * 3;
    let u64_count = total.div_ceil(8);

    let mut buffer: Vec<u64> = vec![0; u64_count];
    let base = buffer.as_mut_ptr() as *mut u8;
    unsafe { *(base as *mut u64) = 5 };
    let accounts_start = unsafe { base.add(size_of::<u64>()) };

    for idx in 0..2 {
        let raw = unsafe { accounts_start.add(idx * acct_size) as *mut RuntimeAccount };
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = 0;
            (*raw).is_writable = 1;
            (*raw).executable = 0;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array([(idx + 1) as u8; 32]);
            (*raw).owner = Address::new_from_array([0xAA; 32]);
            (*raw).lamports = (idx as u64 + 1) * 100;
            (*raw).data_len = 0;
        }
    }

    let dup_base = unsafe { accounts_start.add(acct_size * 2) };
    unsafe {
        *dup_base = 0u8;
        *dup_base.add(dup_size) = 1u8;
        *dup_base.add(dup_size * 2) = 0u8;
    }

    const N: usize = 5;
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
            let dup = unsafe { core::ptr::read(arr_ptr.add(borrow as usize)) };
            unsafe { core::ptr::write(arr_ptr.add(i), dup) };
            unsafe { ptr = ptr.add(size_of::<u64>()) };
        }
    }

    let accounts = unsafe { buf.assume_init() };
    assert_eq!(accounts[0].lamports(), 100);
    assert_eq!(accounts[1].lamports(), 200);
    assert_eq!(accounts[2].address(), accounts[0].address());
    assert_eq!(accounts[3].address(), accounts[1].address());
    assert_eq!(accounts[4].address(), accounts[0].address());
}

#[test]
fn uninit_sysvar_maybeuninit_write_bytes_assume_init() {
    use quasar_lang::sysvars::rent::Rent;

    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let var_addr = var.as_mut_ptr() as *mut u8;
        unsafe { var_addr.write_bytes(0, size_of::<Rent>()) };
        unsafe { var.assume_init() }
    };
    assert_eq!(rent.minimum_balance_unchecked(100), 0);
}

#[test]
fn uninit_sysvar_rent_2x_threshold() {
    use quasar_lang::sysvars::rent::{Rent, ACCOUNT_STORAGE_OVERHEAD};

    let rent: Rent = {
        let mut var = MaybeUninit::<Rent>::uninit();
        let ptr = var.as_mut_ptr() as *mut u8;
        unsafe {
            let lpb: u64 = 3480;
            core::ptr::copy_nonoverlapping(lpb.to_le_bytes().as_ptr(), ptr, 8);
            let threshold: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 64];
            core::ptr::copy_nonoverlapping(threshold.as_ptr(), ptr.add(8), 8);
            var.assume_init()
        }
    };

    let data_len = 100usize;
    let expected = 2 * (ACCOUNT_STORAGE_OVERHEAD + data_len as u64) * 3480;
    assert_eq!(rent.minimum_balance_unchecked(data_len), expected);
}

// ###########################################################################
// SECTION 4: Event serialization
// ###########################################################################

#[repr(C)]
struct SmallEvent {
    disc: [u8; 4],
    amount: PodU64,
    flag: PodBool,
}
const _: () = assert!(size_of::<SmallEvent>() == 13);
const _: () = assert!(align_of::<SmallEvent>() == 1);

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

#[repr(C)]
struct MaxEvent {
    disc: [u8; 8],
    a: PodU128,
    b: PodI128,
    c: PodU64,
    d: PodI64,
    e: PodU32,
    f: PodI32,
    g: PodU16,
    h: PodI16,
    i: PodBool,
}
const _: () = assert!(size_of::<MaxEvent>() == 8 + 16 + 16 + 8 + 8 + 4 + 4 + 2 + 2 + 1);
const _: () = assert!(align_of::<MaxEvent>() == 1);

#[derive(Copy, Clone, Debug, PartialEq, Eq, quasar_lang::prelude::QuasarSerialize)]
struct RoundTripArgs {
    amount: u64,
    flag: bool,
}

#[test]
fn event_memcpy_small() {
    let event = SmallEvent {
        disc: [0xDE, 0xAD, 0xBE, 0xEF],
        amount: PodU64::from(1_000_000u64),
        flag: PodBool::from(true),
    };
    let mut buf = [0u8; 13];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const SmallEvent as *const u8,
            buf.as_mut_ptr(),
            13,
        );
    }
    assert_eq!(&buf[0..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        u64::from_le_bytes(buf[4..12].try_into().unwrap()),
        1_000_000
    );
    assert_eq!(buf[12], 1);
}

#[test]
fn event_memcpy_wider() {
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

#[test]
fn event_memcpy_max_all_pod_types() {
    let event = MaxEvent {
        disc: [0xFF; 8],
        a: PodU128::from(u128::MAX),
        b: PodI128::from(i128::MIN),
        c: PodU64::from(u64::MAX),
        d: PodI64::from(i64::MIN),
        e: PodU32::from(u32::MAX),
        f: PodI32::from(i32::MIN),
        g: PodU16::from(u16::MAX),
        h: PodI16::from(i16::MIN),
        i: PodBool::from(true),
    };
    let size = size_of::<MaxEvent>();
    let mut buf = vec![0u8; size];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const MaxEvent as *const u8,
            buf.as_mut_ptr(),
            size,
        );
    }
    assert_eq!(&buf[0..8], &[0xFF; 8]);
    assert_eq!(event.a.get(), u128::MAX);
    assert_eq!(event.b.get(), i128::MIN);
}

#[test]
fn instruction_arg_round_trip_u64() {
    let zc = <u64 as InstructionArg>::to_zc(&777u64);
    assert_eq!(<u64 as InstructionArg>::from_zc(&zc), 777u64);
    assert_eq!(align_of::<<u64 as InstructionArg>::Zc>(), 1);
}

#[test]
fn instruction_arg_round_trip_struct() {
    let value = RoundTripArgs {
        amount: 55,
        flag: true,
    };
    let zc = <RoundTripArgs as InstructionArg>::to_zc(&value);
    assert_eq!(<RoundTripArgs as InstructionArg>::from_zc(&zc), value);
    assert_eq!(align_of::<<RoundTripArgs as InstructionArg>::Zc>(), 1);
}

// ###########################################################################
// SECTION 5: Account operations (assign, resize, close)
// ###########################################################################

#[test]
fn ops_assign_changes_owner() {
    let mut buf = AccountBuffer::new(8);
    buf.init([1u8; 32], [0xAA; 32], 100, 8, false, true);
    let mut view = unsafe { buf.view() };

    for i in 0..5u8 {
        let owner = Address::new_from_array([i; 32]);
        unsafe { view.assign(&owner) };
        assert!(view.owned_by(&owner));
    }
}

#[test]
fn ops_resize_grows_and_zeroes_extension() {
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
    buf.write_data(&[0xFF; 8]);

    let mut view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 8);

    resize(&mut view, 16).unwrap();
    assert_eq!(view.data_len(), 16);

    let data = unsafe { view.borrow_unchecked() };
    assert!(data[..8].iter().all(|&b| b == 0xFF));
    assert!(data[8..16].iter().all(|&b| b == 0));

    resize(&mut view, 4).unwrap();
    assert_eq!(view.data_len(), 4);
}

#[test]
fn ops_close_transfers_lamports_and_zeroes_fields() {
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
    dst_buf.init([2u8; 32], [0u8; 32], 500_000, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    account.close(&dst_view).unwrap();

    assert_eq!(src_view.lamports(), 0);
    assert_eq!(src_view.data_len(), 0);
    assert!(src_view.owned_by(&Address::new_from_array([0u8; 32])));
    assert_eq!(dst_view.lamports(), 1_500_000);
}

#[test]
fn ops_close_rejected_by_check_owner() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [3u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dest_buf = AccountBuffer::new(0);
    dest_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let dest_view = unsafe { dest_buf.view() };

    let closeable =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    closeable.close(&dest_view).unwrap();

    let result = <TestCloseableType as CheckOwner>::check_owner(&src_view);
    assert!(result.is_err());
}

#[test]
fn ops_close_rejects_non_writable_destination() {
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

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    let result = account.close(&dst_view);
    assert!(result.is_err());
    assert_eq!(src_view.lamports(), 1_000_000);
}

#[test]
fn ops_close_rejects_lamport_overflow() {
    // Lamport overflow is physically impossible (total SOL supply ~5.8e17 <
    // u64::MAX ~1.8e19). close() uses wrapping_add to skip the overflow branch.
    // This test verifies the wrapping behavior with synthetic values that can't
    // occur in production.
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

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    let result = account.close(&dst_view);
    // wrapping_add: u64::MAX + 1_000_000 wraps (physically impossible on Solana)
    assert!(result.is_ok());
    assert_eq!(src_view.lamports(), 0);
    assert_eq!(dst_view.lamports(), u64::MAX.wrapping_add(1_000_000));
}

#[test]
fn ops_borrow_unchecked_mut_write_then_read_via_data_ptr() {
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);
    let mut view = unsafe { buf.view() };

    {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&42u64.to_le_bytes());
    }
    let val = unsafe { *(view.data_ptr() as *const u64) };
    assert_eq!(val, 42);
}

// ###########################################################################
// SECTION 6: Dynamic fields
// ###########################################################################

#[test]
fn dynamic_size_sweep() {
    let name_lens: &[usize] = &[0, 1, 7, 8, 15, 16, 31, 32];
    let tag_counts: &[usize] = &[0, 1, 5, 10];

    for &name_len in name_lens {
        for &tags_count in tag_counts {
            let name = vec![b'x'; name_len];
            let tags: Vec<[u8; 32]> = (0..tags_count).map(|i| [i as u8; 32]).collect();
            let mut buf = make_dyn_buffer_exact(&name, &tags);
            let view = unsafe { buf.view() };
            let data = unsafe { view.borrow_unchecked() };

            let expected_len = DYN_HEADER_SIZE + 4 + name_len + 4 + tags_count * 32;
            assert_eq!(data.len(), expected_len);

            let mut offset = DYN_HEADER_SIZE;
            let read_name_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            assert_eq!(read_name_len, name_len);
            offset += 4 + name_len;

            let read_tags_count =
                u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            assert_eq!(read_tags_count, tags_count);
            offset += 4;

            if tags_count > 0 {
                let slice: &[Address] = unsafe {
                    core::slice::from_raw_parts(
                        data[offset..].as_ptr() as *const Address,
                        tags_count,
                    )
                };
                assert_eq!(
                    slice[tags_count - 1].as_array(),
                    &[(tags_count - 1) as u8; 32]
                );
            }
        }
    }
}

#[test]
fn dynamic_memmove_1byte_grow_1byte_tail() {
    let name_data_offset = DYN_HEADER_SIZE + 4;
    let data_len = name_data_offset + 1 + 1;
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
    data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4].copy_from_slice(&1u32.to_le_bytes());
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = 0xEE;
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    resize(&mut view, data_len + 1).unwrap();
    let data = unsafe { view.borrow_unchecked_mut() };

    let old_end = name_data_offset + 1;
    let new_end = name_data_offset + 2;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            1,
        );
    }
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = b'B';
    assert_eq!(data[new_end], 0xEE);
}

#[test]
fn dynamic_memmove_1byte_shrink_overlapping() {
    let name_data_offset = DYN_HEADER_SIZE + 4;
    let data_len = name_data_offset + 2 + 2;
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
    data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4].copy_from_slice(&2u32.to_le_bytes());
    data[name_data_offset] = b'A';
    data[name_data_offset + 1] = b'B';
    data[name_data_offset + 2] = 0xDD;
    data[name_data_offset + 3] = 0xEE;
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked_mut() };

    let old_end = name_data_offset + 2;
    let new_end = name_data_offset + 1;
    unsafe {
        core::ptr::copy(
            data.as_ptr().add(old_end),
            data.as_mut_ptr().add(new_end),
            2,
        );
    }
    data[name_data_offset] = b'A';
    assert_eq!(data[name_data_offset + 1], 0xDD);
    assert_eq!(data[name_data_offset + 2], 0xEE);
    resize(&mut view, name_data_offset + 3).unwrap();
}

#[test]
fn dynamic_batch_write_shared_read_then_mut_write() {
    let name = b"hello";
    let tags = [[0xDD; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let mut view = unsafe { buf.view() };

    let mut preserved_tag = [0u8; 32];
    {
        let data = unsafe { view.borrow_unchecked() };
        let mut offset = DYN_HEADER_SIZE;
        let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + name_len;
        let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        assert_eq!(tags_count, 1);
        preserved_tag.copy_from_slice(&data[offset..offset + 32]);
    }

    let new_name = b"hi";
    let new_total = DYN_HEADER_SIZE + 4 + new_name.len() + 4 + 32;
    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let mut offset = DYN_HEADER_SIZE;
        data[offset..offset + 4].copy_from_slice(&(new_name.len() as u32).to_le_bytes());
        offset += 4;
        data[offset..offset + new_name.len()].copy_from_slice(new_name);
        offset += new_name.len();
        data[offset..offset + 4].copy_from_slice(&1u32.to_le_bytes());
        offset += 4;
        data[offset..offset + 32].copy_from_slice(&preserved_tag);
    }
    resize(&mut view, new_total).unwrap();

    let data = unsafe { view.borrow_unchecked() };
    let mut offset = DYN_HEADER_SIZE;
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(name_len, 2);
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + name_len]) };
    assert_eq!(s, "hi");
    offset += name_len;
    let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(tags_count, 1);
    assert_eq!(&data[offset..offset + 32], &[0xDD; 32]);
}

#[test]
fn dynamic_vec_mut_write_then_shared_read() {
    let tags = [[0x11; 32]];
    let mut buf = make_dyn_buffer_exact(b"", &tags);
    let mut view = unsafe { buf.view() };

    let tags_data_offset = DYN_HEADER_SIZE + 4 + 4;

    {
        let data = unsafe { view.borrow_unchecked_mut() };
        let slice: &mut [Address] = unsafe {
            core::slice::from_raw_parts_mut(
                data[tags_data_offset..].as_mut_ptr() as *mut Address,
                1,
            )
        };
        slice[0] = Address::new_from_array([0xFF; 32]);
    }

    {
        let data = unsafe { view.borrow_unchecked() };
        let slice: &[Address] = unsafe {
            core::slice::from_raw_parts(data[tags_data_offset..].as_ptr() as *const Address, 1)
        };
        assert_eq!(slice[0].as_array(), &[0xFF; 32]);
    }
}

#[test]
fn dynamic_copy_nonoverlapping_at_allocation_edge() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let mut view = unsafe { buf.view() };
    let target_len = DYN_HEADER_SIZE + 4 + 4 + 96;
    resize(&mut view, target_len).unwrap();

    let new_tags = [
        Address::new_from_array([0xAA; 32]),
        Address::new_from_array([0xBB; 32]),
        Address::new_from_array([0xCC; 32]),
    ];

    let data = unsafe { view.borrow_unchecked_mut() };
    let tags_data_offset = DYN_HEADER_SIZE + 4 + 4;
    let tags_prefix_offset = DYN_HEADER_SIZE + 4;
    data[tags_prefix_offset..tags_prefix_offset + 4].copy_from_slice(&3u32.to_le_bytes());

    assert_eq!(tags_data_offset + 96, target_len);
    unsafe {
        core::ptr::copy_nonoverlapping(
            new_tags.as_ptr() as *const u8,
            data[tags_data_offset..].as_mut_ptr(),
            96,
        );
    }

    let data = unsafe { view.borrow_unchecked() };
    let slice: &[Address] = unsafe {
        core::slice::from_raw_parts(data[tags_data_offset..].as_ptr() as *const Address, 3)
    };
    assert_eq!(slice[2].as_array(), &[0xCC; 32]);
}

#[test]
fn dynamic_interleaved_shared_mut_shared() {
    let name = b"AB";
    let tags = [[0xCC; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let mut view = unsafe { buf.view() };

    let name_data_offset = DYN_HEADER_SIZE + 4;

    for round in 0..3u8 {
        {
            let data = unsafe { view.borrow_unchecked() };
            let name_len = u32::from_le_bytes(
                data[DYN_HEADER_SIZE..DYN_HEADER_SIZE + 4]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(name_len, 2);
        }
        {
            let data = unsafe { view.borrow_unchecked_mut() };
            data[name_data_offset] = b'A' + round;
            data[name_data_offset + 1] = b'B' + round;
        }
        {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data[name_data_offset], b'A' + round);
            assert_eq!(data[name_data_offset + 1], b'B' + round);
        }
    }
}

#[test]
fn dynamic_offset_cached_parse_access() {
    let name = b"hello";
    let tags: Vec<[u8; 32]> = vec![[0xAA; 32], [0xBB; 32]];
    let mut buf = make_dyn_buffer_exact(name, &tags);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let mut offset = DYN_HEADER_SIZE;
    let mut __off: [u32; 1] = [0u32; 1];

    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4 + name_len;
    __off[0] = offset as u32;

    let name_offset = DYN_HEADER_SIZE;
    let name_prefix_len =
        u32::from_le_bytes(data[name_offset..name_offset + 4].try_into().unwrap()) as usize;
    let name_start = name_offset + 4;
    let name_str =
        unsafe { core::str::from_utf8_unchecked(&data[name_start..name_start + name_prefix_len]) };
    assert_eq!(name_str, "hello");

    let tags_offset = __off[0] as usize;
    let tags_count =
        u32::from_le_bytes(data[tags_offset..tags_offset + 4].try_into().unwrap()) as usize;
    let tags_start = tags_offset + 4;
    assert_eq!(tags_count, 2);
    let tags_slice: &[Address] = unsafe {
        core::slice::from_raw_parts(data[tags_start..].as_ptr() as *const Address, tags_count)
    };
    assert_eq!(tags_slice[0].as_array(), &[0xAA; 32]);
    assert_eq!(tags_slice[1].as_array(), &[0xBB; 32]);
}

#[test]
fn dynamic_offset_cached_empty_fields() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let mut offset = DYN_HEADER_SIZE;
    let mut __off: [u32; 1] = [0u32; 1];

    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    assert_eq!(name_len, 0);
    offset += 4;
    __off[0] = offset as u32;

    let tags_offset = __off[0] as usize;
    let tags_count =
        u32::from_le_bytes(data[tags_offset..tags_offset + 4].try_into().unwrap()) as usize;
    assert_eq!(tags_count, 0);
    let tags_start = tags_offset + 4;
    let tags_slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[tags_start..].as_ptr() as *const Address, 0) };
    assert_eq!(tags_slice.len(), 0);
}

// ###########################################################################
// SECTION 7: Instruction data
// ###########################################################################

#[test]
fn instruction_zc_cast_exact_length() {
    let name = b"solana";
    let score: u64 = 42;
    let cap = 1 + size_of::<IxDataZc>() + 4 + name.len();
    let mut ix_data: Vec<u8> = Vec::with_capacity(cap);
    ix_data.push(0x00);
    ix_data.extend_from_slice(&score.to_le_bytes());
    ix_data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    ix_data.extend_from_slice(name);
    assert_eq!(ix_data.len(), ix_data.capacity());

    let after_disc = &ix_data[1..];
    let zc = unsafe { &*(after_disc.as_ptr() as *const IxDataZc) };
    assert_eq!(zc.score.get(), 42);

    let dyn_start = size_of::<IxDataZc>();
    let dyn_len =
        u32::from_le_bytes(after_disc[dyn_start..dyn_start + 4].try_into().unwrap()) as usize;
    assert_eq!(dyn_len, 6);
    let name_start = dyn_start + 4;
    let s = core::str::from_utf8(&after_disc[name_start..name_start + dyn_len]).unwrap();
    assert_eq!(s, "solana");
}

#[test]
fn instruction_vec_arg_from_raw_parts_exact_boundary() {
    let count = 10usize;
    let cap = 1 + 4 + count * size_of::<PodU64>();
    let mut ix_data = Vec::with_capacity(cap);
    ix_data.push(0x01);
    ix_data.extend_from_slice(&(count as u32).to_le_bytes());
    for i in 0..count {
        ix_data.extend_from_slice(&(i as u64).to_le_bytes());
    }
    assert_eq!(ix_data.len(), ix_data.capacity());

    let after_disc = &ix_data[1..];
    let elem_count = u32::from_le_bytes(after_disc[..4].try_into().unwrap()) as usize;
    assert_eq!(elem_count, 10);

    let elements = &after_disc[4..];
    assert_eq!(elements.len(), count * size_of::<PodU64>());

    let slice: &[PodU64] =
        unsafe { core::slice::from_raw_parts(elements.as_ptr() as *const PodU64, count) };
    assert_eq!(slice[9].get(), 9);
    assert_eq!(slice[0].get(), 0);
}

// ###########################################################################
// SECTION 8: Tail fields
// ###########################################################################

#[test]
fn tail_str_exact_boundary() {
    let tail = b"tail data at boundary!";
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    let tail_len = data.len() - offset;
    assert_eq!(tail_len, tail.len());
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + tail_len]) };
    assert_eq!(s, "tail data at boundary!");
}

#[test]
fn tail_str_empty() {
    let mut buf = make_tail_buffer(b"");
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    assert_eq!(data.len() - offset, 0);
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset]) };
    assert_eq!(s, "");
}

#[test]
fn tail_bytes_exact_boundary() {
    let tail: &[u8] = &[0xFF, 0xFE, 0xFD, 0x00, 0x01];
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    assert_eq!(&data[offset..], tail);
}

#[test]
fn tail_bytes_empty() {
    let mut buf = make_tail_buffer(b"");
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };
    assert_eq!(data[DYN_HEADER_SIZE..].len(), 0);
}

#[test]
fn tail_str_multibyte_utf8() {
    let tail = "caf\u{00e9}".as_bytes();
    let mut buf = make_tail_buffer(tail);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    let offset = DYN_HEADER_SIZE;
    let tail_len = data.len() - offset;
    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset + tail_len]) };
    assert_eq!(s, "caf\u{00e9}");
}

// ###########################################################################
// SECTION 9: Adversarial inputs
// ###########################################################################

#[test]
fn adversarial_all_zero_buffer() {
    let exact = size_of::<RuntimeAccount>();
    let mut buf = AccountBuffer::exact(exact);
    // Leave everything zero -- borrow_state=0 means mutably borrowed.
    let view = unsafe { buf.view() };

    assert_eq!(view.data_len(), 0);
    assert_eq!(view.lamports(), 0);
    assert!(!view.is_signer());
    assert!(!view.is_writable());
    assert!(!view.executable());
}

#[test]
fn adversarial_max_lamports() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], u64::MAX, 0, true, true);
    let view = unsafe { buf.view() };
    assert_eq!(view.lamports(), u64::MAX);
    set_lamports(&view, u64::MAX);
    assert_eq!(view.lamports(), u64::MAX);
    set_lamports(&view, 0);
    assert_eq!(view.lamports(), 0);
}

#[test]
fn adversarial_interleaved_close_write_read() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        500_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut other_buf = AccountBuffer::new(16);
    other_buf.init([3u8; 32], TEST_OWNER.to_bytes(), 999, 16, false, true);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 100, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let other_view = unsafe { other_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    account.close(&dst_view).unwrap();

    assert_eq!(src_view.lamports(), 0);
    assert_eq!(other_view.lamports(), 999);
    set_lamports(&other_view, 888);
    assert_eq!(other_view.lamports(), 888);
}

#[test]
fn adversarial_remaining_zero_data_len_all() {
    let entries: Vec<_> = (0..8).map(|i| MultiAccountEntry::account(i, 0)).collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 8);
    for v in &views {
        assert_eq!(v.data_len(), 0);
    }
}

#[test]
fn adversarial_remaining_all_duplicates() {
    let mut entries = vec![MultiAccountEntry::account(0x01, 8)];
    for _ in 0..7 {
        entries.push(MultiAccountEntry::duplicate(0));
    }
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]);

    let err = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
    assert_eq!(err, QuasarError::RemainingAccountDuplicate.into());
}

#[test]
fn adversarial_remaining_all_duplicates_passthrough() {
    let mut entries = vec![MultiAccountEntry::account(0x01, 8)];
    for _ in 0..7 {
        entries.push(MultiAccountEntry::duplicate(0));
    }
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = RemainingAccounts::new_passthrough(buf.as_mut_ptr(), buf.boundary(), &[]);

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 8);
    let first_addr = *views[0].address();
    for v in &views[1..] {
        assert_eq!(v.address(), &first_addr);
    }
}

#[test]
fn adversarial_pod_alignment_is_one() {
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
fn adversarial_transparent_wrapper_sizes() {
    assert_eq!(
        size_of::<Account<TestAccountType>>(),
        size_of::<AccountView>()
    );
    assert_eq!(
        align_of::<Account<TestAccountType>>(),
        align_of::<AccountView>()
    );
}

#[test]
fn adversarial_resize_to_max_permitted_data_increase() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, false, true);
    let mut view = unsafe { buf.view() };

    resize(&mut view, MAX_PERMITTED_DATA_INCREASE).unwrap();
    assert_eq!(view.data_len(), MAX_PERMITTED_DATA_INCREASE);

    let result = resize(&mut view, MAX_PERMITTED_DATA_INCREASE + 1);
    assert!(result.is_err());
}

#[test]
fn adversarial_resize_ping_pong() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, false, true);
    let mut view = unsafe { buf.view() };

    for _ in 0..20 {
        resize(&mut view, 100).unwrap();
        assert_eq!(view.data_len(), 100);
        let data = unsafe { view.borrow_unchecked() };
        assert!(data.iter().all(|&b| b == 0));
        resize(&mut view, 0).unwrap();
        assert_eq!(view.data_len(), 0);
    }
}

#[test]
fn adversarial_write_all_data_bytes_then_verify() {
    for &data_len in &[1usize, 7, 8, 15, 16, 31, 32, 64, 128, 255] {
        let mut buf = AccountBuffer::new(data_len);
        buf.init([1u8; 32], [0u8; 32], 100, data_len as u64, false, true);
        let mut view = unsafe { buf.view() };

        {
            let data = unsafe { view.borrow_unchecked_mut() };
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }
        }

        {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data.len(), data_len);
            for (i, &byte) in data.iter().enumerate() {
                assert_eq!(byte, (i % 256) as u8);
            }
        }
    }
}

#[test]
fn adversarial_remaining_iterator_varied_data_lengths() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 3,
            data: Some(vec![0xFF; 3]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 0,
            data: None,
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 15,
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
fn adversarial_cpi_create_account_boundary_space() {
    for &space in &[0u64, 1, u64::MAX] {
        let mut from_buf = AccountBuffer::new(0);
        from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
        let mut to_buf = AccountBuffer::new(0);
        to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);
        let from = unsafe { from_buf.view() };
        let to = unsafe { to_buf.view() };
        let owner = Address::new_from_array([0xAA; 32]);

        let call = quasar_lang::cpi::system::create_account(&from, &to, 1u64, space, &owner);
        let data = call.instruction_data();
        assert_eq!(u64::from_le_bytes(data[12..20].try_into().unwrap()), space);
    }
}

#[test]
fn adversarial_dynamic_header_only_no_tail() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    assert_eq!(data.len(), DYN_HEADER_SIZE + 4 + 4);

    let mut offset = DYN_HEADER_SIZE;
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(name_len, 0);

    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset]) };
    assert_eq!(s, "");

    let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(tags_count, 0);

    let slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, 0) };
    assert_eq!(slice.len(), 0);
}

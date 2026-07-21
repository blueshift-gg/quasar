//! Miri UB tests for quasar-lang unsafe code paths.
#![allow(
    clippy::undocumented_unsafe_blocks,
    reason = "this adversarial fixture centralizes account-region safety at its constructors"
)]
//!
//! **Design philosophy: adversarial.** Tests are designed to find undefined
//! behavior, not merely confirm correct output. Each test exercises a specific
//! unsafe pattern with inputs chosen to maximize the chance of catching UB:
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
//! - `-Zmiri-tree-borrows`: Tree Borrows model. The shared-to-mutable cast in
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
//! | Remaining accounts preserve duplicate metas through checked handles | Sound |
//! | Dynamic inline prefix read + boundary probes | Sound |
//! | `from_utf8_unchecked` on account data String fields | Sound |
//! | `slice::from_raw_parts` for Vec field access | Sound |
//! | `ptr::copy` (memmove) for shifting dynamic fields | Sound |
//! | `slice::from_raw_parts_mut` for Vec in-place mutation | Sound |
//! | Offset-cached view parse + O(1) accessor | Sound |
//! | Tail &str / &[u8] to end of buffer | Sound |
//! | DerefMut (data_mut_ptr) write + cpi_account_from_view on same view | Sound under Tree Borrows |
//! | set_lamports write + cpi_account_from_view on same view | Sound under Tree Borrows |
//! | Interleaved data write + CpiCall construction (20 cycles) | Sound under Tree Borrows |
//! | Two views: write via one, cpi_account_from_view via the other | Sound under Tree Borrows |
//! | Multi-account CPI after writes to all accounts | Sound under Tree Borrows |
//!
//! Note: these aliasing tests exercise low-level internal constructors only.
//! The public `#[account(dup)]` API rejects writable duplicate field bindings
//! at macro-expansion time; duplicate runtime metas still follow SVM behavior.
//!
//! ## What Miri cannot test
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
        __internal::{
            AccountView, ParseFlags, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
        },
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

solana_address::declare_id!("11111111111111111111111111111112");

mod remaining_group_fixture {
    use quasar_lang::prelude::*;

    #[derive(quasar_derive::Accounts)]
    pub struct RemainingPair {
        pub first: UncheckedAccount,
        pub second: UncheckedAccount,
    }
}

#[allow(dead_code)]
mod init_with_rent_fixture {
    use quasar_lang::prelude::*;

    #[account(discriminator = 1)]
    pub struct ProbeData {
        pub value: PodU64,
    }

    #[derive(quasar_derive::Accounts)]
    pub struct InitWithRent {
        #[account(mut)]
        pub payer: Signer,
        pub rent: Sysvar<Rent>,
        #[account(mut, init)]
        pub target: Account<ProbeData>,
    }
}

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
        // SAFETY: `self.raw()` points into the live, correctly aligned
        // account buffer owned by this fixture.
        unsafe { AccountView::new_unchecked(self.raw()) }
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

impl quasar_lang::account_load::AccountLoad for TestAccountType {
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
        // SAFETY: the fixture constructs this account with a four-byte
        // discriminator followed by an aligned, initialized `TestZcData`.
        unsafe { &*(view.data_ptr().add(4) as *const TestZcData) }
    }

    #[inline(always)]
    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
        // SAFETY: same layout contract as `deref_from`; the exclusive view
        // grants exclusive access to the payload.
        unsafe { &mut *(view.data_ptr().add(4) as *mut TestZcData) }
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

impl quasar_lang::account_load::AccountLoad for TestCloseableType {
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

// Account<T> is repr(transparent) over AccountView, which holds a raw pointer.
// from_account_view_unchecked_mut casts AccountView to Account<T>. Under Tree
// Borrows it is sound because the mutable wrapper writes through the raw
// RuntimeAccount pointer rather than into the AccountView value itself.

#[path = "miri/account_ops.rs"]
mod account_ops;
#[path = "miri/adversarial.rs"]
mod adversarial;
#[path = "miri/aliasing.rs"]
mod aliasing;
#[path = "miri/bounds.rs"]
mod bounds;
#[path = "miri/cpi_aliasing.rs"]
mod cpi_aliasing;
#[path = "miri/dynamic.rs"]
mod dynamic;
#[path = "miri/events_and_args.rs"]
mod events_and_args;
#[path = "miri/initialization.rs"]
mod initialization;

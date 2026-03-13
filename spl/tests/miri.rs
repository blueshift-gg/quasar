//! Miri UB tests for quasar-spl unsafe code paths.
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
//!   cargo +nightly miri test -p quasar-spl --test miri
//! ```
//!
//! ## Flags
//!
//! - `-Zmiri-tree-borrows`: Tree Borrows model. The `& -> &mut` cast in
//!   `from_account_view_mut` is instant UB under Stacked Borrows. Under Tree
//!   Borrows it is sound because the `&mut` never writes to the AccountView
//!   memory itself — writes go through the raw pointer to a separate
//!   RuntimeAccount allocation.
//! - `-Zmiri-symbolic-alignment-check`: Catch alignment issues that depend on
//!   allocation placement rather than happenstance.
//!
//! ## Findings
//!
//! | Pattern | Result |
//! |---------|--------|
//! | `&AccountView -> &TokenAccountState` via Deref | Sound |
//! | `&AccountView -> &mut TokenAccountState` via DerefMut | Sound under Tree Borrows |
//! | `&AccountView -> &MintAccountState` via Deref | Sound |
//! | `&AccountView -> &InterfaceAccount<T>` cast | Sound |
//! | `&AccountView -> &mut InterfaceAccount<T>` cast | Sound under Tree Borrows |
//! | `view.owner()` read for CheckOwner | Sound |
//! | MaybeUninit [u8; N] instruction data (transfer, mint_to, etc.) | Sound |
//! | ZeroCopyDeref `deref_from` / `deref_from_mut` | Sound under Tree Borrows |
//! | Interleaved shared/mutable access via InterfaceAccount | Sound under Tree Borrows |
//!
//! ## What Miri CANNOT test
//!
//! | Pattern | Why |
//! |---------|-----|
//! | `sol_invoke_signed_c` syscall | FFI, SBF-only |
//! | Actual CPI execution | Requires SVM runtime |
//! | Token-2022 extensions beyond 165/82 bytes | Layout-dependent on runtime |

use {
    quasar_core::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        accounts::{account::set_lamports, Account},
        traits::*,
    },
    quasar_spl::{
        InterfaceAccount, Mint, MintAccountState, Token, TokenAccountState, SPL_TOKEN_ID,
        TOKEN_2022_ID,
    },
    solana_address::Address,
    solana_program_error::ProgramError,
    std::mem::{size_of, MaybeUninit},
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

const SPL_TOKEN_BYTES: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];
const TOKEN_2022_BYTES: [u8; 32] = [
    6, 221, 246, 225, 238, 130, 236, 193, 200, 168, 65, 2, 106, 93, 64, 59, 117, 155, 197, 130,
    200, 159, 250, 31, 239, 205, 35, 168, 238, 94, 220, 87,
];
const SPL_TOKEN_OWNER: [u8; 32] = SPL_TOKEN_BYTES;
const TOKEN_2022_OWNER: [u8; 32] = TOKEN_2022_BYTES;

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

/// Build a 165-byte token account data buffer.
///
/// Layout: mint(32) | owner(32) | amount(8) | delegate_flag(4) | delegate(32) |
///         state(1) | is_native(4) | native_amount(8) | delegated_amount(8) |
///         close_authority_flag(4) | close_authority(32)
#[allow(clippy::too_many_arguments)]
fn build_token_data(
    mint: [u8; 32],
    owner: [u8; 32],
    amount: u64,
    delegate_flag: bool,
    delegate: [u8; 32],
    state: u8,
    is_native: bool,
    native_amount: u64,
    delegated_amount: u64,
    close_authority_flag: bool,
    close_authority: [u8; 32],
) -> [u8; 165] {
    let mut data = [0u8; 165];
    let mut off = 0;
    data[off..off + 32].copy_from_slice(&mint);
    off += 32;
    data[off..off + 32].copy_from_slice(&owner);
    off += 32;
    data[off..off + 8].copy_from_slice(&amount.to_le_bytes());
    off += 8;
    data[off] = delegate_flag as u8;
    off += 4;
    data[off..off + 32].copy_from_slice(&delegate);
    off += 32;
    data[off] = state;
    off += 1;
    data[off] = is_native as u8;
    off += 4;
    data[off..off + 8].copy_from_slice(&native_amount.to_le_bytes());
    off += 8;
    data[off..off + 8].copy_from_slice(&delegated_amount.to_le_bytes());
    off += 8;
    data[off] = close_authority_flag as u8;
    off += 4;
    data[off..off + 32].copy_from_slice(&close_authority);
    data
}

/// Build a simple initialized token account with given amount.
fn build_simple_token_data(amount: u64) -> [u8; 165] {
    build_token_data(
        [0xAA; 32], // mint
        [0xBB; 32], // owner
        amount,     // amount
        false,      // delegate_flag
        [0; 32],    // delegate
        1,          // state = Initialized
        false,      // is_native
        0,          // native_amount
        0,          // delegated_amount
        false,      // close_authority_flag
        [0; 32],    // close_authority
    )
}

/// Build an 82-byte mint account data buffer.
///
/// Layout: mint_authority_flag(4) | mint_authority(32) | supply(8) |
///         decimals(1) | is_initialized(1) | freeze_authority_flag(4) |
///         freeze_authority(32)
fn build_mint_data(
    mint_authority_flag: bool,
    mint_authority: [u8; 32],
    supply: u64,
    decimals: u8,
    is_initialized: bool,
    freeze_authority_flag: bool,
    freeze_authority: [u8; 32],
) -> [u8; 82] {
    let mut data = [0u8; 82];
    let mut off = 0;
    data[off] = mint_authority_flag as u8;
    off += 4;
    data[off..off + 32].copy_from_slice(&mint_authority);
    off += 32;
    data[off..off + 8].copy_from_slice(&supply.to_le_bytes());
    off += 8;
    data[off] = decimals;
    off += 1;
    data[off] = is_initialized as u8;
    off += 1;
    data[off] = freeze_authority_flag as u8;
    off += 4;
    data[off..off + 32].copy_from_slice(&freeze_authority);
    data
}

/// Build a simple initialized mint.
fn build_simple_mint_data(supply: u64, decimals: u8) -> [u8; 82] {
    build_mint_data(
        true,       // mint_authority_flag
        [0xCC; 32], // mint_authority
        supply, decimals, true,    // is_initialized
        false,   // freeze_authority_flag
        [0; 32], // freeze_authority
    )
}

/// Create an AccountBuffer initialized as a token account with SPL Token owner.
fn token_account_buffer(amount: u64) -> (AccountBuffer, [u8; 165]) {
    let data = build_simple_token_data(amount);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);
    (buf, data)
}

/// Create an AccountBuffer initialized as a mint account with SPL Token owner.
fn mint_account_buffer(supply: u64, decimals: u8) -> (AccountBuffer, [u8; 82]) {
    let data = build_simple_mint_data(supply, decimals);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);
    (buf, data)
}

// ===========================================================================
// Section 1: TokenAccountState Deref
// ===========================================================================

#[test]
fn token_deref_reads_all_fields() {
    let (mut buf, _data) = token_account_buffer(1_000_000);
    let view = unsafe { buf.view() };

    // Use Account<Token> to exercise Deref → TokenAccountState
    <Token as CheckOwner>::check_owner(&view).unwrap();
    <Token as AccountCheck>::check(&view).unwrap();
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;

    assert_eq!(state.mint(), &Address::new_from_array([0xAA; 32]));
    assert_eq!(state.owner(), &Address::new_from_array([0xBB; 32]));
    assert_eq!(state.amount(), 1_000_000);
    assert!(!state.has_delegate());
    assert!(state.delegate().is_none());
    assert!(state.is_initialized());
    assert!(!state.is_frozen());
    assert!(!state.is_native());
    assert!(state.native_amount().is_none());
    assert_eq!(state.delegated_amount(), 0);
    assert!(!state.has_close_authority());
    assert!(state.close_authority().is_none());
}

#[test]
fn token_deref_exact_size_buffer() {
    // Allocate exactly 165 bytes of data — no slack.
    let data = build_simple_token_data(42);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 500_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;
    assert_eq!(state.amount(), 42);
}

#[test]
fn token_deref_mut_writes_amount() {
    let (mut buf, _data) = token_account_buffer(100);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read initial amount
    assert_eq!(account.amount(), 100);

    // Write new amount through DerefMut
    let state: &mut TokenAccountState = &mut *account;
    // TokenAccountState fields are private, so we write through raw pointer
    // to the amount field at offset 64 (mint=32, owner=32)
    unsafe {
        let amount_ptr = (state as *mut TokenAccountState as *mut u8).add(64);
        let new_amount: u64 = 999;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }

    // Verify write took effect
    assert_eq!(account.amount(), 999);
}

#[test]
fn token_deref_mut_aliasing_stress() {
    // &view and &mut Account<Token>, interleaved reads/writes
    let (mut buf, _data) = token_account_buffer(500);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read through &mut Account
    assert_eq!(account.amount(), 500);

    // Read lamports through the account's view
    assert_eq!(account.to_account_view().lamports(), 1_000_000);

    // Write lamports through the account's view (interior mutability)
    set_lamports(account.to_account_view(), 2_000_000);

    // Read back through &mut Account
    assert_eq!(account.to_account_view().lamports(), 2_000_000);

    // Read token data through &mut
    assert_eq!(account.amount(), 500);

    // Interleave: read account view, read account, repeat
    for _ in 0..10 {
        let _ = account.to_account_view().lamports();
        let _ = account.amount();
        let _ = account.to_account_view().data_len();
        let _ = account.mint();
    }
}

#[test]
fn token_deref_various_flag_patterns() {
    // All flags set: delegate, is_native, close_authority
    let data = build_token_data(
        [0x11; 32], // mint
        [0x22; 32], // owner
        5_000_000,  // amount
        true,       // delegate_flag
        [0x33; 32], // delegate
        2,          // state = Frozen
        true,       // is_native
        100_000,    // native_amount
        3_000_000,  // delegated_amount
        true,       // close_authority_flag
        [0x44; 32], // close_authority
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;

    assert!(state.has_delegate());
    assert_eq!(
        state.delegate().unwrap(),
        &Address::new_from_array([0x33; 32])
    );
    assert_eq!(
        state.delegate_unchecked(),
        &Address::new_from_array([0x33; 32])
    );
    assert!(state.is_frozen());
    assert!(state.is_initialized());
    assert!(state.is_native());
    assert_eq!(state.native_amount().unwrap(), 100_000);
    assert_eq!(state.delegated_amount(), 3_000_000);
    assert!(state.has_close_authority());
    assert_eq!(
        state.close_authority().unwrap(),
        &Address::new_from_array([0x44; 32])
    );
    assert_eq!(
        state.close_authority_unchecked(),
        &Address::new_from_array([0x44; 32])
    );
}

#[test]
fn token_deref_no_flags_set() {
    // All optional flags off
    let data = build_token_data(
        [0x11; 32], [0x22; 32], 0,     // zero amount
        false, // no delegate
        [0; 32], 0,     // state = Uninitialized
        false, // not native
        0, 0, false, // no close authority
        [0; 32],
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;

    assert!(!state.has_delegate());
    assert!(state.delegate().is_none());
    assert!(!state.is_native());
    assert!(state.native_amount().is_none());
    assert!(!state.has_close_authority());
    assert!(state.close_authority().is_none());
    assert!(!state.is_initialized());
    assert!(!state.is_frozen());
}

// ===========================================================================
// Section 2: MintAccountState Deref
// ===========================================================================

#[test]
fn mint_deref_reads_all_fields() {
    let (mut buf, _data) = mint_account_buffer(1_000_000_000, 9);
    let view = unsafe { buf.view() };

    <Mint as CheckOwner>::check_owner(&view).unwrap();
    <Mint as AccountCheck>::check(&view).unwrap();
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintAccountState = &*account;

    assert!(state.has_mint_authority());
    assert_eq!(
        state.mint_authority().unwrap(),
        &Address::new_from_array([0xCC; 32])
    );
    assert_eq!(
        state.mint_authority_unchecked(),
        &Address::new_from_array([0xCC; 32])
    );
    assert_eq!(state.supply(), 1_000_000_000);
    assert_eq!(state.decimals(), 9);
    assert!(state.is_initialized());
    assert!(!state.has_freeze_authority());
    assert!(state.freeze_authority().is_none());
}

#[test]
fn mint_exact_size_buffer() {
    let data = build_simple_mint_data(0, 6);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 500_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    assert_eq!(account.decimals(), 6);
    assert_eq!(account.supply(), 0);
}

#[test]
fn mint_deref_mut_write() {
    let (mut buf, _data) = mint_account_buffer(500, 6);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Mint>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(account.supply(), 500);

    // Write supply through raw pointer. Supply is at offset 36 (flag=4,
    // authority=32)
    let state: &mut MintAccountState = &mut *account;
    unsafe {
        let supply_ptr = (state as *mut MintAccountState as *mut u8).add(36);
        let new_supply: u64 = 999_999;
        core::ptr::copy_nonoverlapping(new_supply.to_le_bytes().as_ptr(), supply_ptr, 8);
    }
    assert_eq!(account.supply(), 999_999);
}

#[test]
fn mint_all_flags_set() {
    let data = build_mint_data(true, [0xAA; 32], u64::MAX, 18, true, true, [0xBB; 32]);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintAccountState = &*account;

    assert!(state.has_mint_authority());
    assert_eq!(state.supply(), u64::MAX);
    assert_eq!(state.decimals(), 18);
    assert!(state.is_initialized());
    assert!(state.has_freeze_authority());
    assert_eq!(
        state.freeze_authority().unwrap(),
        &Address::new_from_array([0xBB; 32])
    );
    assert_eq!(
        state.freeze_authority_unchecked(),
        &Address::new_from_array([0xBB; 32])
    );
}

#[test]
fn mint_no_authorities() {
    let data = build_mint_data(false, [0; 32], 0, 0, false, false, [0; 32]);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintAccountState = &*account;

    assert!(!state.has_mint_authority());
    assert!(state.mint_authority().is_none());
    assert!(!state.has_freeze_authority());
    assert!(state.freeze_authority().is_none());
    assert!(!state.is_initialized());
}

// ===========================================================================
// Section 3: InterfaceAccount Casts
// ===========================================================================

#[test]
fn interface_account_cast_spl_token_owner() {
    let data = build_simple_token_data(42);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view(&view).unwrap();

    // Deref through InterfaceAccount -> TokenAccountState
    assert_eq!(iface.amount(), 42);
    assert_eq!(iface.mint(), &Address::new_from_array([0xAA; 32]));
}

#[test]
fn interface_account_cast_token_2022_owner() {
    let data = build_simple_token_data(77);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], TOKEN_2022_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view(&view).unwrap();

    assert_eq!(iface.amount(), 77);
}

#[test]
fn interface_account_mut_cast() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, true, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();

    // Read through &mut InterfaceAccount
    assert_eq!(iface.amount(), 100);

    // Write through DerefMut -> &mut TokenAccountState
    let state: &mut TokenAccountState = &mut *iface;
    unsafe {
        let amount_ptr = (state as *mut TokenAccountState as *mut u8).add(64);
        let new_amount: u64 = 200;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }

    assert_eq!(iface.amount(), 200);
}

#[test]
fn interface_account_aliasing() {
    // &mut view -> &mut InterfaceAccount<Token>, interleaved R/W
    let data = build_simple_token_data(50);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();

    // Interleaved access — go through iface.to_account_view() to avoid
    // reborrowing `view` while `iface` holds a mutable borrow.
    assert_eq!(iface.amount(), 50);
    assert_eq!(iface.to_account_view().lamports(), 1_000_000);

    set_lamports(iface.to_account_view(), 2_000_000);
    assert_eq!(iface.to_account_view().lamports(), 2_000_000);

    // Rapid interleaving through the wrapper
    for _ in 0..20 {
        let _ = iface.to_account_view().lamports();
        let _ = iface.amount();
        let _ = iface.to_account_view().data_len();
        let _ = iface.mint();
    }
}

#[test]
fn interface_account_wrong_owner_rejected() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    // Use a random owner that is neither SPL Token nor Token-2022
    buf.init([1u8; 32], [0xFF; 32], 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view(&view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::IllegalOwner),
        Ok(_) => panic!("expected IllegalOwner"),
    }
}

#[test]
fn interface_account_immutable_rejected() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false); // NOT writable
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view_mut(&mut view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::Immutable),
        Ok(_) => panic!("expected Immutable"),
    }
}

#[test]
fn interface_account_data_too_small() {
    // Only 100 bytes of data, but TokenAccountState needs 165
    let mut buf = AccountBuffer::new(100);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 100, false, true);

    let view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view(&view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::AccountDataTooSmall),
        Ok(_) => panic!("expected AccountDataTooSmall"),
    }
}

#[test]
fn interface_account_mint_spl_token() {
    let data = build_simple_mint_data(1_000_000, 6);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Mint>::from_account_view(&view).unwrap();
    assert_eq!(iface.supply(), 1_000_000);
    assert_eq!(iface.decimals(), 6);
}

#[test]
fn interface_account_mint_token_2022() {
    let data = build_simple_mint_data(999, 9);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], TOKEN_2022_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Mint>::from_account_view(&view).unwrap();
    assert_eq!(iface.supply(), 999);
    assert_eq!(iface.decimals(), 9);
}

#[test]
fn interface_account_unchecked_cast() {
    let data = build_simple_token_data(77);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = unsafe { InterfaceAccount::<Token>::from_account_view_unchecked(&view) };
    assert_eq!(iface.amount(), 77);
}

#[test]
fn interface_account_unchecked_mut_cast() {
    let data = build_simple_token_data(88);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = unsafe { InterfaceAccount::<Token>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(iface.amount(), 88);
}

// ===========================================================================
// Section 4: ZeroCopyDeref
// ===========================================================================

#[test]
fn zero_copy_deref_from_token() {
    let (mut buf, _data) = token_account_buffer(12345);
    let view = unsafe { buf.view() };

    let state = <Token as ZeroCopyDeref>::deref_from(&view);
    assert_eq!(state.amount(), 12345);
    assert_eq!(state.mint(), &Address::new_from_array([0xAA; 32]));
    assert_eq!(state.owner(), &Address::new_from_array([0xBB; 32]));
}

#[test]
fn zero_copy_deref_from_mut_token() {
    let (mut buf, _data) = token_account_buffer(500);
    let mut view = unsafe { buf.view() };

    let state = <Token as ZeroCopyDeref>::deref_from_mut(&mut view);

    // Read
    assert_eq!(state.amount(), 500);

    // Write through mut reference
    unsafe {
        let amount_ptr = (state as *mut TokenAccountState as *mut u8).add(64);
        let new_amount: u64 = 777;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }
    assert_eq!(state.amount(), 777);
}

#[test]
fn zero_copy_deref_from_mint() {
    let (mut buf, _data) = mint_account_buffer(1_000_000, 6);
    let view = unsafe { buf.view() };

    let state = <Mint as ZeroCopyDeref>::deref_from(&view);
    assert_eq!(state.supply(), 1_000_000);
    assert_eq!(state.decimals(), 6);
}

#[test]
fn zero_copy_deref_from_mut_mint() {
    let (mut buf, _data) = mint_account_buffer(100, 9);
    let mut view = unsafe { buf.view() };

    let state = <Mint as ZeroCopyDeref>::deref_from_mut(&mut view);
    assert_eq!(state.supply(), 100);

    // Write supply
    unsafe {
        let supply_ptr = (state as *mut MintAccountState as *mut u8).add(36);
        let new_supply: u64 = 42;
        core::ptr::copy_nonoverlapping(new_supply.to_le_bytes().as_ptr(), supply_ptr, 8);
    }
    assert_eq!(state.supply(), 42);
}

#[test]
fn zero_copy_deref_from_exact_boundary() {
    // Exactly 165 bytes — tests boundary alignment of the cast
    let data = build_simple_token_data(u64::MAX);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let state = <Token as ZeroCopyDeref>::deref_from(&view);
    assert_eq!(state.amount(), u64::MAX);
}

#[test]
fn zero_copy_deref_aliased_read_after_mut() {
    // Get &mut via deref_from_mut, write, drop it, then get & via deref_from.
    let (mut buf, _data) = token_account_buffer(300);
    let mut view = unsafe { buf.view() };

    {
        let state_mut = <Token as ZeroCopyDeref>::deref_from_mut(&mut view);
        assert_eq!(state_mut.amount(), 300);

        // Write through mut
        unsafe {
            let amount_ptr = (state_mut as *mut TokenAccountState as *mut u8).add(64);
            let new_amount: u64 = 600;
            core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
        }
    }

    // Read through a fresh deref_from — tests that the write is visible
    let state_shared = <Token as ZeroCopyDeref>::deref_from(&view);
    assert_eq!(state_shared.amount(), 600);
}

// ===========================================================================
// Section 5: CPI Instruction Data (MaybeUninit patterns)
// ===========================================================================

#[test]
fn transfer_data_all_bytes_initialized() {
    let amount: u64 = 1_000_000;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 3u8); // TRANSFER opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 3);
    assert_eq!(
        u64::from_le_bytes(data[1..9].try_into().unwrap()),
        1_000_000
    );
}

#[test]
fn transfer_boundary_amounts() {
    for &amount in &[0u64, 1, u64::MAX] {
        let data = unsafe {
            let mut buf = MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 3u8);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };
        assert_eq!(data[0], 3);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), amount);
    }
}

#[test]
fn mint_to_data_initialized() {
    let amount: u64 = 999_999;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 7u8); // MINT_TO opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 7);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 999_999);
}

#[test]
fn approve_data_initialized() {
    let amount: u64 = 500;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 4u8); // APPROVE opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 4);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 500);
}

#[test]
fn burn_data_initialized() {
    let amount: u64 = u64::MAX;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 8u8); // BURN opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 8);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), u64::MAX);
}

#[test]
fn revoke_data_initialized() {
    // Revoke is a single byte — no MaybeUninit needed
    let data: [u8; 1] = [5u8]; // REVOKE opcode
    assert_eq!(data[0], 5);
}

#[test]
fn close_account_data_initialized() {
    // Close account is a single byte
    let data: [u8; 1] = [9u8]; // CLOSE_ACCOUNT opcode
    assert_eq!(data[0], 9);
}

#[test]
fn initialize_account_data_initialized() {
    let owner = Address::new_from_array([0xDD; 32]);
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 18u8); // INITIALIZE_ACCOUNT3 opcode
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };
    assert_eq!(data[0], 18);
    assert_eq!(&data[1..33], &[0xDD; 32]);
}

#[test]
fn initialize_mint_data_with_freeze_authority() {
    let mint_authority = Address::new_from_array([0xAA; 32]);
    let freeze_authority = Address::new_from_array([0xBB; 32]);
    let decimals: u8 = 9;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8); // INITIALIZE_MINT2 opcode
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        // With freeze authority
        core::ptr::write(ptr.add(34), 1u8); // COption::Some
        core::ptr::copy_nonoverlapping(freeze_authority.as_ref().as_ptr(), ptr.add(35), 32);
        buf.assume_init()
    };
    assert_eq!(data[0], 20);
    assert_eq!(data[1], 9);
    assert_eq!(&data[2..34], &[0xAA; 32]);
    assert_eq!(data[34], 1); // COption::Some tag
    assert_eq!(&data[35..67], &[0xBB; 32]);
}

#[test]
fn initialize_mint_data_without_freeze_authority() {
    let mint_authority = Address::new_from_array([0xAA; 32]);
    let decimals: u8 = 6;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8);
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        // Without freeze authority — zero the remaining 33 bytes
        core::ptr::write_bytes(ptr.add(34), 0, 33);
        buf.assume_init()
    };
    assert_eq!(data[0], 20);
    assert_eq!(data[1], 6);
    assert_eq!(&data[2..34], &[0xAA; 32]);
    // All zeros for COption::None + padding
    assert!(data[34..67].iter().all(|&b| b == 0));
}

#[test]
fn transfer_checked_data_initialized() {
    let amount: u64 = 42_000;
    let decimals: u8 = 9;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 12u8); // TRANSFER_CHECKED opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        core::ptr::write(ptr.add(9), decimals);
        buf.assume_init()
    };
    assert_eq!(data[0], 12);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 42_000);
    assert_eq!(data[9], 9);
}

#[test]
fn transfer_checked_boundary_values() {
    for &(amount, decimals) in &[(0u64, 0u8), (u64::MAX, 255), (1, 18)] {
        let data = unsafe {
            let mut buf = MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 12u8);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            core::ptr::write(ptr.add(9), decimals);
            buf.assume_init()
        };
        assert_eq!(data[0], 12);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), amount);
        assert_eq!(data[9], decimals);
    }
}

#[test]
fn sync_native_data_initialized() {
    let data: [u8; 1] = [17u8]; // SYNC_NATIVE opcode
    assert_eq!(data[0], 17);
}

// ===========================================================================
// Section 6: CheckOwner tests
// ===========================================================================

#[test]
fn check_owner_spl_token_passes() {
    let (mut buf, _data) = token_account_buffer(100);
    let view = unsafe { buf.view() };
    assert!(<Token as CheckOwner>::check_owner(&view).is_ok());
}

#[test]
fn check_owner_wrong_owner_fails() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], [0xFF; 32], 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Token as CheckOwner>::check_owner(&view).unwrap_err(),
        ProgramError::IllegalOwner
    );
}

#[test]
fn account_check_data_too_small() {
    let mut buf = AccountBuffer::new(100);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 100, false, true);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Token as AccountCheck>::check(&view).unwrap_err(),
        ProgramError::AccountDataTooSmall
    );
}

#[test]
fn mint_check_owner_passes() {
    let (mut buf, _data) = mint_account_buffer(100, 6);
    let view = unsafe { buf.view() };
    assert!(<Mint as CheckOwner>::check_owner(&view).is_ok());
}

#[test]
fn mint_account_check_data_too_small() {
    let mut buf = AccountBuffer::new(50);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 50, false, true);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Mint as AccountCheck>::check(&view).unwrap_err(),
        ProgramError::AccountDataTooSmall
    );
}

// ===========================================================================
// Section 7: Adversarial Tests
// ===========================================================================

#[test]
fn all_zero_token_data() {
    // 165 bytes of all zeros — state=0 means uninitialized
    let data = [0u8; 165];
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;

    // All fields should be zero/default
    assert_eq!(state.amount(), 0);
    assert!(!state.is_initialized());
    assert!(!state.is_frozen());
    assert!(!state.has_delegate());
    assert!(!state.is_native());
    assert!(!state.has_close_authority());
    assert_eq!(state.delegated_amount(), 0);
}

#[test]
fn all_zero_mint_data() {
    let data = [0u8; 82];
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintAccountState = &*account;

    assert_eq!(state.supply(), 0);
    assert_eq!(state.decimals(), 0);
    assert!(!state.is_initialized());
    assert!(!state.has_mint_authority());
    assert!(!state.has_freeze_authority());
}

#[test]
fn all_ff_token_data() {
    // All 0xFF bytes — tests maximum field values.
    // Note: flag fields check byte[0] == 1 (not != 0), so 0xFF flags are "false".
    let data = [0xFF; 165];
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenAccountState = &*account;

    assert_eq!(state.amount(), u64::MAX);
    // 0xFF != 1, so flags are NOT set despite all bytes being 0xFF
    assert!(!state.has_delegate());
    assert!(!state.is_native());
    assert_eq!(state.delegated_amount(), u64::MAX);
    assert!(!state.has_close_authority());
    // delegate_unchecked / close_authority_unchecked still return addresses
    assert_eq!(
        state.delegate_unchecked(),
        &Address::new_from_array([0xFF; 32])
    );
    assert_eq!(
        state.close_authority_unchecked(),
        &Address::new_from_array([0xFF; 32])
    );
}

#[test]
fn max_amount_values() {
    let data = build_token_data(
        [0xFF; 32],
        [0xFF; 32],
        u64::MAX,
        true,
        [0xFF; 32],
        1,
        true,
        u64::MAX,
        u64::MAX,
        true,
        [0xFF; 32],
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, u64::MAX, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };

    assert_eq!(account.amount(), u64::MAX);
    assert_eq!(account.native_amount().unwrap(), u64::MAX);
    assert_eq!(account.delegated_amount(), u64::MAX);
    assert_eq!(view.lamports(), u64::MAX);
}

#[test]
fn rapid_deref_mut_cycling() {
    // 50 mut/shared cycles on the same view
    let (mut buf, _data) = token_account_buffer(0);
    let mut view = unsafe { buf.view() };

    for i in 0u64..50 {
        // Mutable deref — scoped so borrow is released before shared deref
        {
            let state_mut = <Token as ZeroCopyDeref>::deref_from_mut(&mut view);
            unsafe {
                let amount_ptr = (state_mut as *mut TokenAccountState as *mut u8).add(64);
                core::ptr::copy_nonoverlapping(i.to_le_bytes().as_ptr(), amount_ptr, 8);
            }
        }

        // Shared deref
        let state_shared = <Token as ZeroCopyDeref>::deref_from(&view);
        assert_eq!(state_shared.amount(), i);
    }
}

#[test]
fn rapid_interface_account_cycling() {
    // Repeated from_account_view / from_account_view_mut cycles
    let data = build_simple_token_data(0);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };

    for _ in 0..30 {
        let shared = InterfaceAccount::<Token>::from_account_view(&view).unwrap();
        let _ = shared.amount();

        let mutable = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();
        let _ = mutable.amount();
    }
}

#[test]
fn token_account_size_assertion() {
    // Compile-time assertion is in the source, but let's verify at runtime too
    assert_eq!(TokenAccountState::LEN, 165);
    assert_eq!(core::mem::align_of::<TokenAccountState>(), 1);
}

#[test]
fn mint_account_size_assertion() {
    assert_eq!(MintAccountState::LEN, 82);
    assert_eq!(core::mem::align_of::<MintAccountState>(), 1);
}

#[test]
fn token_deref_then_lamport_write_then_reread() {
    // Lifecycle: read token data, write lamports, re-read token data
    let (mut buf, _data) = token_account_buffer(42);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read token state
    assert_eq!(account.amount(), 42);

    // Modify lamports (different region of RuntimeAccount)
    set_lamports(account.to_account_view(), 0);

    // Re-read token state — should be unaffected
    assert_eq!(account.amount(), 42);
    assert_eq!(account.to_account_view().lamports(), 0);
}

#[test]
fn maybeunit_init_then_read_every_byte_transfer() {
    // Verify every byte of the 9-byte transfer buffer is deterministic
    let amount: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 3u8);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    // Read every byte individually
    for i in 0..9 {
        let _ = data[i];
    }
    assert_eq!(data[0], 3);
    let amount_bytes = amount.to_le_bytes();
    for i in 0..8 {
        assert_eq!(data[i + 1], amount_bytes[i]);
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_initialize_mint() {
    // The largest MaybeUninit buffer: 67 bytes for initialize_mint2
    let mint_auth = [0xAA; 32];
    let freeze_auth = [0xBB; 32];

    // With freeze authority
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8);
        core::ptr::write(ptr.add(1), 9u8);
        core::ptr::copy_nonoverlapping(mint_auth.as_ptr(), ptr.add(2), 32);
        core::ptr::write(ptr.add(34), 1u8);
        core::ptr::copy_nonoverlapping(freeze_auth.as_ptr(), ptr.add(35), 32);
        buf.assume_init()
    };
    // Read every byte — Miri will flag if any is uninitialized
    for i in 0..67 {
        let _ = data[i];
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_initialize_account() {
    let owner = [0xCC; 32];
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 18u8);
        core::ptr::copy_nonoverlapping(owner.as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };
    // Read every byte
    for i in 0..33 {
        let _ = data[i];
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_transfer_checked() {
    let amount: u64 = 0x0102030405060708;
    let decimals: u8 = 18;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 12u8);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        core::ptr::write(ptr.add(9), decimals);
        buf.assume_init()
    };
    // Read every byte
    for i in 0..10 {
        let _ = data[i];
    }
    assert_eq!(data[9], 18);
}

#[test]
fn keys_eq_spl_token_id() {
    // Verify the SPL_TOKEN_ID constant matches expected bytes
    assert!(quasar_core::keys_eq(
        &SPL_TOKEN_ID,
        &Address::new_from_array(SPL_TOKEN_BYTES)
    ));
    assert!(!quasar_core::keys_eq(
        &SPL_TOKEN_ID,
        &Address::new_from_array(TOKEN_2022_BYTES)
    ));
}

#[test]
fn keys_eq_token_2022_id() {
    assert!(quasar_core::keys_eq(
        &TOKEN_2022_ID,
        &Address::new_from_array(TOKEN_2022_BYTES)
    ));
    assert!(!quasar_core::keys_eq(
        &TOKEN_2022_ID,
        &Address::new_from_array(SPL_TOKEN_BYTES)
    ));
}

#[test]
fn multiple_interface_accounts_from_different_buffers() {
    // Two separate buffers, two separate InterfaceAccount views
    let data1 = build_simple_token_data(111);
    let data2 = build_simple_token_data(222);

    let mut buf1 = AccountBuffer::new(165);
    buf1.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf1.write_data(&data1);

    let mut buf2 = AccountBuffer::new(165);
    buf2.init([2u8; 32], TOKEN_2022_OWNER, 2_000_000, 165, false, true);
    buf2.write_data(&data2);

    let view1 = unsafe { buf1.view() };
    let view2 = unsafe { buf2.view() };

    let iface1 = InterfaceAccount::<Token>::from_account_view(&view1).unwrap();
    let iface2 = InterfaceAccount::<Token>::from_account_view(&view2).unwrap();

    assert_eq!(iface1.amount(), 111);
    assert_eq!(iface2.amount(), 222);

    // Cross-read doesn't interfere
    assert_eq!(iface1.amount(), 111);
    assert_eq!(iface2.amount(), 222);
}

#[test]
fn account_view_owner_read_for_interface_check() {
    // Tests the view.owner() call path used in InterfaceAccount::from_account_view
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };

    // Explicitly test the owner read
    let owner = view.owner();
    assert!(quasar_core::keys_eq(owner, &SPL_TOKEN_ID));
}

#[test]
fn account_view_owner_read_token_2022() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], TOKEN_2022_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let owner = view.owner();
    assert!(quasar_core::keys_eq(owner, &TOKEN_2022_ID));
}

#[test]
fn token_deref_after_lamport_drain() {
    // Simulate closing: drain lamports, then read token data
    let (mut buf, _data) = token_account_buffer(42);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Drain lamports
    set_lamports(account.to_account_view(), 0);

    // Token data should still be readable (account data region unchanged)
    assert_eq!(account.amount(), 42);
}

#[test]
fn interleaved_token_and_mint_deref() {
    // Create both a token and mint buffer, interleave reads
    let (mut token_buf, _) = token_account_buffer(500);
    let (mut mint_buf, _) = mint_account_buffer(1_000_000, 6);

    let token_view = unsafe { token_buf.view() };
    let mint_view = unsafe { mint_buf.view() };

    let token_acct = unsafe { Account::<Token>::from_account_view_unchecked(&token_view) };
    let mint_acct = unsafe { Account::<Mint>::from_account_view_unchecked(&mint_view) };

    // Interleave reads between the two
    for _ in 0..20 {
        assert_eq!(token_acct.amount(), 500);
        assert_eq!(mint_acct.supply(), 1_000_000);
        assert_eq!(mint_acct.decimals(), 6);
        let _ = token_acct.mint();
        let _ = mint_acct.mint_authority();
    }
}

#[test]
fn spl_token_id_and_token_2022_id_differ() {
    // Verify the two program IDs are distinct (last byte differs)
    assert!(!quasar_core::keys_eq(&SPL_TOKEN_ID, &TOKEN_2022_ID));
    // Verify specific byte difference
    assert_ne!(SPL_TOKEN_BYTES[31], TOKEN_2022_BYTES[31]);
}

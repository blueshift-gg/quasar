//! Miri UB tests for quasar-spl unsafe code paths.
//!
//! These tests are designed to find undefined behavior, not confirm correct
//! output. Each test exercises a specific unsafe pattern under conditions
//! that would trigger Miri if the pattern is unsound.
#![allow(
    clippy::undocumented_unsafe_blocks,
    clippy::manual_div_ceil,
    clippy::useless_vec,
    clippy::deref_addrof,
    clippy::needless_range_loop,
    clippy::borrow_deref_ref,
    reason = "this adversarial fixture centralizes account-buffer safety at its constructors"
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
//! - `-Zmiri-tree-borrows`: Tree Borrows model. The shared-to-mutable cast in
//!   `from_account_view_mut` is instant UB under Stacked Borrows. Under Tree
//!   Borrows it is sound because the `&mut` never writes to the AccountView
//!   memory itself; writes go through the raw pointer to a separate
//!   RuntimeAccount allocation.
//! - `-Zmiri-symbolic-alignment-check`: Catch alignment issues that depend on
//!   allocation placement rather than happenstance.
//!
//! ## Findings
//!
//! | Pattern | Result |
//! |---------|--------|
//! | `&AccountView -> &TokenDataZc` via Deref | Sound |
//! | `&AccountView -> &mut TokenDataZc` via DerefMut | Sound under Tree Borrows |
//! | `&AccountView -> &MintDataZc` via Deref | Sound |
//! | `&AccountView -> &InterfaceAccount<T>` cast | Sound |
//! | `&AccountView -> &mut InterfaceAccount<T>` cast | Sound under Tree Borrows |
//! | `view.owner()` read for CheckOwner | Sound |
//! | MaybeUninit [u8; N] instruction data (transfer, mint_to, etc.) | Sound |
//! | ZeroCopyDeref `deref_from` / `deref_from_mut` | Sound under Tree Borrows |
//! | Interleaved shared/mutable access via InterfaceAccount | Sound under Tree Borrows |
//!
//! ## What Miri cannot test
//!
//! | Pattern | Why |
//! |---------|-----|
//! | `sol_invoke_signed_c` syscall | FFI, SBF-only |
//! | Actual CPI execution | Requires SVM runtime |
//! | Token-2022 extensions beyond 165/82 bytes | Layout-dependent on runtime |

use {
    quasar_lang::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        account_load::AccountLoad,
        accounts::{account::set_lamports, Account},
        traits::*,
    },
    quasar_spl::{
        validate_mint_with_freeze, validate_token_account, FreezeCheck, InterfaceAccount, Mint,
        MintDataZc, Token, TokenDataZc, SPL_TOKEN_ID, TOKEN_2022_ID,
    },
    solana_address::Address,
    solana_program_error::ProgramError,
    std::mem::{size_of, MaybeUninit},
};

const SPL_TOKEN_BYTES: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];
const TOKEN_2022_BYTES: [u8; 32] = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
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

#[path = "miri/adversarial.rs"]
mod adversarial;
#[path = "miri/instruction_data.rs"]
mod instruction_data;
#[path = "miri/interface_accounts.rs"]
mod interface_accounts;
#[path = "miri/mint_views.rs"]
mod mint_views;
#[path = "miri/owner_checks.rs"]
mod owner_checks;
#[path = "miri/token_views.rs"]
mod token_views;
#[path = "miri/validation.rs"]
mod validation;
#[path = "miri/zero_copy.rs"]
mod zero_copy;

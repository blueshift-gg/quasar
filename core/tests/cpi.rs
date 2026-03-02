use std::mem::size_of;

use quasar_core::__internal::{
    AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
};
use quasar_core::cpi::{system, BufCpiCall, InstructionAccount};
use solana_address::Address;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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
}

// ---------------------------------------------------------------------------
// create_account — construction + invoke
// ---------------------------------------------------------------------------

#[test]
fn create_account_constructs_without_panic() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);
    let to = unsafe { to_buf.view() };

    let owner = Address::new_from_array([3u8; 32]);
    let cpi = system::create_account(&from, &to, 1000u64, 64, &owner);
    assert!(cpi.invoke().is_ok());
}

#[test]
fn create_account_space_zero() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);
    let to = unsafe { to_buf.view() };

    let owner = Address::new_from_array([0u8; 32]);
    let cpi = system::create_account(&from, &to, 0u64, 0, &owner);
    assert!(cpi.invoke().is_ok());
}

// ---------------------------------------------------------------------------
// transfer — construction + invoke
// ---------------------------------------------------------------------------

#[test]
fn transfer_constructs_without_panic() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 10_000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let to = unsafe { to_buf.view() };

    let cpi = system::transfer(&from, &to, 5000u64);
    assert!(cpi.invoke().is_ok());
}

#[test]
fn transfer_zero_lamports() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let to = unsafe { to_buf.view() };

    let cpi = system::transfer(&from, &to, 0u64);
    assert!(cpi.invoke().is_ok());
}

#[test]
fn transfer_max_lamports() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], u64::MAX, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let to = unsafe { to_buf.view() };

    let cpi = system::transfer(&from, &to, u64::MAX);
    assert!(cpi.invoke().is_ok());
}

// ---------------------------------------------------------------------------
// assign — construction + invoke
// ---------------------------------------------------------------------------

#[test]
fn assign_constructs_without_panic() {
    let mut acct_buf = AccountBuffer::new(0);
    acct_buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let acct = unsafe { acct_buf.view() };

    let owner = Address::new_from_array([5u8; 32]);
    let cpi = system::assign(&acct, &owner);
    assert!(cpi.invoke().is_ok());
}

#[test]
fn assign_zero_owner() {
    let mut acct_buf = AccountBuffer::new(0);
    acct_buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let acct = unsafe { acct_buf.view() };

    let owner = Address::new_from_array([0u8; 32]);
    let cpi = system::assign(&acct, &owner);
    assert!(cpi.invoke().is_ok());
}

// ---------------------------------------------------------------------------
// invoke_signed with empty seeds
// ---------------------------------------------------------------------------

#[test]
fn invoke_signed_empty_seeds() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 10_000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let to = unsafe { to_buf.view() };

    let cpi = system::transfer(&from, &to, 100u64);
    assert!(cpi.invoke_signed(&[]).is_ok());
}

// ---------------------------------------------------------------------------
// CpiCall const-generic sizing (compile-time type check)
// ---------------------------------------------------------------------------

#[test]
fn create_account_type_is_cpi_call_2_52() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);
    let to = unsafe { to_buf.view() };

    let owner = Address::new_from_array([3u8; 32]);
    let _: quasar_core::cpi::CpiCall<'_, 2, 52> =
        system::create_account(&from, &to, 1000u64, 64, &owner);
}

#[test]
fn transfer_type_is_cpi_call_2_12() {
    let mut from_buf = AccountBuffer::new(0);
    from_buf.init([1u8; 32], [0u8; 32], 1000, 0, true, true);
    let from = unsafe { from_buf.view() };

    let mut to_buf = AccountBuffer::new(0);
    to_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let to = unsafe { to_buf.view() };

    let _: quasar_core::cpi::CpiCall<'_, 2, 12> = system::transfer(&from, &to, 100u64);
}

#[test]
fn assign_type_is_cpi_call_1_36() {
    let mut acct_buf = AccountBuffer::new(0);
    acct_buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let acct = unsafe { acct_buf.view() };

    let owner = Address::new_from_array([5u8; 32]);
    let _: quasar_core::cpi::CpiCall<'_, 1, 36> = system::assign(&acct, &owner);
}

// ---------------------------------------------------------------------------
// BufCpiCall — construction, assertion, invoke
// ---------------------------------------------------------------------------

#[test]
fn buf_cpi_call_constructs_with_valid_data_len() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let _: BufCpiCall<'_, 1, 16> = BufCpiCall::new(
        &program_id,
        [InstructionAccount::readonly(view.address())],
        [&view],
        [0u8; 16],
        10,
    );
}

#[test]
fn buf_cpi_call_data_len_zero() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<1, 8>::new(
        &program_id,
        [InstructionAccount::readonly(view.address())],
        [&view],
        [0u8; 8],
        0,
    );
    assert!(cpi.invoke().is_ok());
}

#[test]
fn buf_cpi_call_data_len_at_max() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<1, 32>::new(
        &program_id,
        [InstructionAccount::readonly(view.address())],
        [&view],
        [0xAA; 32],
        32,
    );
    assert!(cpi.invoke().is_ok());
}

#[test]
#[should_panic(expected = "BufCpiCall: data_len exceeds buffer capacity")]
fn buf_cpi_call_panics_when_data_len_exceeds_max() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let _ = BufCpiCall::<1, 8>::new(
        &program_id,
        [InstructionAccount::readonly(view.address())],
        [&view],
        [0u8; 8],
        9,
    );
}

#[test]
fn buf_cpi_call_invoke_returns_ok() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<1, 16>::new(
        &program_id,
        [InstructionAccount::writable_signer(view.address())],
        [&view],
        [0u8; 16],
        4,
    );
    assert!(cpi.invoke().is_ok());
}

#[test]
fn buf_cpi_call_invoke_signed_empty_seeds() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<1, 16>::new(
        &program_id,
        [InstructionAccount::writable_signer(view.address())],
        [&view],
        [0u8; 16],
        4,
    );
    assert!(cpi.invoke_signed(&[]).is_ok());
}

#[test]
fn buf_cpi_call_invoke_with_signers_empty() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 0, 0, true, true);
    let view = unsafe { buf.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<1, 16>::new(
        &program_id,
        [InstructionAccount::writable_signer(view.address())],
        [&view],
        [0u8; 16],
        4,
    );
    assert!(cpi.invoke_with_signers(&[]).is_ok());
}

#[test]
fn buf_cpi_call_multiple_accounts() {
    let mut buf1 = AccountBuffer::new(0);
    buf1.init([1u8; 32], [0u8; 32], 1000, 0, true, true);
    let view1 = unsafe { buf1.view() };

    let mut buf2 = AccountBuffer::new(0);
    buf2.init([2u8; 32], [0u8; 32], 0, 0, false, true);
    let view2 = unsafe { buf2.view() };

    let program_id = Address::new_from_array([99u8; 32]);
    let cpi = BufCpiCall::<2, 64>::new(
        &program_id,
        [
            InstructionAccount::writable_signer(view1.address()),
            InstructionAccount::writable(view2.address()),
        ],
        [&view1, &view2],
        [0u8; 64],
        20,
    );
    assert!(cpi.invoke().is_ok());
}

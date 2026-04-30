#![allow(dead_code)]

solana_address::declare_id!("11111111111111111111111111111112");

use {
    quasar_lang::{
        __internal::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
        prelude::*,
    },
    std::{mem::size_of, vec::Vec},
};

struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn new() -> Self {
        let byte_len = size_of::<RuntimeAccount>() + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();
        Self {
            inner: vec![0; byte_len.div_ceil(8)],
        }
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn init_signer(&mut self, seed: u8) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = 1;
            (*raw).is_writable = 0;
            (*raw).executable = 0;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array([seed; 32]);
            (*raw).owner = Address::new_from_array([0u8; 32]);
            (*raw).lamports = 1;
            (*raw).data_len = 0;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }
}

#[derive(Accounts)]
struct OnlySigner {
    #[account(mut)]
    signer: Signer,
}

#[test]
fn parse_accounts_rejects_too_few_non_composite_accounts() {
    let mut accounts: [AccountView; 0] = [];
    let err = match OnlySigner::parse(&mut accounts, &Address::default()) {
        Ok(_) => panic!("parse unexpectedly accepted an empty account slice"),
        Err(err) => err,
    };
    assert_eq!(err, ProgramError::NotEnoughAccountKeys);
}

#[test]
fn parse_accounts_rejects_extra_non_composite_accounts() {
    let mut first = AccountBuffer::new();
    first.init_signer(1);
    let mut second = AccountBuffer::new();
    second.init_signer(2);
    let mut accounts = unsafe { [first.view(), second.view()] };

    let err = match OnlySigner::parse(&mut accounts, &Address::default()) {
        Ok(_) => panic!("parse unexpectedly accepted extra accounts"),
        Err(err) => err,
    };
    assert_eq!(err, ProgramError::InvalidArgument);
}

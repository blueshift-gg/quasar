//! Bounded reusable account groups.
#![allow(unexpected_cfgs)]
extern crate alloc;

use quasar_derive::Accounts;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct SignerPair {
    pub first: Signer,
    pub second: Signer,
}

#[derive(Accounts)]
pub struct UsesAccountArray {
    pub payer: Signer,
    pub pairs: AccountsArray<SignerPair, 2>,
}

fn main() {
    fn _assert_count<T: AccountCount>() {}
    _assert_count::<AccountsArray<SignerPair, 2>>();
    _assert_count::<UsesAccountArray>();
}

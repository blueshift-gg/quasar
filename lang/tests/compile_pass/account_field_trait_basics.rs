#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Verifies that AccountLoad trait is implemented for all wrapper types
/// and that the const bools are correctly set.
///
/// This is a compile-time test — the assertions are const.

// Account<T>
#[account(discriminator = 1)]
pub struct MyData {
    pub value: u64,
    pub bump: u8,
}

const _: () = assert!(!<Account<MyData> as AccountLoad>::IS_SIGNER);
const _: () = assert!(!<Account<MyData> as AccountLoad>::IS_EXECUTABLE);

// Signer has IS_SIGNER = true
const _: () = assert!(<Signer as AccountLoad>::IS_SIGNER);

// Program<T> has IS_EXECUTABLE = true
const _: () = assert!(<Program<System> as AccountLoad>::IS_EXECUTABLE);

// UncheckedAccount has all defaults false
const _: () = assert!(!<UncheckedAccount as AccountLoad>::IS_SIGNER);
const _: () = assert!(!<UncheckedAccount as AccountLoad>::IS_EXECUTABLE);

fn main() {}

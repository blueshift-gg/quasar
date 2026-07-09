#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Config {
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Bad {
    // Quasar has no raw-seed-array PDA form; addresses use typed
    // `Type::seeds(...)`. A raw byte-string array does not implement
    // `AddressVerify`, so using one as an `address = ...` constraint must fail.
    #[account(address = [b"config"])]
    pub config: Account<Config>,
}

fn main() {}

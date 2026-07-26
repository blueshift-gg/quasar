//! Signed integers as PDA seeds, stored as their little-endian bytes like the
//! unsigned widths.
#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_derive::Accounts;
use quasar_lang::prelude::*;
solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"tick", tick: i32, offset: i64, flag: i8, small: i16)]
pub struct Tick {
    pub authority: Address,
}

#[derive(Accounts)]
#[instruction(tick: i32, offset: i64, flag: i8, small: i16)]
pub struct UseSigned {
    #[account(address = Tick::seeds(tick, offset, flag, small))]
    pub tick_account: Account<Tick>,
}
fn main() {}

#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"item", namespace: u32)]
pub struct Item {
    pub namespace: u32,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Bad {
    #[account(mut)]
    pub payer: Signer,
    // The PDA is derived from `item.namespace`, but `item` is the account being
    // initialized — its data does not exist at address-derivation time, so the
    // self-referential seed must fail to compile.
    #[account(mut, init, payer = payer, address = Item::seeds(item.namespace))]
    pub item: Account<Item>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}

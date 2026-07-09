#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// A multi-field fixed account whose `Space::SPACE` now references
// `<__Schema as ZeroPodFixed>::SIZE` instead of re-summing the field pod sizes.
#[account(discriminator = [1, 2])]
pub struct SpaceProbe {
    pub authority: Address,
    pub amount: u64,
    pub flag: bool,
}

// Prove the schema reference equals the old field-wise sum
// (disc_len + Σ size_of::<field pod companion>). If a future schema change makes
// `ZeroPodFixed::SIZE` diverge from the field sum, this const-assert fails at
// compile time instead of silently shipping a wrong account footprint.
const _: () = assert!(
    <SpaceProbe as quasar_lang::traits::Space>::SPACE
        == 2 + core::mem::size_of::<
            <Address as quasar_lang::instruction_arg::InstructionArg>::Zc,
        >()
            + core::mem::size_of::<<u64 as quasar_lang::instruction_arg::InstructionArg>::Zc>()
            + core::mem::size_of::<<bool as quasar_lang::instruction_arg::InstructionArg>::Zc>(),
);

fn main() {}

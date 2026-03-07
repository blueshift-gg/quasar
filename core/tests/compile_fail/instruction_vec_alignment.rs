#![allow(unexpected_cfgs)]
use quasar_core::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

type Vec<T, const N: usize> = quasar_core::dynamic::Vec<T, u32, N>;

#[derive(Accounts)]
pub struct Test<'info> {
    pub signer: &'info Signer,
}

#[instruction(discriminator = 0)]
pub fn bad_vec(_ctx: Ctx<Test>, _vals: Vec<u64, 2>) -> Result<(), ProgramError> {
    Ok(())
}

fn main() {}

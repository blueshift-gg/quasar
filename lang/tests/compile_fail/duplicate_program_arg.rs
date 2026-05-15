#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[program(no_entrypoint, no_entrypoint)]
pub mod duplicate_program_arg {}

fn main() {}

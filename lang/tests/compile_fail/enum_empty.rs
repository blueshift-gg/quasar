use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// An enum with zero variants has no inhabited value for `validate_zc`
// to ever accept, so the derive must reject it up-front rather than
// emitting code that is impossible to exercise.
#[repr(u8)]
#[derive(QuasarSerialize)]
pub enum Empty {}

fn main() {}

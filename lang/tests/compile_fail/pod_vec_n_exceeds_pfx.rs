use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// PodVec<u8, 300, 1>: N=300 exceeds max for PFX=1 (u8::MAX = 255)
const _: () = PodVec::<u8, 300, 1>::VALID;

fn main() {}

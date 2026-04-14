use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// PodString<256, 1>: N=256 exceeds max for PFX=1 (u8::MAX = 255)
const _: () = PodString::<256, 1>::VALID;

fn main() {}

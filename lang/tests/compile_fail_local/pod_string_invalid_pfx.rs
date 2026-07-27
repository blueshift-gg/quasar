use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// PodString<10, 3>: PFX=3 is not a valid prefix width (must be 1, 2, 4, or 8)
const _: () = PodString::<10, 3>::VALID;

fn main() {}

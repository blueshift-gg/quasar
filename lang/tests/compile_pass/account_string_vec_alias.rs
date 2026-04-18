#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Account using user-facing String<N> and Vec<T, N> aliases in fields.
/// The #[account] macro's map_to_pod_type rewrites these to PodString/PodVec
/// in the ZC struct.
/// Fixed fields must precede dynamic PodString/PodVec fields.
#[account(discriminator = 1, set_inner)]
pub struct Profile {
    pub bump: u8,
    pub name: String<32>,
    pub scores: Vec<u8, 10>,
}

fn main() {}

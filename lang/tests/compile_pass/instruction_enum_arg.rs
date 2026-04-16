#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

/// Explicit, non-contiguous discriminants — the common "wire format"
/// shape where two sides of an exchange need stable on-chain tags.
#[repr(u8)]
#[derive(Copy, Clone, QuasarSerialize)]
pub enum Side {
    Bid = 7,
    Ask = 42,
}

/// Implicit discriminants (the `0, 1, 2, ...` shape) — exercises the
/// `*self as u8` path without the user writing explicit tags.
#[repr(u8)]
#[derive(Copy, Clone, QuasarSerialize)]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Accounts)]
pub struct PlaceOrder {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn place_order(
        _ctx: Ctx<PlaceOrder>,
        _side: Side,
        _priority: Priority,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}

use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct EscrowAccount {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}

#[event(discriminator = 1)]
pub struct EscrowTaken {
    pub maker: Address,
    pub taker: Address,
    pub amount: u64,
}
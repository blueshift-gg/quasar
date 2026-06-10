use quasar_lang::prelude::*;

#[account(discriminator = 1, set_inner)]
#[seeds(b"escrow", maker: Address)]
pub struct Escrow {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}

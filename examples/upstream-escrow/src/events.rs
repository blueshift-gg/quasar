use quasar_lang::prelude::*;

#[event(discriminator = 0)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub deposit: u64,
    pub receive: u64,
}

#[event(discriminator = 1)]
pub struct TakeEvent {
    pub escrow: Address,
}

#[event(discriminator = 2)]
pub struct RefundEvent {
    pub escrow: Address,
}

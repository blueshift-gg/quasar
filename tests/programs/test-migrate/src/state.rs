use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct ConfigV1 {
    pub authority: Address,
    pub value: PodU64,
}

#[account(discriminator = 2)]
pub struct ConfigV2 {
    pub authority: Address,
    pub value: PodU64,
    pub extra: PodU32,
}

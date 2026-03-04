use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct SimpleAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = [1, 2])]
pub struct MultiDiscAccount {
    pub data: u64,
}

#[account(discriminator = 5)]
pub struct DynamicAccount<'a> {
    pub name: String<'a, 8>,
    pub tags: Vec<'a, Address, 2>,
}

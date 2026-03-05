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
    pub name: String<u32, 8>,
    pub tags: Vec<Address, u32, 2>,
}

#[account(discriminator = 6)]
pub struct MixedAccount<'a> {
    pub authority: Address,
    pub value: u64,
    pub label: String<u32, 32>,
}

#[account(discriminator = 7)]
pub struct SmallPrefixAccount<'a> {
    pub tag: String<u8, 100>,
    pub scores: Vec<u8, u8, 10>,
}

#[account(discriminator = 8)]
pub struct TailStrAccount<'a> {
    pub authority: Address,
    pub label: &'a str,
}

#[account(discriminator = 9)]
pub struct TailBytesAccount<'a> {
    pub authority: Address,
    pub data: &'a [u8],
}

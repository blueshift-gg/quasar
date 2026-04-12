use quasar_lang::prelude::*;

pub const SIDE_A: u8 = 0;
pub const SIDE_B: u8 = 1;

#[account(discriminator = 1, set_inner)]
#[seeds(b"config")]
pub struct ConfigAccount {
    pub bump: u8,
}

#[account(discriminator = 2, set_inner)]
#[seeds(b"user", authority: Address)]
pub struct UserAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = 3, set_inner)]
#[seeds(b"item", authority: Address)]
pub struct ItemAccount {
    pub id: u64,
    pub bump: u8,
}

#[account(discriminator = 4, set_inner)]
#[seeds(b"complex", payer: Address, authority: Address)]
pub struct ComplexAccount {
    pub authority: Address,
    pub amount: u64,
    pub bump: u8,
}

#[account(discriminator = 5, set_inner)]
#[seeds(b"")]
pub struct EmptySeedAccount {
    pub bump: u8,
}

#[account(discriminator = 6, set_inner)]
#[seeds(b"abcdefghijklmnopqrstuvwxyz012345")]
pub struct MaxSeedAccount {
    pub bump: u8,
}

#[account(discriminator = 7, set_inner)]
#[seeds(b"triple", first: Address, second: Address)]
pub struct ThreeSeedAccount {
    pub first: Address,
    pub second: Address,
    pub bump: u8,
}

#[account(discriminator = 8, set_inner)]
#[seeds(b"indexed", authority: Address, index: u64)]
pub struct IndexedAccount {
    pub authority: Address,
    pub index: u64,
    pub bump: u8,
}

#[account(discriminator = 9, set_inner)]
#[seeds(b"ns_config")]
pub struct NamespaceConfig {
    pub namespace: u32,
    pub bump: u8,
}

#[account(discriminator = 10, set_inner)]
#[seeds(b"scoped", namespace: u32)]
pub struct ScopedItem {
    pub namespace: u32,
    pub data: u64,
    pub bump: u8,
}

#[account(discriminator = 11, set_inner)]
#[seeds(b"intake", authority: Address, side: u8)]
pub struct IntakeQueue {
    pub authority: Address,
    pub side: u8,
    pub bump: u8,
}

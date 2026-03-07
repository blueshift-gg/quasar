use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct ConfigAccount {
    pub bump: u8,
}

#[account(discriminator = 2)]
pub struct UserAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = 3)]
pub struct ItemAccount {
    pub id: u64,
    pub bump: u8,
}

#[account(discriminator = 4)]
pub struct ComplexAccount {
    pub authority: Address,
    pub amount: u64,
    pub bump: u8,
}

#[account(discriminator = 5)]
pub struct EmptySeedAccount {
    pub bump: u8,
}

#[account(discriminator = 6)]
pub struct MaxSeedAccount {
    pub bump: u8,
}

#[account(discriminator = 7)]
pub struct ThreeSeedAccount {
    pub first: Address,
    pub second: Address,
    pub bump: u8,
}

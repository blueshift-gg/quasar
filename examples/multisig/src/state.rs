use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct MultisigConfig<'a> {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    pub label: String<'a, 32>,
    pub signers: Vec<'a, Address, 10>,
}

use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct ErrorTestAccount {
    pub authority: Address,
    pub value: u64,
}

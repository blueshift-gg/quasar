use quasar_core::prelude::*;

#[account(discriminator = 1)]
pub struct ClockSnapshot {
    pub slot: u64,
    pub unix_timestamp: i64,
}

#[account(discriminator = 2)]
pub struct RentSnapshot {
    pub min_balance_100: u64,
}

use {
    crate::pyth::{PythPrice, SOL_USD_FEED},
    quasar_lang::prelude::*,
};

const MAX_AGE: u64 = 30;

#[derive(Accounts)]
pub struct CheckPrice {
    pub user: Signer,
    // validated manually via PythPrice::from_account.
    pub price_feed: UncheckedAccount,
    pub clock: Sysvar<Clock>,
}

impl CheckPrice {
    #[inline(always)]
    pub fn check_price(&self) -> Result<(), ProgramError> {
        let view = self.price_feed.to_account_view();
        let oracle = PythPrice::from_account(view)?;
        let clock = self.clock.get();
        oracle.validate(&SOL_USD_FEED, MAX_AGE, clock.unix_timestamp.get())
    }
}

use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
pub struct Counter {
    pub count: u64,
}

#[account(discriminator = 1)]
pub struct Balance {
    pub amount: u64,
}

fn main() {}

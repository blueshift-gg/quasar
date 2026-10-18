use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Accounts)]
pub struct Approve {
    pub signers: Remaining<Signer>,
    pub config: UncheckedAccount,
}

fn main() {}

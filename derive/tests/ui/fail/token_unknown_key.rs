use quasar_lang::prelude::*;
use quasar_spl::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Accounts)]
pub struct Check {
    pub owner: Signer,
    #[account(token(owner = owner))]
    pub vault: InterfaceAccount<TokenAccount>,
}

fn main() {}

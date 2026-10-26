use quasar_lang::prelude::*;
use quasar_spl::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Accounts)]
pub struct Open {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<Mint>,
    #[account(init(payer = payer), token(mint = mint))]
    pub vault: InterfaceAccount<TokenAccount>,
    pub token_program: Interface<TokenInterface>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}

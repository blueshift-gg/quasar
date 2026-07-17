#![allow(unexpected_cfgs)]

use {quasar_lang::prelude::*, quasar_spl::prelude::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"authority", owner: Address)]
pub struct VaultAuthority {
    pub owner: Address,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct CloseVault {
    pub owner: Signer,
    #[account(address = VaultAuthority::seeds(owner.address()))]
    pub authority: Account<VaultAuthority>,
    #[account(
        mut,
        token_close(
            dest = destination,
            authority = authority,
            token_program = token_program
        )
    )]
    pub vault: Account<Token>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}

fn main() {}

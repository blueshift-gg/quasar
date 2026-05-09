#![allow(unexpected_cfgs)]

use {quasar_derive::Accounts, quasar_lang::prelude::*, quasar_spl::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct DeferredTokenInit {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub vault: Uninit<Account<Token>>,
}

#[program]
pub mod test_uninit_spl {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn init_token(ctx: Ctx<DeferredTokenInit>) -> Result<(), ProgramError> {
        ctx.accounts.vault.init(
            &ctx.accounts.payer,
            TokenInitKind::Token {
                mint: ctx.accounts.mint.to_account_view(),
                authority: ctx.accounts.payer.address(),
                token_program: ctx.accounts.token_program.to_account_view(),
            },
        )?;
        Ok(())
    }
}

fn main() {}

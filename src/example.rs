use crate::prelude::*;

no_alloc!();
panic_handler!();

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: &'info mut Signer,
    pub escrow: &'info mut Initialize<EscrowAccount>,
    pub maker_ta_a: &'info mut Account<TokenAccount>,
    pub maker_ta_b: &'info Account<TokenAccount>,
    pub vault_ta_a: &'info Account<TokenAccount>,
    pub rent: &'info Rent,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

#[account(discriminator = 1)]
pub struct EscrowAccount {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
}

#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: &'info mut Signer,
    #[account(has_one = maker, constraint = escrow.receive > 0)]
    pub escrow: &'info Account<EscrowAccount>,
    pub maker: &'info UncheckedAccount,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

#[program]
mod instruction {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(ctx: Ctx<Make>, receive: u64) -> Result<(), ProgramError> {
        EscrowAccount {
            maker: ctx.accounts.maker.to_account_view().address().clone(),
            mint_a: ctx.accounts.maker_ta_a.mint().clone(),
            mint_b: ctx.accounts.maker_ta_b.mint().clone(),
            maker_ta_b: ctx.accounts.maker_ta_b.to_account_view().address().clone(),
            receive
        }.init(
            ctx.accounts.escrow,
            ctx.accounts.maker.to_account_view(),
            Some(ctx.accounts.rent),
        )
    }

    #[instruction(discriminator = 1)]
    pub fn take(ctx: Ctx<Take>) -> Result<(), ProgramError> {
        let _escrow_data = ctx.accounts.escrow.get()?;
        Ok(())
    }

    #[instruction(discriminator = 2)]
    pub fn refund(ctx: Ctx<Take>) -> Result<(), ProgramError> {
        let _escrow_data = ctx.accounts.escrow.get()?;
        Ok(())
    }
}

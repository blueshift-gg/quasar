use anchor_lang_v2::prelude::*;

#[cfg(test)]
mod tests;

declare_id!("33333333333333333333333333333333333333333333");

#[program]
pub mod anchor_v2_vault {
    use super::*;

    #[discrim = 0]
    pub fn deposit(ctx: &mut Context<'_, Deposit>, amount: u64) -> Result<()> {
        pinocchio_system::instructions::Transfer {
            from: ctx.accounts.user.account(),
            to: ctx.accounts.vault.account(),
            lamports: amount,
        }
        .invoke()?;
        Ok(())
    }

    #[discrim = 1]
    pub fn withdraw(ctx: &mut Context<'_, Withdraw>, amount: u64) -> Result<()> {
        let mut vault = *ctx.accounts.vault.account();
        let mut user = *ctx.accounts.user.account();
        vault.set_lamports(vault.lamports() - amount);
        user.set_lamports(user.lamports() + amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub user: Signer,
    #[account(
        mut,
        seeds = [b"vault", user.account().address().as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Withdraw {
    #[account(mut)]
    pub user: Signer,
    #[account(
        mut,
        seeds = [b"vault", user.account().address().as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount,
}

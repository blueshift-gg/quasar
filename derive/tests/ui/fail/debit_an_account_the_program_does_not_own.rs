use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

const OTHER: Address = Address::new_from_array([9; 32]);

#[account(discriminator = 1, owner = OTHER)]
pub struct Tally {
    pub count: u64,
}

#[derive(Accounts)]
pub struct Take {
    #[account(mut)]
    pub tally: Account<Tally>,
}
#[program]
mod taking {
    use super::*;

    #[instruction(0)]
    pub fn take(mut ctx: Ctx<Take>) -> Result<()> {
        ctx.accounts.tally.sub_lamports(1)
    }
}

fn main() {}

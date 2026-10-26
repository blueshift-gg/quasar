use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Accounts)]
pub struct Shut {
    #[account(mut, close(to = to))]
    pub wallet: SystemAccount,
    #[account(mut)]
    pub to: UncheckedAccount,
}
#[program]
mod shutting {
    use super::*;

    #[instruction(0)]
    pub fn shut(_ctx: Ctx<Shut>) -> Result {
        Ok(())
    }
}

fn main() {}
